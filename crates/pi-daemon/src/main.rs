use anyhow::Result;
use clap::Parser;
use pi_daemon::{DaemonServer, CronContext};
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Parser, Debug)]
#[command(name = "taud", about = "Tau Background Intelligence Daemon (100% Pure Rust)", version)]
struct DaemonCli {
    /// Custom Unix domain socket path
    #[arg(short, long)]
    socket: Option<std::path::PathBuf>,

    /// Run in foreground
    #[arg(long, default_value = "true")]
    foreground: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = DaemonCli::parse();

    let socket_path = cli
        .socket
        .unwrap_or_else(DaemonServer::default_socket_path);

    eprintln!(
        "⚡ Starting Tau Background Daemon (taud) on Unix socket: {}",
        socket_path.display()
    );

    let server = match DaemonServer::open_default() {
        Ok(mut s) => {
            s.socket_path = socket_path;
            Arc::new(s)
        }
        Err(e) => {
            eprintln!("Failed to open Tau vault for daemon: {}", e);
            std::process::exit(1);
        }
    };

    let (shutdown_tx, _shutdown_rx) = tokio::sync::broadcast::channel(1);
    let (cron_notify_tx, _cron_notify_rx) = watch::channel(pi_daemon::cron::CronNotification::LoopStopped);

    // Capture Ctrl+C / SIGINT
    let tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        eprintln!("\n🛑 Received shutdown signal. Gracefully shutting down taud...");
        let _ = tx_clone.send(());
    });

    let cron_ctx = Arc::new(CronContext::default());
    let cron_handle = {
        let ctx = cron_ctx.clone();
        let tx = shutdown_tx.clone();
        tokio::spawn(async move {
            ctx.run_cron_loop(tx.subscribe(), cron_notify_tx).await;
        })
    };

    let server_handle = {
        let s = server.clone();
        let shutdown = shutdown_tx.subscribe();
        tokio::spawn(async move {
            let _ = s.run_server(shutdown).await;
        })
    };

    let _ = server_handle.await;
    let _ = cron_handle.await;

    eprintln!("✔ taud shut down cleanly.");
    Ok(())
}
