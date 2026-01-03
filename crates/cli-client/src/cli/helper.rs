use crate::cli::{Cli, HelperCommand};
use crate::config::Config;
use crate::error::Error;
use crate::wallet::Wallet;

use coin_store::UtxoStore;
use simplicityhl::elements::bitcoin::secp256k1;

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

                let filter = coin_store::UtxoFilter::new()
                    .script_pubkey(wallet.signer().p2pk_address(config.address_params())?.script_pubkey());
                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[filter]).await?;

                let mut balances: std::collections::HashMap<simplicityhl::elements::AssetId, u64> =
                    std::collections::HashMap::new();

                if let Some(coin_store::UtxoQueryResult::Found(entries, _)) = results.into_iter().next() {
                    for entry in entries {
                        *balances.entry(entry.asset()).or_insert(0) += entry.value();
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

                let filter = coin_store::UtxoFilter::new();
                let results = wallet.store().query_utxos(&[filter]).await?;

                if let Some(coin_store::UtxoQueryResult::Found(entries, _)) = results.into_iter().next() {
                    for entry in &entries {
                        println!("{} | {} | {}", entry.outpoint(), entry.asset(), entry.value());
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
                        let bytes: [u8; secp256k1::constants::SECRET_KEY_SIZE] = hex::decode(key_hex)
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
