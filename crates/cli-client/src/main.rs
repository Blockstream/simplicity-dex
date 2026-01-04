#![warn(clippy::all, clippy::pedantic)]

mod cli;
mod config;
mod error;
mod explorer;
mod fee;
mod logging;
mod metadata;
mod signing;
mod sync;
mod wallet;

use crate::cli::Cli;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    logging::init();

    Box::pin(Cli::parse().run()).await?;

    Ok(())
}
