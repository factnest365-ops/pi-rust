#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pi_cli::run_cli().await
}
