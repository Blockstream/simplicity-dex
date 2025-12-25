mod basic;
mod commands;
mod common;
mod helper;
mod maker;
mod taker;

use std::path::PathBuf;

use clap::Parser;

use crate::config::{Config, default_config_path};
use crate::error::Error;
pub use commands::{BasicCommand, Command, HelperCommand, MakerCommand, TakerCommand};
use signer::Signer;

use crate::wallet::Wallet;

#[derive(Debug, Parser)]
#[command(name = "simplicity-dex")]
#[command(about = "CLI for Simplicity Options trading on Liquid")]
pub struct Cli {
    #[arg(short, long, default_value_os_t = default_config_path(), env = "SIMPLICITY_DEX_CONFIG")]
    pub config: PathBuf,

    #[arg(short, long, env = "SIMPLICITY_DEX_SEED")]
    pub seed: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    #[must_use]
    pub fn load_config(&self) -> Config {
        Config::load_or_default(&self.config)
    }

    fn parse_seed(&self) -> Result<[u8; Signer::SEED_LEN], Error> {
        let seed_hex = self
            .seed
            .as_ref()
            .ok_or_else(|| Error::Config("Seed is required. Use --seed or SIMPLICITY_DEX_SEED".to_string()))?;

        let bytes = hex::decode(seed_hex).map_err(|e| Error::Config(format!("Invalid seed hex: {e}")))?;

        bytes.try_into().map_err(|_| {
            Error::Config(format!(
                "Seed must be exactly {} bytes ({} hex chars)",
                Signer::SEED_LEN,
                Signer::SEED_LEN * 2
            ))
        })
    }

    async fn get_wallet(&self, config: &Config) -> Result<Wallet, Error> {
        let seed = self.parse_seed()?;
        let db_path = config.database_path();

        Wallet::open(&seed, &db_path, config.address_params()).await
    }

    pub async fn run(&self) -> Result<(), Error> {
        let config = self.load_config();

        match &self.command {
            Command::Basic { command } => self.run_basic(config, command).await,
            Command::Maker { command } => self.run_maker(config, command).await,
            Command::Taker { command } => self.run_taker(config, command).await,
            Command::Helper { command } => self.run_helper(config, command).await,
            Command::Config => {
                println!("{config:#?}");
                Ok(())
            }
        }
    }
}
