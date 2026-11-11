use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    pi_metafuge::cli::run().await
}
