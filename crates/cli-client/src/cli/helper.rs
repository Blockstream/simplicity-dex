use crate::cli::{Cli, HelperCommand};
use crate::config::Config;
use crate::error::Error;
use crate::wallet::Wallet;

impl Cli {
    pub(crate) async fn run_helper(&self, config: Config, command: &HelperCommand) -> Result<(), Error> {
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
                let wallet = self.get_wallet(&config).await?;

                wallet.signer().print_details()?;

                Ok(())
            }
            HelperCommand::Balance => {
                let wallet = self.get_wallet(&config).await?;

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
                let wallet = self.get_wallet(&config).await?;

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
                let wallet = self.get_wallet(&config).await?;

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
