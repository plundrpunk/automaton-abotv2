use tokio::sync::mpsc;
use tracing::info;

/// Set up OS signal handlers for graceful shutdown.
/// Returns a sender that will fire when SIGINT or SIGTERM is received.
pub fn setup_signal_handler() -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel(1);

    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler");
            let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT handler");

            tokio::select! {
                _ = sigterm.recv() => info!("Received SIGTERM"),
                _ = sigint.recv() => info!("Received SIGINT"),
            }
        }

        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c().await.expect("Ctrl+C handler");
            info!("Received Ctrl+C");
        }

        let _ = tx.send(()).await;
    });

    rx
}
