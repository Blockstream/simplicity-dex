use clap::Parser;
use global_utils::logger::init_logger;
use nostr::prelude::*;
use nostr_options_cli::cli_processor::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    let _logger_guard = init_logger();

    let cli = Cli::parse();
    cli.process()?;

    Ok(())
}
