use global_utils::logger::init_logger;
use nostr::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let _logger_guard = init_logger();

    Ok(())
}
