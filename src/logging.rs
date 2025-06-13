//! Logging-Initialisierung für die App
use anyhow::Result;
use tracing_appender::non_blocking;
use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;

pub fn setup_logger() -> Result<()> {
    let file_appender = rolling::daily("logs", "mac-updater.log");
    let (non_blocking, _guard) = non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(non_blocking)
        .init();
    Ok(())
}
