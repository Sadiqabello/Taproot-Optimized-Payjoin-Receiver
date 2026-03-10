use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::cli::{GlobalArgs, InitArgs};
use crate::coin_selection::Strategy;

/// Default Payjoin Directory (BIP 77 public directory)
const DEFAULT_DIRECTORY_URL: &str = "https://payjo.in";
/// Default OHTTP relay for IP privacy
const DEFAULT_OHTTP_RELAY_URL: &str = "https://pj.bobspacebkk.com";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub rpc_host: String,
    pub rpc_port: u16,
    pub rpc_user: String,
    pub rpc_pass: String,
    pub wallet: Option<String>,
    pub network: String,
    pub data_dir: PathBuf,
    pub directory_url: Url,
    pub ohttp_relay_url: Url,
    pub strategy: Strategy,
    pub max_inputs: usize,
    pub expiry: Duration,
    pub poll_interval: Duration,
}

impl Config {
    /// Create a new configuration from init arguments.
    pub fn initialize(args: InitArgs) -> Result<Self> {
        let data_dir = match args.data_dir {
            Some(dir) => PathBuf::from(dir),
            None => default_data_dir()?,
        };

        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data directory: {}", data_dir.display()))?;

        Ok(Config {
            rpc_host: args.rpc_host,
            rpc_port: args.rpc_port,
            rpc_user: args.rpc_user,
            rpc_pass: args.rpc_pass,
            wallet: args.wallet,
            network: args.network,
            data_dir,
            directory_url: Url::parse(DEFAULT_DIRECTORY_URL)?,
            ohttp_relay_url: Url::parse(DEFAULT_OHTTP_RELAY_URL)?,
            strategy: Strategy::Balanced,
            max_inputs: 1,
            expiry: Duration::from_secs(60 * 60), // 60 minutes
            poll_interval: Duration::from_secs(5),
        })
    }

    /// Save configuration to disk.
    pub fn save(&self) -> Result<()> {
        let path = self.data_dir.join("config.json");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)
            .with_context(|| format!("Failed to write config to {}", path.display()))?;
        Ok(())
    }

    /// Load configuration from the default data directory.
    pub fn load() -> Result<Self> {
        let data_dir = default_data_dir()?;
        let path = data_dir.join("config.json");
        let json = std::fs::read_to_string(&path)
            .with_context(|| format!("No config found at {}. Run `pj-receive init` first.", path.display()))?;
        let config: Config = serde_json::from_str(&json)?;
        Ok(config)
    }

    /// Load configuration and apply any CLI overrides.
    pub fn load_with_overrides(args: &GlobalArgs) -> Result<Self> {
        let mut config = Self::load()?;

        if let Some(ref host) = args.rpc_host {
            config.rpc_host = host.clone();
        }
        if let Some(port) = args.rpc_port {
            config.rpc_port = port;
        }
        if let Some(ref user) = args.rpc_user {
            config.rpc_user = user.clone();
        }
        if let Some(ref pass) = args.rpc_pass {
            config.rpc_pass = pass.clone();
        }
        if let Some(ref wallet) = args.wallet {
            config.wallet = Some(wallet.clone());
        }
        if let Some(ref network) = args.network {
            config.network = network.clone();
        }
        if let Some(ref dir) = args.data_dir {
            config.data_dir = PathBuf::from(dir);
        }

        Ok(config)
    }

    /// Build the Bitcoin Core RPC URL.
    pub fn rpc_url(&self) -> String {
        format!("http://{}:{}", self.rpc_host, self.rpc_port)
    }
}

fn default_data_dir() -> Result<PathBuf> {
    let home = dirs_path()?;
    Ok(home.join(".pj-receive"))
}

fn dirs_path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .context("HOME not set")
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .context("HOME not set")
    }
}
