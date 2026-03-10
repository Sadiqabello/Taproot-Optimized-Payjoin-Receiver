use bitcoin::Amount;
use bitcoincore_rpc::RpcApi;

use crate::coin_selection::{self, Strategy};
use crate::config::Config;
use crate::persistence;
use crate::rpc::{BitcoinRpc, WalletUtxo};
use crate::session::{SessionState, SessionStatus};

/// Which screen is currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    NewSession,
    Sessions,
    Help,
}

/// Input fields in the New Session form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputField {
    Amount,
    Strategy,
    MaxInputs,
    Expiry,
    Label,
}

/// A log entry displayed in the TUI.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

/// Main application state for the TUI.
pub struct App {
    pub config: Config,
    pub screen: Screen,
    pub should_quit: bool,

    // Connection state
    pub connected: bool,
    pub network: String,
    pub block_height: u64,
    pub wallet_name: Option<String>,

    // Wallet data
    pub utxos: Vec<WalletUtxo>,
    pub feerate: Option<f64>,
    pub sessions: Vec<SessionState>,

    // New session form
    pub active_field: InputField,
    pub editing: bool,
    pub input_amount: String,
    pub input_strategy: String,
    pub input_max_inputs: String,
    pub input_expiry: String,
    pub input_label: String,

    // Session state
    pub session_uri: Option<String>,
    pub session_status: String,
    pub selection_preview: Option<Vec<String>>,

    // Log
    pub logs: Vec<LogEntry>,

    // Tick counter for periodic refresh
    tick_count: u32,
}

impl App {
    pub fn new(config: Config) -> Self {
        let network = config.network.clone();
        let wallet_name = config.wallet.clone();

        App {
            config,
            screen: Screen::Dashboard,
            should_quit: false,

            connected: false,
            network,
            block_height: 0,
            wallet_name,

            utxos: Vec::new(),
            feerate: None,
            sessions: Vec::new(),

            active_field: InputField::Amount,
            editing: false,
            input_amount: String::new(),
            input_strategy: "balanced".to_string(),
            input_max_inputs: "1".to_string(),
            input_expiry: "60".to_string(),
            input_label: String::new(),

            session_uri: None,
            session_status: String::new(),
            selection_preview: None,

            logs: Vec::new(),
            tick_count: 0,
        }
    }

    /// Total wallet balance in sats.
    pub fn total_balance(&self) -> u64 {
        self.utxos.iter().map(|u| u.amount.to_sat()).sum()
    }

    /// Count of active sessions.
    pub fn active_session_count(&self) -> usize {
        self.sessions
            .iter()
            .filter(|s| matches!(s.status, SessionStatus::Pending | SessionStatus::ProposalSent))
            .count()
    }

