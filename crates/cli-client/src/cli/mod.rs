mod commands;

use std::path::PathBuf;

use clap::Parser;

use crate::config::{Config, default_config_path};
use crate::error::Error;
use crate::wallet::Wallet;

pub use commands::{Command, HelperCommand, MakerCommand, TakerCommand};

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

    fn parse_seed(&self) -> Result<[u8; 32], Error> {
        let seed_hex = self
            .seed
            .as_ref()
            .ok_or_else(|| Error::Config("Seed is required. Use --seed or SIMPLICITY_DEX_SEED".to_string()))?;

        let bytes = hex::decode(seed_hex).map_err(|e| Error::Config(format!("Invalid seed hex: {e}")))?;

        bytes
            .try_into()
            .map_err(|_| Error::Config("Seed must be exactly 32 bytes (64 hex chars)".to_string()))
    }

    pub async fn run(&self) -> Result<(), Error> {
        let config = self.load_config();

        match &self.command {
            Command::Basic { command: _ } => todo!(),
            Command::Maker { command: _ } => todo!(),
            Command::Taker { command: _ } => todo!(),
            Command::Helper { command } => self.run_helper(config, command).await,
            Command::Config => {
                println!("{config:#?}");
                Ok(())
            }
        }
    }

    async fn run_helper(&self, config: Config, command: &HelperCommand) -> Result<(), Error> {
        match command {
            HelperCommand::Init => {
                let seed = self.parse_seed()?;
                let db_path = config.database_path();

                std::fs::create_dir_all(&config.storage.data_dir)?;
                Wallet::create(&seed, &db_path, config.address_params()).await?;

                println!("Wallet initialized at {}", db_path.display());
                Ok(())
            }
            HelperCommand::Address => {
                let seed = self.parse_seed()?;
                let db_path = config.database_path();
                let wallet = Wallet::open(&seed, &db_path, config.address_params()).await?;

                wallet.signer().print_details()?;

                Ok(())
            }
            HelperCommand::Balance => {
                let seed = self.parse_seed()?;
                let db_path = config.database_path();
                let wallet = Wallet::open(&seed, &db_path, config.address_params()).await?;

                let filter = coin_store::Filter::new()
                    .script_pubkey(wallet.signer().p2pk_address(config.address_params())?.script_pubkey());
                let results = wallet.store().query(&[filter]).await?;

                let mut balances: std::collections::HashMap<simplicityhl::elements::AssetId, u64> =
                    std::collections::HashMap::new();

                if let Some(coin_store::QueryResult::Found(entries)) = results.into_iter().next() {
                    for entry in entries {
                        let (asset, value) = match entry {
                            coin_store::UtxoEntry::Confidential { secrets, .. } => (secrets.asset, secrets.value),
                            coin_store::UtxoEntry::Explicit { txout, .. } => {
                                let asset = txout.asset.explicit().unwrap();
                                let value = txout.value.explicit().unwrap();
                                (asset, value)
                            }
                        };
                        *balances.entry(asset).or_insert(0) += value;
                    }
                }

                if balances.is_empty() {
                    println!("No UTXOs found");
                } else {
                    for (asset, value) in &balances {
                        println!("{asset}: {value}");
                    }
                }
                Ok(())
            }
            HelperCommand::Utxos => {
                let seed = self.parse_seed()?;
                let db_path = config.database_path();
                let wallet = Wallet::open(&seed, &db_path, config.address_params()).await?;

                let filter = coin_store::Filter::new();
                let results = wallet.store().query(&[filter]).await?;

                if let Some(coin_store::QueryResult::Found(entries)) = results.into_iter().next() {
                    for entry in &entries {
                        let outpoint = entry.outpoint();
                        let (asset, value) = match entry {
                            coin_store::UtxoEntry::Confidential { secrets, .. } => (secrets.asset, secrets.value),
                            coin_store::UtxoEntry::Explicit { txout, .. } => {
                                let asset = txout.asset.explicit().unwrap();
                                let value = txout.value.explicit().unwrap();
                                (asset, value)
                            }
                        };
                        println!("{outpoint} | {asset} | {value}");
                    }
                    println!("Total: {} UTXOs", entries.len());
                } else {
                    println!("No UTXOs found");
                }
                Ok(())
            }
            HelperCommand::Import { outpoint, blinding_key } => {
                let seed = self.parse_seed()?;
                let db_path = config.database_path();
                let wallet = Wallet::open(&seed, &db_path, config.address_params()).await?;

                let txout = cli_helper::explorer::fetch_utxo(*outpoint).await?;

                let blinder = match blinding_key {
                    Some(key_hex) => {
                        let bytes: [u8; 32] = hex::decode(key_hex)
                            .map_err(|e| Error::Config(format!("Invalid blinding key hex: {e}")))?
                            .try_into()
                            .map_err(|_| Error::Config("Blinding key must be 32 bytes".to_string()))?;
                        Some(bytes)
                    }
                    None => None,
                };

                wallet.store().insert(*outpoint, txout, blinder).await?;

                println!("Imported {outpoint}");
                Ok(())
            }
        }
    }
}
