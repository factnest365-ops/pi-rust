use anyhow::Result;
use clap::Parser;
use pi_daemon::DaemonServer;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "taud", about = "Tau Background Intelligence Daemon (100% Pure Rust)", version)]
struct DaemonCli {
    /// Custom Unix domain socket path
    #[arg(short, long)]
    socket: Option<PathBuf>,

    /// Run in foreground
    #[arg(long, default_value = "true")]
    foreground: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = DaemonCli::parse();

    let socket_path = cli.socket.unwrap_or_else(DaemonServer::default_socket_path);

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

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    // Capture Ctrl+C / SIGINT
    let tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        eprintln!("\n🛑 Received shutdown signal. Gracefully shutting down taud...");
        let _ = tx_clone.send(());
    });

    server.run_server(shutdown_rx).await?;

    eprintln!("✔ taud shut down cleanly.");
    Ok(())
}