    /// Add a log entry.
    pub fn log(&mut self, level: &str, message: &str) {
        let now = chrono_simple_now();
        self.logs.push(LogEntry {
            timestamp: now,
            level: level.to_string(),
            message: message.to_string(),
        });
        // Keep last 100 entries
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    /// Refresh wallet data from Bitcoin Core.
    pub fn refresh_data(&mut self) {
        match BitcoinRpc::new(&self.config) {
            Ok(rpc) => {
                // Blockchain info
                match rpc.client().get_blockchain_info() {
                    Ok(info) => {
                        self.connected = true;
                        self.block_height = info.blocks;
                        self.network = info.chain.to_string();
                        self.log("INFO", &format!(
                            "Connected to Bitcoin Core ({}), block {}",
                            self.network, self.block_height
                        ));
                    }
                    Err(e) => {
                        self.connected = false;
                        self.log("ERROR", &format!("RPC connection failed: {}", e));
                        return;
                    }
                }

                // UTXOs
                match rpc.list_unspent() {
                    Ok(utxos) => {
                        let count = utxos.len();
                        self.utxos = utxos;
                        self.log("INFO", &format!("Loaded {} UTXOs", count));
                    }
                    Err(e) => {
                        self.log("WARN", &format!("Failed to list UTXOs: {}", e));
                    }
                }

                // Fee rate
                match rpc.estimate_feerate(6) {
                    Ok(rate) => self.feerate = Some(rate),
                    Err(_) => self.feerate = Some(1.0),
                }
            }
            Err(e) => {
                self.connected = false;
                self.log("ERROR", &format!("Failed to connect: {}", e));
            }
        }

        self.refresh_sessions();
    }

    /// Refresh session list from disk.
    pub fn refresh_sessions(&mut self) {
        match persistence::load_sessions(&self.config) {
            Ok(sessions) => self.sessions = sessions,
            Err(e) => self.log("WARN", &format!("Failed to load sessions: {}", e)),
        }
    }

    /// Called on each tick (250ms).
    pub fn tick(&mut self) {
        self.tick_count += 1;
        // Refresh every 40 ticks (10 seconds)
        if self.tick_count % 40 == 0 {
            self.refresh_data();
        }
    }

    // ─── Form navigation ────────────────────────────────────────────

    pub fn next_field(&mut self) {
        self.active_field = match self.active_field {
            InputField::Amount => InputField::Strategy,
            InputField::Strategy => InputField::MaxInputs,
            InputField::MaxInputs => InputField::Expiry,
            InputField::Expiry => InputField::Label,
            InputField::Label => InputField::Amount,
        };
        self.editing = true;
    }

    pub fn prev_field(&mut self) {
        self.active_field = match self.active_field {
            InputField::Amount => InputField::Label,
            InputField::Strategy => InputField::Amount,
            InputField::MaxInputs => InputField::Strategy,
            InputField::Expiry => InputField::MaxInputs,
            InputField::Label => InputField::Expiry,
        };
        self.editing = true;
    }

    pub fn insert_char(&mut self, c: char) {
        let field = self.active_field_mut();
        field.push(c);
        self.update_preview();
    }

    pub fn delete_char(&mut self) {
        let field = self.active_field_mut();
        field.pop();
        self.update_preview();
    }

    fn active_field_mut(&mut self) -> &mut String {
        match self.active_field {
            InputField::Amount => &mut self.input_amount,
            InputField::Strategy => &mut self.input_strategy,
            InputField::MaxInputs => &mut self.input_max_inputs,
            InputField::Expiry => &mut self.input_expiry,
            InputField::Label => &mut self.input_label,
        }
    }

    /// Update the coin selection preview when amount changes.
    fn update_preview(&mut self) {
        let amount: u64 = match self.input_amount.parse() {
            Ok(a) if a > 0 => a,
            _ => {
                self.selection_preview = None;
                return;
            }
        };

        let strategy: Strategy = match self.input_strategy.parse() {
            Ok(s) => s,
            _ => Strategy::Balanced,
        };

        let max_inputs: usize = self.input_max_inputs.parse().unwrap_or(1);
        let feerate = self.feerate.unwrap_or(1.0);

        match coin_selection::select_inputs(
            &self.utxos,
            Amount::from_sat(amount),
            strategy,
            max_inputs,
            feerate,
        ) {
            Ok(result) => {
                let mut lines = vec![
                    format!(
                        "Coin Selection: {} input(s), {} sats total",
                        result.selected.len(),
                        result.total_value.to_sat()
                    ),
                    format!("Strategy: {} | Fee rate: {:.1} sat/vB", strategy, feerate),
                    String::new(),
                ];
                for (i, scored) in result.selected.iter().enumerate() {
                    lines.push(format!(
                        "  #{}: {} sats ({}) - score: {:.1}",
                        i + 1,
                        scored.utxo.amount.to_sat(),
                        scored.script_type,
                        scored.score,
                    ));
                }
                self.selection_preview = Some(lines);
            }
            Err(_) => {
                self.selection_preview = Some(vec!["No suitable UTXOs found".to_string()]);
            }
        }
    }

    // ─── Session management ─────────────────────────────────────────

    /// Start a new Payjoin receive session.
    pub async fn start_session(&mut self) {
        let amount: u64 = match self.input_amount.parse() {
            Ok(a) if a > 0 => a,
            _ => {
                self.log("ERROR", "Invalid amount. Enter a positive number of satoshis.");
                return;
            }
        };

        let strategy: Strategy = match self.input_strategy.parse() {
            Ok(s) => s,
            _ => {
                self.log("ERROR", "Invalid strategy. Use: balanced, privacy-max, fee-min, consolidate");
                return;
            }
        };

        let max_inputs: usize = self.input_max_inputs.parse().unwrap_or(1);
        let expiry: u64 = self.input_expiry.parse().unwrap_or(60);

        self.log("INFO", &format!(
            "Starting receive session: {} sats, strategy={}, max_inputs={}, expiry={}min",
            amount, strategy, max_inputs, expiry
        ));
        self.session_status = "Initializing...".to_string();

        // Build CLI args and run protocol
        let args = crate::cli::ReceiveArgs {
            amount,
            label: if self.input_label.is_empty() {
                None
            } else {
                Some(self.input_label.clone())
            },
            strategy,
            max_inputs,
            expiry,
            directory: None,
            ohttp_relay: None,
            global: crate::cli::GlobalArgs {
                rpc_host: None,
                rpc_port: None,
                rpc_user: None,
                rpc_pass: None,
                wallet: None,
                network: None,
                data_dir: None,
            },
        };

        self.session_status = "Connecting to Payjoin Directory...".to_string();
        self.log("INFO", "Fetching OHTTP keys from relay...");

        match crate::protocol::run_receive_session(&self.config, &args).await {
            Ok(()) => {
                self.session_status = "Proposal sent to sender!".to_string();
                self.log("INFO", "Payjoin proposal sent successfully");
                self.refresh_sessions();
            }
            Err(e) => {
                let msg = format!("{}", e);
                self.session_status = format!("Error: {}", msg);
                self.log("ERROR", &msg);
            }
        }
    }
}

/// Simple timestamp without pulling in chrono.
fn chrono_simple_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Convert to HH:MM:SS (UTC)
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
