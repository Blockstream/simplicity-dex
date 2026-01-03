#![warn(clippy::all, clippy::pedantic)]

mod cli;
mod config;
mod error;
mod wallet;

use crate::cli::Cli;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    Cli::parse().run().await?;

    Ok(())
}
