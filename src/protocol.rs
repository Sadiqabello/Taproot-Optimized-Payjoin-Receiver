use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use bitcoin::psbt::Input as PsbtInput;
use bitcoin::{Amount, FeeRate, TxIn};
use payjoin::receive::InputPair;
use url::Url;

use crate::cli::ReceiveArgs;
use crate::coin_selection;
use crate::config::Config;
use crate::persistence;
use crate::rpc::BitcoinRpc;
use crate::session::{SeenInputs, SessionState, SessionStatus};

/// Default Payjoin Directory URL.
const DEFAULT_DIRECTORY: &str = "https://payjo.in";
/// Default OHTTP relay URL.
const DEFAULT_OHTTP_RELAY: &str = "https://pj.bobspacebkk.com";

/// Run a complete Payjoin v2 receive session.
///
/// This implements the full BIP 77 receiver flow:
/// 1. Connect to Bitcoin Core and fetch wallet UTXOs
/// 2. Generate a receiving address (Bech32m / P2TR)
/// 3. Initialize a v2 session with the Payjoin Directory
/// 4. Display BIP 21 URI for the sender
/// 5. Poll for incoming Original PSBT
/// 6. Validate the Original PSBT
/// 7. Select receiver inputs via coin selection engine
/// 8. Construct and sign the Payjoin Proposal
/// 9. Submit the proposal back to the directory
pub async fn run_receive_session(config: &Config, args: &ReceiveArgs) -> Result<()> {
    let payment_amount = Amount::from_sat(args.amount);

    // 1. Connect to Bitcoin Core
    let rpc = BitcoinRpc::new(config)?;
    rpc.verify_connection()?;

    // 2. Verify we have UTXOs available
    let wallet_utxos = rpc.list_unspent()?;
    if wallet_utxos.is_empty() {
        anyhow::bail!(
            "No spendable UTXOs in wallet. Fund the wallet before receiving a Payjoin."
        );
    }

    tracing::info!(
        utxo_count = wallet_utxos.len(),
        "Wallet UTXOs loaded"
    );

    // 3. Pre-compute coin selection to show the user what will be contributed
    let feerate = rpc.estimate_feerate(6)?;
    let strategy = args.strategy;
    let selection = coin_selection::select_inputs(
        &wallet_utxos,
        payment_amount,
        strategy,
        args.max_inputs,
        feerate,
    )?;

    if selection.selected.is_empty() {
        anyhow::bail!("Coin selection found no suitable UTXOs for this payment.");
    }

    println!("Coin Selection Preview:");
    println!("  Strategy:    {}", strategy);
    println!("  Fee rate:    {:.1} sat/vB", feerate);
    println!("  Selected:    {} input(s)", selection.selected.len());
    println!(
        "  Total value: {} sats",
        selection.total_value.to_sat()
    );
    for (i, scored) in selection.selected.iter().enumerate() {
        println!(
            "  Input #{}: {} sats ({}) — score: {:.1}",
            i + 1,
            scored.utxo.amount.to_sat(),
            scored.script_type,
            scored.score,
        );
    }

    // 4. Generate receiving address
    let address = rpc.get_new_address()?;
    tracing::info!(address = %address, "Generated receiving address");

    // 5. Resolve directory and relay URLs
    let directory_url = args
        .directory
        .as_deref()
        .unwrap_or(DEFAULT_DIRECTORY);
    let ohttp_relay_url = args
        .ohttp_relay
        .as_deref()
        .unwrap_or(DEFAULT_OHTTP_RELAY);

    let directory = Url::parse(directory_url)
        .context("Invalid directory URL")?;
    let ohttp_relay = Url::parse(ohttp_relay_url)
        .context("Invalid OHTTP relay URL")?;

    // 6. Fetch OHTTP keys from the relay
    println!("\nFetching OHTTP keys from relay...");
    let ohttp_keys = payjoin::io::fetch_ohttp_keys(
        ohttp_relay.clone(),
        directory.clone(),
    )
    .await
    .context("Failed to fetch OHTTP keys from relay")?;

    // 7. Initialize BIP 77 v2 session
    let mut session = payjoin::receive::v2::Receiver::new(
        address,
        directory,
        ohttp_keys,
        ohttp_relay,
        Some(std::time::Duration::from_secs(args.expiry * 60)),
    );

    let pj_uri = session.pj_uri_builder()
        .amount(payment_amount)
        .build();

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let session_state = SessionState {
        id: format!("{:x}", now),
        pj_uri: pj_uri.to_string(),
        amount_sats: args.amount,
        status: SessionStatus::Pending,
        created_at: now,
        expires_at: now + (args.expiry * 60),
        label: args.label.clone(),
        strategy: strategy.to_string(),
    };
    persistence::upsert_session(config, session_state.clone())?;

    println!("\nShare this URI with the sender:");
    println!("{}", pj_uri);
    println!("\nWaiting for sender's Original PSBT...");
    println!("(Press Ctrl+C to cancel)\n");

    // 8. Poll for incoming Original PSBT
    let http = reqwest::Client::new();

    let unchecked_proposal = loop {
        let (request, ohttp_ctx) = session
            .extract_req()
            .map_err(|e| anyhow!("Failed to create polling request: {}", e))?;

        let response = http
            .post(request.url)
            .header("Content-Type", request.content_type)
            .body(request.body)
            .send()
            .await
            .context("Failed to poll directory")?;

        let response_bytes = response.bytes().await?;

        match session.process_res(&response_bytes, ohttp_ctx) {
            Ok(Some(proposal)) => break proposal,
            Ok(None) => {
                tracing::debug!("No proposal yet, polling again...");
                tokio::time::sleep(config.poll_interval).await;
            }
            Err(e) => {
                tracing::warn!("Error processing response: {}", e);
                tokio::time::sleep(config.poll_interval).await;
            }
        }
    };

    println!("Received Original PSBT from sender!");

    // 9. Validate the Original PSBT
    let mut seen_inputs = SeenInputs::load(config)?;

    // For async/interactive receivers, skip broadcast suitability check
    let maybe_inputs_owned = unchecked_proposal.assume_interactive_receiver();

    let maybe_inputs_seen = maybe_inputs_owned
        .check_inputs_not_owned(|script| {
            Ok(rpc.is_mine(script).unwrap_or(false))
        })
        .map_err(|e| anyhow!("Validation failed: sender inputs owned by receiver: {}", e))?;

    let outputs_unknown = maybe_inputs_seen
        .check_no_inputs_seen_before(|outpoint| {
            Ok(!seen_inputs.contains(outpoint))
        })
        .map_err(|e| anyhow!("Validation failed: replay detected: {}", e))?;

    let wants_outputs = outputs_unknown
        .identify_receiver_outputs(|script| {
            Ok(rpc.is_mine(script).unwrap_or(false))
        })
        .map_err(|e| anyhow!("Failed to identify receiver outputs: {}", e))?;

    // Record seen inputs for replay protection
    // Access the original PSBT to get sender input outpoints
    // (done after validation to avoid storing inputs from invalid proposals)

    // 10. Commit outputs and move to input contribution phase
    let wants_inputs = wants_outputs.commit_outputs();

    // 11. Build InputPairs from our selected UTXOs
    let input_pairs: Vec<InputPair> = selection
        .selected
        .iter()
        .map(|scored| {
            let txin = TxIn {
                previous_output: scored.utxo.outpoint,
                ..Default::default()
            };
            let mut psbtin = PsbtInput::default();
            psbtin.witness_utxo = Some(bitcoin::TxOut {
                value: scored.utxo.amount,
                script_pubkey: scored.utxo.script_pubkey.clone(),
            });
            InputPair::new(txin, psbtin)
                .map_err(|e| anyhow!("Failed to create InputPair: {}", e))
        })
        .collect::<Result<Vec<_>>>()?;

    // Try to use privacy-preserving selection first, fall back to contributing all
    let wants_inputs = match wants_inputs.try_preserving_privacy(input_pairs.clone()) {
        Ok(selected_input) => {
            tracing::info!("Using privacy-preserving input selection");
            wants_inputs
                .contribute_inputs(vec![selected_input].into_iter())
                .map_err(|e| anyhow!("Failed to contribute inputs: {}", e))?
        }
        Err(_) => {
            tracing::info!("Falling back to contributing all selected inputs");
            wants_inputs
                .contribute_inputs(input_pairs.into_iter())
                .map_err(|e| anyhow!("Failed to contribute inputs: {}", e))?
        }
    };

    // 12. Commit inputs and finalize
    let provisional = wants_inputs.commit_inputs();

    // Calculate max feerate (2x current for safety margin)
    let max_feerate = FeeRate::from_sat_per_vb((feerate * 2.0).ceil() as u64)
        .unwrap_or(FeeRate::from_sat_per_vb(20).unwrap());

    let mut proposal = provisional
        .finalize_proposal(
            |psbt| {
                let psbt_str = psbt.to_string();
                let signed_str = rpc
                    .wallet_process_psbt(&psbt_str)
                    .map_err(|e| payjoin::Error::Server(e.into()))?;
                let signed_psbt: bitcoin::Psbt = signed_str
                    .parse()
                    .map_err(|e| payjoin::Error::Server(Box::new(e)))?;
                Ok(signed_psbt)
            },
            Some(FeeRate::from_sat_per_vb(1).unwrap()),
            max_feerate,
        )
        .map_err(|e| anyhow!("Failed to finalize proposal: {}", e))?;

    // Save seen inputs for replay protection
    for outpoint in proposal.utxos_to_be_locked() {
        seen_inputs.insert(outpoint);
    }
    seen_inputs.save(config)?;

    // 13. Submit proposal back to directory
    let (req, ohttp_ctx) = proposal
        .extract_v2_req()
        .map_err(|e| anyhow!("Failed to create proposal response: {}", e))?;

    let resp = http
        .post(req.url)
        .header("Content-Type", req.content_type)
        .body(req.body)
        .send()
        .await
        .context("Failed to submit proposal to directory")?;

    let resp_bytes = resp.bytes().await?;
    proposal
        .process_res(&resp_bytes, ohttp_ctx)
        .map_err(|e| anyhow!("Directory rejected proposal: {}", e))?;

    println!("Payjoin proposal sent to sender!");
    println!("Waiting for sender to broadcast the transaction...");

    // Update session status
    let mut updated_state = session_state;
    updated_state.status = SessionStatus::ProposalSent;
    persistence::upsert_session(config, updated_state)?;

    Ok(())
}
