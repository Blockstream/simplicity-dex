#![warn(clippy::all, clippy::pedantic)]

use clap::Parser;
use cli_client::cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();

    cli.run().await?;

    Ok(())
}
