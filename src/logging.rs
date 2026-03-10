use anyhow::Result;
use tracing_subscriber::EnvFilter;

/// Initialize structured logging with the given verbosity level.
///
/// Verbosity levels:
///   0 (default): warn
///   1 (-v):      info
///   2 (-vv):     debug
///   3+ (-vvv):   trace
pub fn init(verbosity: u8) -> Result<()> {
    let filter = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(filter));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .init();

    Ok(())
}
