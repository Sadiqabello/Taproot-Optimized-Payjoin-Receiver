use anyhow::{Context, Result};

use crate::config::Config;
use crate::session::SessionState;

const SESSIONS_FILE: &str = "sessions.json";

/// Load all sessions from disk.
pub fn load_sessions(config: &Config) -> Result<Vec<SessionState>> {
    let path = config.data_dir.join(SESSIONS_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let json = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read sessions from {}", path.display()))?;

    let sessions: Vec<SessionState> = serde_json::from_str(&json)
        .with_context(|| "Failed to parse sessions file")?;

    Ok(sessions)
}

/// Save all sessions to disk.
pub fn save_sessions(config: &Config, sessions: &[SessionState]) -> Result<()> {
    let path = config.data_dir.join(SESSIONS_FILE);
    let json = serde_json::to_string_pretty(sessions)?;
    std::fs::write(&path, json)
        .with_context(|| format!("Failed to write sessions to {}", path.display()))?;
    Ok(())
}

/// Add or update a session in the sessions list.
pub fn upsert_session(config: &Config, session: SessionState) -> Result<()> {
    let mut sessions = load_sessions(config)?;

    if let Some(existing) = sessions.iter_mut().find(|s| s.id == session.id) {
        *existing = session;
    } else {
        sessions.push(session);
    }

    save_sessions(config, &sessions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionStatus;
    use std::path::PathBuf;
    use std::time::Duration;
    use url::Url;

    fn test_config(dir: &std::path::Path) -> Config {
        Config {
            rpc_host: "127.0.0.1".to_string(),
            rpc_port: 38332,
            rpc_user: "test".to_string(),
            rpc_pass: "test".to_string(),
            wallet: None,
            network: "regtest".to_string(),
            data_dir: PathBuf::from(dir),
            directory_url: Url::parse("https://payjo.in").unwrap(),
            ohttp_relay_url: Url::parse("https://pj.bobspacebkk.com").unwrap(),
            strategy: crate::coin_selection::Strategy::Balanced,
            max_inputs: 1,
            expiry: Duration::from_secs(3600),
            poll_interval: Duration::from_secs(5),
        }
    }

    #[test]
    fn test_load_empty() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());
        let sessions = load_sessions(&config).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let session = SessionState {
            id: "test-session-1".to_string(),
            pj_uri: "bitcoin:tb1qtest?pj=https://payjo.in/xyz".to_string(),
            amount_sats: 100_000,
            status: SessionStatus::Pending,
            created_at: 1700000000,
            expires_at: 1700003600,
            label: Some("Test payment".to_string()),
            strategy: "balanced".to_string(),
        };

        upsert_session(&config, session.clone()).unwrap();

        let loaded = load_sessions(&config).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "test-session-1");
        assert_eq!(loaded[0].amount_sats, 100_000);
    }

    #[test]
    fn test_upsert_updates_existing() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let session = SessionState {
            id: "test-session-1".to_string(),
            pj_uri: "bitcoin:tb1qtest?pj=https://payjo.in/xyz".to_string(),
            amount_sats: 100_000,
            status: SessionStatus::Pending,
            created_at: 1700000000,
            expires_at: 1700003600,
            label: None,
            strategy: "balanced".to_string(),
        };

        upsert_session(&config, session).unwrap();

        let updated = SessionState {
            id: "test-session-1".to_string(),
            pj_uri: "bitcoin:tb1qtest?pj=https://payjo.in/xyz".to_string(),
            amount_sats: 100_000,
            status: SessionStatus::Completed,
            created_at: 1700000000,
            expires_at: 1700003600,
            label: None,
            strategy: "balanced".to_string(),
        };

        upsert_session(&config, updated).unwrap();

        let loaded = load_sessions(&config).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].status, SessionStatus::Completed);
    }
}
