use anyhow::{Context, Result};
use bitcoin::{Address, Amount, Network, OutPoint, ScriptBuf, Txid};
use bitcoincore_rpc::json::{AddressType, ListUnspentResultEntry};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::{Deserialize, Serialize};

use crate::config::Config;

/// Wrapper around Bitcoin Core RPC client with convenience methods.
pub struct BitcoinRpc {
    client: Client,
    network: Network,
}

/// A wallet UTXO with all fields needed for coin selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletUtxo {
    pub outpoint: OutPoint,
    pub txid: Txid,
    pub vout: u32,
    pub amount: Amount,
    pub confirmations: u32,
    pub script_pubkey: ScriptBuf,
    pub address: String,
    pub spendable: bool,
    pub solvable: bool,
}

impl From<ListUnspentResultEntry> for WalletUtxo {
    fn from(entry: ListUnspentResultEntry) -> Self {
        WalletUtxo {
            outpoint: OutPoint::new(entry.txid, entry.vout),
            txid: entry.txid,
            vout: entry.vout,
            amount: entry.amount,
            confirmations: entry.confirmations,
            script_pubkey: entry.script_pub_key.clone(),
            address: entry
                .address
                .map(|a| a.assume_checked().to_string())
                .unwrap_or_default(),
            spendable: entry.spendable,
            solvable: entry.solvable,
        }
    }
}

impl BitcoinRpc {
    /// Connect to Bitcoin Core.
    pub fn new(config: &Config) -> Result<Self> {
        let url = match &config.wallet {
            Some(wallet) => format!(
                "http://{}:{}/wallet/{}",
                config.rpc_host, config.rpc_port, wallet
            ),
            None => format!("http://{}:{}", config.rpc_host, config.rpc_port),
        };

        let auth = Auth::UserPass(config.rpc_user.clone(), config.rpc_pass.clone());
        let client =
            Client::new(&url, auth).context("Failed to connect to Bitcoin Core RPC")?;

        let network = parse_network(&config.network)?;

        Ok(BitcoinRpc { client, network })
    }

    /// Get the underlying RPC client.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get the configured network.
    pub fn network(&self) -> Network {
        self.network
    }

    /// Generate a new Taproot (P2TR / Bech32m) receiving address.
    pub fn get_new_address(&self) -> Result<Address> {
        let addr = self
            .client
            .get_new_address(None, Some(AddressType::Bech32m))
            .context("Failed to get new Bech32m address")?;
        let checked = addr.require_network(self.network)?;
        Ok(checked)
    }

    /// List all unspent transaction outputs in the wallet.
    pub fn list_unspent(&self) -> Result<Vec<WalletUtxo>> {
        let utxos = self
            .client
            .list_unspent(Some(1), None, None, None, None)
            .context("Failed to list unspent outputs")?;

        Ok(utxos.into_iter().map(WalletUtxo::from).collect())
    }

    /// List unspent outputs with a minimum confirmation count.
    pub fn list_unspent_min_conf(&self, min_conf: usize) -> Result<Vec<WalletUtxo>> {
        let utxos = self
            .client
            .list_unspent(Some(min_conf), None, None, None, None)
            .context("Failed to list unspent outputs")?;

        Ok(utxos.into_iter().map(WalletUtxo::from).collect())
    }

    /// Process (sign) a PSBT using the wallet's keys.
    pub fn wallet_process_psbt(&self, psbt: &str) -> Result<String> {
        let result = self
            .client
            .wallet_process_psbt(psbt, Some(true), None, None)
            .context("Failed to process PSBT")?;
        Ok(result.psbt)
    }

    /// Check if a script belongs to this wallet.
    pub fn is_mine(&self, script: &bitcoin::Script) -> Result<bool> {
        // Use getaddressinfo to check ownership
        match Address::from_script(script, self.network) {
            Ok(addr) => {
                let info = self.client.get_address_info(&addr)?;
                Ok(info.is_mine.unwrap_or(false))
            }
            Err(_) => Ok(false),
        }
    }

    /// Get the current estimated fee rate in sat/vB.
    pub fn estimate_feerate(&self, conf_target: u16) -> Result<f64> {
        let estimate = self
            .client
            .estimate_smart_fee(conf_target, None)
            .context("Failed to estimate fee")?;

        match estimate.fee_rate {
            Some(fee_rate) => {
                // Convert BTC/kvB to sat/vB
                let sat_per_vb = fee_rate.to_sat() as f64 / 1000.0;
                Ok(sat_per_vb.max(1.0)) // minimum 1 sat/vB
            }
            None => {
                tracing::warn!("Fee estimation unavailable, using default 1 sat/vB");
                Ok(1.0)
            }
        }
    }

    /// Verify connection to Bitcoin Core.
    pub fn verify_connection(&self) -> Result<()> {
        let info = self
            .client
            .get_blockchain_info()
            .context("Failed to connect to Bitcoin Core")?;
        tracing::info!(
            "Connected to Bitcoin Core: chain={}, blocks={}",
            info.chain,
            info.blocks
        );
        Ok(())
    }
}

fn parse_network(network: &str) -> Result<Network> {
    match network {
        "bitcoin" | "mainnet" => Ok(Network::Bitcoin),
        "signet" => Ok(Network::Signet),
        "testnet" => Ok(Network::Testnet),
        "regtest" => Ok(Network::Regtest),
        _ => anyhow::bail!("Unknown network: {}. Use: bitcoin, signet, testnet, regtest", network),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_network() {
        assert_eq!(parse_network("bitcoin").unwrap(), Network::Bitcoin);
        assert_eq!(parse_network("mainnet").unwrap(), Network::Bitcoin);
        assert_eq!(parse_network("signet").unwrap(), Network::Signet);
        assert_eq!(parse_network("testnet").unwrap(), Network::Testnet);
        assert_eq!(parse_network("regtest").unwrap(), Network::Regtest);
        assert!(parse_network("invalid").is_err());
    }
}
