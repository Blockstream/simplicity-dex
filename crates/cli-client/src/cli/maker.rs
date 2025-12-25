use crate::cli::{Cli, MakerCommand};
use crate::config::Config;
use crate::error::Error;

impl Cli {
    pub(crate) async fn run_maker(&self, config: Config, command: &MakerCommand) -> Result<(), Error> {
        match command {
            MakerCommand::Create => {}
            MakerCommand::Fund => {}
            MakerCommand::Exercise => {}
            MakerCommand::Cancel => {}
            MakerCommand::List => {}
        }

        Ok(())
    }
}
