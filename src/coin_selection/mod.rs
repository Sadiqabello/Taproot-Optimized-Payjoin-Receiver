pub mod decorrelation;
pub mod scorer;
pub mod script_types;
pub mod strategies;

use anyhow::Result;
use bitcoin::Amount;
use serde::{Deserialize, Serialize};

use crate::rpc::WalletUtxo;
use scorer::ScoredUtxo;
use strategies::StrategyWeights;

/// Coin selection strategy profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Strategy {
    /// Default balanced approach across all dimensions.
    Balanced,
    /// Maximize privacy heuristic breakage.
    PrivacyMax,
    /// Minimize added transaction weight/fees.
    FeeMin,
    /// Maximize UTXO consolidation during low fees.
    Consolidate,
}

impl std::fmt::Display for Strategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Strategy::Balanced => write!(f, "balanced"),
            Strategy::PrivacyMax => write!(f, "privacy-max"),
            Strategy::FeeMin => write!(f, "fee-min"),
            Strategy::Consolidate => write!(f, "consolidate"),
        }
    }
}

impl std::str::FromStr for Strategy {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "balanced" => Ok(Strategy::Balanced),
            "privacy-max" => Ok(Strategy::PrivacyMax),
            "fee-min" => Ok(Strategy::FeeMin),
            "consolidate" => Ok(Strategy::Consolidate),
            _ => Err(format!(
                "Unknown strategy '{}'. Use: balanced, privacy-max, fee-min, consolidate",
                s
            )),
        }
    }
}

/// Result of coin selection — the UTXOs chosen for the Payjoin proposal.
#[derive(Debug, Clone)]
pub struct SelectionResult {
    /// Selected UTXOs with their scores.
    pub selected: Vec<ScoredUtxo>,
    /// Total value of selected inputs.
    pub total_value: Amount,
    /// Strategy used for selection.
    pub strategy: Strategy,
}

/// Select the optimal receiver inputs for a Payjoin transaction.
///
/// This is the main entry point for the coin selection engine.
/// It scores all candidate UTXOs using the specified strategy and
/// returns the top-scoring set up to `max_inputs`.
pub fn select_inputs(
    candidates: &[WalletUtxo],
    payment_amount: Amount,
    strategy: Strategy,
    max_inputs: usize,
    feerate_sat_per_vb: f64,
) -> Result<SelectionResult> {
    let weights = StrategyWeights::for_strategy(strategy);

    // Score all spendable candidates
    let mut scored: Vec<ScoredUtxo> = candidates
        .iter()
        .filter(|u| u.spendable)
        .map(|utxo| scorer::score_utxo(utxo, payment_amount, feerate_sat_per_vb, &weights))
        .collect();

    // Sort by score descending
    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Take top N
    let selected: Vec<ScoredUtxo> = scored.into_iter().take(max_inputs).collect();

    let total_value = selected
        .iter()
        .map(|s| s.utxo.amount)
        .fold(Amount::ZERO, |acc, a| acc + a);

    tracing::info!(
        strategy = %strategy,
        selected_count = selected.len(),
        total_value_sats = total_value.to_sat(),
        "Coin selection complete"
    );

    for (i, s) in selected.iter().enumerate() {
        tracing::debug!(
            index = i,
            outpoint = %s.utxo.outpoint,
            amount_sats = s.utxo.amount.to_sat(),
            score = s.score,
            script_type = %s.script_type,
            "Selected UTXO"
        );
    }

    Ok(SelectionResult {
        selected,
        total_value,
        strategy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{OutPoint, ScriptBuf, Txid};
    use std::str::FromStr;

    fn make_utxo(amount_sats: u64, confirmations: u32, script_hex: &str) -> WalletUtxo {
        WalletUtxo {
            outpoint: OutPoint::new(
                Txid::from_str(
                    "0000000000000000000000000000000000000000000000000000000000000001",
                )
                .unwrap(),
                0,
            ),
            txid: Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000001",
            )
            .unwrap(),
            vout: 0,
            amount: Amount::from_sat(amount_sats),
            confirmations,
            script_pubkey: ScriptBuf::from_hex(script_hex).unwrap_or_default(),
            address: String::new(),
            spendable: true,
            solvable: true,
        }
    }

    #[test]
    fn test_select_inputs_returns_max_inputs() {
        // P2WPKH script: OP_0 <20-byte-hash>
        let p2wpkh = "0014".to_string() + &"aa".repeat(20);
        let candidates = vec![
            make_utxo(100_000, 10, &p2wpkh),
            make_utxo(200_000, 20, &p2wpkh),
            make_utxo(300_000, 30, &p2wpkh),
        ];

        let result = select_inputs(
            &candidates,
            Amount::from_sat(50_000),
            Strategy::Balanced,
            2,
            1.0,
        )
        .unwrap();

        assert_eq!(result.selected.len(), 2);
        assert_eq!(result.strategy, Strategy::Balanced);
    }

    #[test]
    fn test_select_inputs_filters_unspendable() {
        let p2wpkh = "0014".to_string() + &"aa".repeat(20);
        let mut utxo = make_utxo(100_000, 10, &p2wpkh);
        utxo.spendable = false;

        let result = select_inputs(
            &[utxo],
            Amount::from_sat(50_000),
            Strategy::Balanced,
            1,
            1.0,
        )
        .unwrap();

        assert!(result.selected.is_empty());
    }

    #[test]
    fn test_strategy_from_str() {
        assert_eq!("balanced".parse::<Strategy>().unwrap(), Strategy::Balanced);
        assert_eq!(
            "privacy-max".parse::<Strategy>().unwrap(),
            Strategy::PrivacyMax
        );
        assert_eq!("fee-min".parse::<Strategy>().unwrap(), Strategy::FeeMin);
        assert_eq!(
            "consolidate".parse::<Strategy>().unwrap(),
            Strategy::Consolidate
        );
        assert!("invalid".parse::<Strategy>().is_err());
    }
}
