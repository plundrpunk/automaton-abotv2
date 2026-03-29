use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use abot_core::{AbotConfig, Runtime};

/// Agent Bot (ABot) - Autonomous agent runtime
#[derive(Parser, Debug)]
#[command(
    name = "abot",
    version,
    about = "Autonomous agent runtime with AMS integration",
    long_about = None
)]
struct Args {
    /// Path to configuration file
    #[arg(
        long,
        short,
        default_value = "abot.toml",
    )]
    config: PathBuf,

    /// Agent name (overrides config)
    #[arg(long, short = 'n')]
    agent_name: Option<String>,

    /// Agent ID (overrides config)
    #[arg(long, short = 'i')]
    agent_id: Option<String>,

    /// Log level
    #[arg(
        long,
        short = 'l',
        default_value = "info",
    )]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup tracing
    setup_tracing(&args.log_level)?;

    info!("Starting ABot agent runtime");
    info!(config = ?args.config, "Loading configuration");

    // Load configuration
    let mut config = AbotConfig::load(&args.config)?;

    // Override configuration with CLI arguments
    if let Some(name) = args.agent_name {
        config.agent.name = name.clone();
        info!(agent_name = %name, "Agent name overridden");
    }

    if let Some(id) = args.agent_id {
        config.agent.id = id.clone();
        info!(agent_id = %id, "Agent ID overridden");
    }

    info!(
        agent_name = %config.agent.name,
        agent_id = %config.agent.id,
        "Configuration loaded"
    );

    // Setup shutdown channel for graceful shutdown
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);

    // Spawn signal handler
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Ctrl+C received, initiating shutdown");
        let _ = shutdown_tx.send(()).await;
    });

    // Create runtime
    let mut runtime = Runtime::new(config, shutdown_rx)?;

    // Run the agent
    info!("Starting agent runtime loop");
    match runtime.run().await {
        Ok(_) => {
            info!("Agent runtime completed successfully");
            Ok(())
        }
        Err(e) => {
            error!(error = ?e, "Agent runtime failed");
            Err(e)
        }
    }
}

/// Initialize logging with tracing
fn setup_tracing(log_level: &str) -> Result<()> {
    let filter = EnvFilter::try_new(log_level)
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(std::io::stderr)
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true),
        )
        .with(filter)
        .init();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parsing() {
        let args = Args::try_parse_from(vec![
            "abot",
            "--config",
            "test.toml",
            "--agent-name",
            "test-agent",
        ]);

        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args.config.to_string_lossy(), "test.toml");
        assert_eq!(args.agent_name, Some("test-agent".to_string()));
    }

    #[test]
    fn test_default_log_level() {
        let args = Args::try_parse_from(vec!["abot"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args.log_level, "info");
    }

    #[test]
    fn test_custom_log_level() {
        let args = Args::try_parse_from(vec![
            "abot",
            "--log-level",
            "debug",
        ]);

        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args.log_level, "debug");
    }
}
