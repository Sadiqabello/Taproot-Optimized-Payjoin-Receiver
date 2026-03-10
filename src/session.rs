use std::collections::HashSet;

use anyhow::Result;
use bitcoin::OutPoint;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::persistence;

/// Session state for a Payjoin receive operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Unique session identifier.
    pub id: String,
    /// BIP 21 URI for the sender.
    pub pj_uri: String,
    /// Expected payment amount in satoshis.
    pub amount_sats: u64,
    /// Session status.
    pub status: SessionStatus,
    /// Timestamp when the session was created (Unix epoch seconds).
    pub created_at: u64,
    /// Timestamp when the session expires.
    pub expires_at: u64,
    /// Label for this session.
    pub label: Option<String>,
    /// Strategy used for coin selection.
    pub strategy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    /// Waiting for sender's Original PSBT.
    Pending,
    /// Proposal sent back to sender, waiting for broadcast.
    ProposalSent,
    /// Transaction confirmed on-chain.
    Completed,
    /// Session expired without completion.
    Expired,
    /// Session failed with an error.
    Failed(String),
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionStatus::Pending => write!(f, "Pending"),
            SessionStatus::ProposalSent => write!(f, "Proposal Sent"),
            SessionStatus::Completed => write!(f, "Completed"),
            SessionStatus::Expired => write!(f, "Expired"),
            SessionStatus::Failed(e) => write!(f, "Failed: {}", e),
        }
    }
}

/// Track previously seen input outpoints for replay protection.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SeenInputs {
    outpoints: HashSet<String>,
}

impl SeenInputs {
    /// Load seen inputs from disk.
    pub fn load(config: &Config) -> Result<Self> {
        let path = config.data_dir.join("seen_inputs.json");
        if path.exists() {
            let json = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&json)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Save seen inputs to disk.
    pub fn save(&self, config: &Config) -> Result<()> {
        let path = config.data_dir.join("seen_inputs.json");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Check if an outpoint has been seen before.
    pub fn contains(&self, outpoint: &OutPoint) -> bool {
        self.outpoints.contains(&outpoint.to_string())
    }

    /// Record an outpoint as seen.
    pub fn insert(&mut self, outpoint: &OutPoint) {
        self.outpoints.insert(outpoint.to_string());
    }
}

/// Display the status of all active sessions.
pub fn show_status(config: &Config) -> Result<()> {
    let sessions = persistence::load_sessions(config)?;

    let active: Vec<_> = sessions
        .iter()
        .filter(|s| matches!(s.status, SessionStatus::Pending | SessionStatus::ProposalSent))
        .collect();

    if active.is_empty() {
        println!("No active sessions.");
        return Ok(());
    }

    println!("Active Payjoin Sessions:");
    println!("{:-<70}", "");
    for session in active {
        println!(
            "  ID:       {}",
            &session.id[..16.min(session.id.len())]
        );
        println!("  Amount:   {} sats", session.amount_sats);
        println!("  Status:   {}", session.status);
        println!("  Strategy: {}", session.strategy);
        if let Some(ref label) = session.label {
            println!("  Label:    {}", label);
        }
        println!("  URI:      {}", session.pj_uri);
        println!("{:-<70}", "");
    }

    Ok(())
}

/// Display completed Payjoin transaction history.
pub fn show_history(config: &Config) -> Result<()> {
    let sessions = persistence::load_sessions(config)?;

    let completed: Vec<_> = sessions
        .iter()
        .filter(|s| {
            matches!(
                s.status,
                SessionStatus::Completed | SessionStatus::Expired | SessionStatus::Failed(_)
            )
        })
        .collect();

    if completed.is_empty() {
        println!("No completed sessions.");
        return Ok(());
    }

    println!("Payjoin History:");
    println!("{:-<70}", "");
    for session in completed {
        println!(
            "  ID:     {}",
            &session.id[..16.min(session.id.len())]
        );
        println!("  Amount: {} sats", session.amount_sats);
        println!("  Status: {}", session.status);
        println!("{:-<70}", "");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seen_inputs() {
        let mut seen = SeenInputs::default();
        let outpoint = OutPoint::null();
        assert!(!seen.contains(&outpoint));
        seen.insert(&outpoint);
        assert!(seen.contains(&outpoint));
    }

    #[test]
    fn test_session_status_display() {
        assert_eq!(SessionStatus::Pending.to_string(), "Pending");
        assert_eq!(SessionStatus::ProposalSent.to_string(), "Proposal Sent");
        assert_eq!(SessionStatus::Completed.to_string(), "Completed");
        assert_eq!(
            SessionStatus::Failed("test".to_string()).to_string(),
            "Failed: test"
        );
    }
}
