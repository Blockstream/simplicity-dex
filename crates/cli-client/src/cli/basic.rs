use crate::cli::{BasicCommand, Cli};
use crate::config::Config;
use crate::error::Error;

use simplicityhl::elements::pset::serialize::Serialize;
use simplicityhl::simplicity::hex::DisplayHex;

use simplicityhl_core::{LIQUID_TESTNET_GENESIS, finalize_p2pk_transaction};

impl Cli {
    pub(crate) async fn run_basic(&self, config: Config, command: &BasicCommand) -> Result<(), Error> {
        match command {
            BasicCommand::SplitNative { parts, fee, broadcast } => {
                let wallet = self.get_wallet(&config).await?;

                let native_asset = simplicityhl_core::LIQUID_TESTNET_BITCOIN_ASSET;
                let filter = coin_store::Filter::new()
                    .asset_id(native_asset)
                    .script_pubkey(wallet.signer().p2pk_address(config.address_params())?.script_pubkey());

                let results = wallet.store().query(&[filter]).await?;

                let entry = results
                    .into_iter()
                    .next()
                    .and_then(|r| match r {
                        coin_store::QueryResult::Found(entries) => entries.into_iter().next(),
                        coin_store::QueryResult::InsufficientValue(_) | coin_store::QueryResult::Empty => None,
                    })
                    .ok_or_else(|| Error::Config("No native UTXO found".to_string()))?;

                let outpoint = entry.outpoint();
                let txout = entry.txout().clone();

                let pst = contracts::sdk::split_native_any((*outpoint, txout.clone()), *parts, *fee)?;

                let tx = pst.extract_tx()?;
                let utxos = &[txout];

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

                if *broadcast {
                    cli_helper::explorer::broadcast_tx(&tx).await?;

                    wallet.store().mark_as_spent(*outpoint).await?;

                    let txid = tx.txid();
                    for (vout, output) in tx.output.iter().enumerate() {
                        if output.is_fee() {
                            continue;
                        }

                        #[allow(clippy::cast_possible_truncation)]
                        let new_outpoint = simplicityhl::elements::OutPoint::new(txid, vout as u32);

                        wallet.store().insert(new_outpoint, output.clone(), None).await?;
                    }

                    println!("Broadcasted: {txid}");
                } else {
                    println!("{}", tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
            BasicCommand::TransferNative { .. } => todo!(),
            BasicCommand::TransferAsset { .. } => todo!(),
            BasicCommand::IssueAsset { .. } => todo!(),
            BasicCommand::ReissueAsset { .. } => todo!(),
        }
    }
}
