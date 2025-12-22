#![warn(clippy::all, clippy::pedantic)]

use clap::Parser;

use dex_cli::logger::init_logger;

use dex_cli::cli::Cli;

#[tokio::main]
#[tracing::instrument]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let _logger_guard = init_logger();

    Cli::parse().process().await?;

    Ok(())
}
