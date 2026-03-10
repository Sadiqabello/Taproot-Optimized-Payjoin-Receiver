use clap::{Parser, Subcommand};

use crate::coin_selection::Strategy;

#[derive(Parser)]
#[command(
    name = "pj-receive",
    about = "Taproot-optimized Payjoin v2 receiver",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Increase logging verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize receiver configuration and wallet connection
    Init(InitArgs),
    /// Start a new Payjoin receive session
    Receive(ReceiveArgs),
    /// Check status of active sessions
    Status(GlobalArgs),
    /// View completed Payjoin transactions
    History(GlobalArgs),
    /// View or modify configuration
    Config(ConfigArgs),
}

#[derive(Parser, Clone)]
pub struct GlobalArgs {
    /// Bitcoin Core RPC host
    #[arg(long, default_value = "127.0.0.1")]
    pub rpc_host: Option<String>,

    /// Bitcoin Core RPC port
    #[arg(long)]
    pub rpc_port: Option<u16>,

    /// Bitcoin Core RPC username
    #[arg(long)]
    pub rpc_user: Option<String>,

    /// Bitcoin Core RPC password
    #[arg(long)]
    pub rpc_pass: Option<String>,

    /// Bitcoin Core wallet name
    #[arg(long)]
    pub wallet: Option<String>,

    /// Network: bitcoin, signet, testnet, regtest
    #[arg(long)]
    pub network: Option<String>,

    /// Data directory for session persistence
    #[arg(long)]
    pub data_dir: Option<String>,
}

#[derive(Parser)]
pub struct InitArgs {
    /// Bitcoin Core RPC host
    #[arg(long, default_value = "127.0.0.1")]
    pub rpc_host: String,

    /// Bitcoin Core RPC port (default: 38332 for signet)
    #[arg(long, default_value_t = 38332)]
    pub rpc_port: u16,

    /// Bitcoin Core RPC username
    #[arg(long)]
    pub rpc_user: String,

    /// Bitcoin Core RPC password
    #[arg(long)]
    pub rpc_pass: String,

    /// Bitcoin Core wallet name
    #[arg(long)]
    pub wallet: Option<String>,

    /// Network: bitcoin, signet, testnet, regtest
    #[arg(long, default_value = "signet")]
    pub network: String,

    /// Data directory for session persistence
    #[arg(long)]
    pub data_dir: Option<String>,
}

#[derive(Parser)]
pub struct ReceiveArgs {
    /// Expected payment amount in satoshis
    #[arg(long)]
    pub amount: u64,

    /// Human-readable label for BIP 21 URI
    #[arg(long)]
    pub label: Option<String>,

    /// Coin selection strategy: balanced, privacy-max, fee-min, consolidate
    #[arg(long, default_value = "balanced")]
    pub strategy: Strategy,

    /// Maximum receiver inputs to contribute
    #[arg(long, default_value_t = 1)]
    pub max_inputs: usize,

    /// Session expiry timeout in minutes
    #[arg(long, default_value_t = 60)]
    pub expiry: u64,

    /// Payjoin Directory URL
    #[arg(long)]
    pub directory: Option<String>,

    /// OHTTP relay URL
    #[arg(long)]
    pub ohttp_relay: Option<String>,

    #[command(flatten)]
    pub global: GlobalArgs,
}

#[derive(Parser)]
pub struct ConfigArgs {
    /// Show current configuration
    #[arg(long)]
    pub show: bool,

    #[command(flatten)]
    pub global: GlobalArgs,
}
