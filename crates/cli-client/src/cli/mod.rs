mod browse;
mod commands;
mod interactive;
mod option;
mod positions;
mod swap;
mod sync;
mod tx;
mod wallet;
mod tables;

use crate::error::Error;

use crate::config::{Config, default_config_path};
use crate::wallet::Wallet;

use clap::Parser;
use nostr::SecretKey;
use options_relay::{PublishingClient, ReadOnlyClient};
use std::path::PathBuf;

use signer::Signer;

pub use commands::{Command, OptionCommand, SwapCommand, SyncCommand, TxCommand, WalletCommand};
pub use interactive::{GRANTOR_TOKEN_TAG, OPTION_TOKEN_TAG, SWAP_COLLATERAL_TAG};

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

/// TBD: delete. Needed for testing.
/// keccak256(seed)
const DEFAULT_SEED: &str = "66a80b61b29ec044d14c4c8c613e762ba1fb8eeb0c454d1ee00ed6dedaa5b5c5";

impl Cli {
    #[must_use]
    pub fn load_config(&self) -> Config {
        Config::load_or_default(&self.config)
    }

    fn parse_seed(&self) -> Result<[u8; Signer::SEED_LEN], Error> {
        let seed_hex = self.seed.as_deref().unwrap_or(DEFAULT_SEED);

        let bytes = hex::decode(seed_hex)?;

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

    async fn get_read_only_client(&self, config: &Config) -> Result<ReadOnlyClient, Error> {
        let relay_config = config.relay.get_nostr_relay_config();

        let client = ReadOnlyClient::connect(relay_config).await?;

        Ok(client)
    }

    async fn get_publishing_client(&self, config: &Config) -> Result<PublishingClient, Error> {
        let seed = self.parse_seed()?;
        let relay_config = config.relay.get_nostr_relay_config();

        let secret_key =
            SecretKey::from_slice(&seed).map_err(|e| Error::Config(format!("Invalid seed for NOSTR key: {e}")))?;
        let keys = nostr::Keys::new(secret_key);

        let client = PublishingClient::connect(relay_config, keys).await?;

        Ok(client)
    }

    pub async fn run(&self) -> Result<(), Error> {
        let config = self.load_config();

        match &self.command {
            Command::Wallet { command } => self.run_wallet(config, command).await,
            Command::Tx { command } => self.run_tx(config, command).await,
            Command::Option { command } => Box::pin(self.run_option(config, command)).await,
            Command::Swap { command } => Box::pin(self.run_swap(config, command)).await,
            Command::Browse => self.run_browse(config).await,
            Command::Positions => self.run_positions(config).await,
            Command::Sync { command } => self.run_sync(config, command).await,
            Command::Config => {
                println!("{config:#?}");
                Ok(())
            }
        }
    }
}
