use crate::cli::{Cli, TakerCommand};
use crate::config::Config;
use crate::error::Error;

impl Cli {
    pub(crate) async fn run_taker(&self, config: Config, command: &TakerCommand) -> Result<(), Error> {
        match command {
            TakerCommand::Browse => {}
            TakerCommand::Take => {}
            TakerCommand::Claim => {}
            TakerCommand::List => {}
        }

        Ok(())
    }
}
