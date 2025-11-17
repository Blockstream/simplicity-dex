use clap::Parser;

use global_utils::logger::init_logger;

use simplicity_dex::cli::Cli;

#[tokio::main]
#[tracing::instrument]
async fn main() -> anyhow::Result<()> {
    let _logger_guard = init_logger();

    Cli::parse().process().await?;

    Ok(())
}
