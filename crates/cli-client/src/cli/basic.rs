use crate::cli::{BasicCommand, Cli};
use crate::config::Config;
use crate::error::Error;

use std::collections::HashMap;

use coin_store::UtxoStore;

use simplicityhl::elements::pset::serialize::Serialize;
use simplicityhl::simplicity::hex::DisplayHex;
use simplicityhl_core::{LIQUID_TESTNET_GENESIS, finalize_p2pk_transaction};

impl Cli {
    pub(crate) async fn run_basic(&self, config: Config, command: &BasicCommand) -> Result<(), Error> {
        match command {
            BasicCommand::SplitNative { parts, fee, broadcast } => {
                let wallet = self.get_wallet(&config).await?;

                let filter = coin_store::UtxoFilter::new()
                    .asset_id(*simplicityhl_core::LIQUID_TESTNET_BITCOIN_ASSET)
                    .script_pubkey(wallet.signer().p2pk_address(config.address_params())?.script_pubkey());

                let results: Vec<coin_store::UtxoQueryResult> =
                    <_ as UtxoStore>::query_utxos(wallet.store(), &[filter]).await?;

                let native_entry = results
                    .into_iter()
                    .next()
                    .and_then(|r| match r {
                        coin_store::UtxoQueryResult::Found(entries, _) => entries.into_iter().next(),
                        coin_store::UtxoQueryResult::InsufficientValue(_, _) | coin_store::UtxoQueryResult::Empty => {
                            None
                        }
                    })
                    .ok_or_else(|| Error::Config("No native UTXO found".to_string()))?;

                let fee_utxo = (*native_entry.outpoint(), native_entry.txout().clone());

                let pst = contracts::sdk::split_native_any(fee_utxo.clone(), *parts, *fee)?;

                let tx = pst.extract_tx()?;
                let utxos = &[fee_utxo.1];

                let signature =
                    wallet
                        .signer()
                        .sign_p2pk(&tx, utxos, 0, config.address_params(), *LIQUID_TESTNET_GENESIS)?;

                let tx = finalize_p2pk_transaction(
                    tx,
                    utxos,
                    &wallet.signer().public_key(),
                    &signature,
                    0,
                    config.address_params(),
                    *LIQUID_TESTNET_GENESIS,
                )?;

                match broadcast {
                    false => {
                        println!("{}", tx.serialize().to_lower_hex_string());
                    }
                    true => {
                        cli_helper::explorer::broadcast_tx(&tx).await?;

                        println!("Broadcasted: {}", tx.txid());

                        wallet.store().insert_transaction(&tx, HashMap::default()).await?;
                    }
                }
            }
            BasicCommand::TransferNative {
                to: _,
                amount: _,
                fee: _,
                broadcast: _,
            } => {
                todo!()
            }
            BasicCommand::TransferAsset {
                asset_id: _,
                to: _,
                amount: _,
                fee: _,
                broadcast: _,
            } => {
                todo!()
            }
            BasicCommand::IssueAsset {
                asset_id: _,
                amount: _issue_amount,
                fee: _,
                broadcast: _,
            } => {
                todo!()
            }
            BasicCommand::ReissueAsset {
                asset_id: _,
                amount: _reissue_amount,
                fee: _,
                broadcast: _,
            } => {
                todo!()
            }
        }

        Ok(())
    }
}
