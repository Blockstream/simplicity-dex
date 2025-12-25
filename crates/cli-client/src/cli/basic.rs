use crate::cli::common::Broadcaster;
use crate::cli::{BasicCommand, Cli};
use crate::config::Config;
use crate::error::Error;
use coin_store::asset_entropy_store::entry::QueryResult;
use coin_store::{AssetEntropyStore, UtxoStore};
use simplicityhl::elements::hashes::{Hash, sha256};
use simplicityhl::elements::{AssetId, ContractHash};
use simplicityhl_core::{
    LIQUID_TESTNET_GENESIS, PUBLIC_SECRET_BLINDER_KEY, derive_public_blinder_key, finalize_p2pk_transaction,
};
use std::str::FromStr;

impl Cli {
    pub(crate) async fn run_basic(&self, config: Config, command: &BasicCommand) -> Result<(), Error> {
        match command {
            BasicCommand::SplitNative { parts, fee, broadcast } => {
                let liquid_genesis = *LIQUID_TESTNET_GENESIS;
                let wallet = self.get_wallet(&config).await?;

                let native_asset = *simplicityhl_core::LIQUID_TESTNET_BITCOIN_ASSET;
                let filter = coin_store::UtxoFilter::new()
                    .asset_id(native_asset)
                    .script_pubkey(wallet.signer().p2pk_address(config.address_params())?.script_pubkey());

                let results: Vec<coin_store::UtxoQueryResult> =
                    <_ as UtxoStore>::query_utxos(wallet.store(), &[filter]).await?;

                let native_entry = results
                    .into_iter()
                    .next()
                    .and_then(|r| match r {
                        coin_store::UtxoQueryResult::Found(entries) => entries.into_iter().next(),
                        coin_store::UtxoQueryResult::InsufficientValue(_) | coin_store::UtxoQueryResult::Empty => None,
                    })
                    .ok_or_else(|| Error::Config("No native UTXO found".to_string()))?;

                let fee_utxo = (*native_entry.outpoint(), native_entry.txout().clone());

                let pst = contracts::sdk::split_native_any(fee_utxo.clone(), *parts, *fee)?;

                let tx = pst.extract_tx()?;
                let utxos = &[fee_utxo.1];

                let signature = wallet
                    .signer()
                    .sign_p2pk(&tx, utxos, 0, config.address_params(), liquid_genesis)?;
                let tx = finalize_p2pk_transaction(
                    tx,
                    utxos,
                    &wallet.signer().public_key(),
                    &signature,
                    0,
                    config.address_params(),
                    liquid_genesis,
                )?;

                Broadcaster::from(*broadcast).broadcast_tx(&tx).await?;

                {
                    let spent_outpoints = &[fee_utxo.0];
                    for outpoint in spent_outpoints {
                        wallet.store().mark_as_spent(*outpoint).await?;
                    }
                    let txid = tx.txid();

                    for (vout, output) in tx.output.iter().enumerate() {
                        if output.is_fee() {
                            continue;
                        }

                        #[allow(clippy::cast_possible_truncation)]
                        let new_outpoint = simplicityhl::elements::OutPoint::new(txid, vout as u32);

                        <_ as UtxoStore>::insert(wallet.store(), new_outpoint, output.clone(), None).await?;
                    }
                }
            }
            BasicCommand::TransferNative {
                to,
                amount,
                fee,
                broadcast,
            } => {
                let liquid_genesis = *LIQUID_TESTNET_GENESIS;
                let wallet = self.get_wallet(&config).await?;

                let native_asset = *simplicityhl_core::LIQUID_TESTNET_BITCOIN_ASSET;
                let filter = coin_store::UtxoFilter::new()
                    .asset_id(native_asset)
                    .script_pubkey(wallet.signer().p2pk_address(config.address_params())?.script_pubkey())
                    .limit(1);

                let results: Vec<coin_store::UtxoQueryResult> =
                    <_ as UtxoStore>::query_utxos(wallet.store(), &[filter]).await?;

                // Todo(Illia): in future add token merging
                let native_entry = results
                    .into_iter()
                    .next()
                    .and_then(|r| match r {
                        coin_store::UtxoQueryResult::Found(entries) => entries.into_iter().next(),
                        coin_store::UtxoQueryResult::InsufficientValue(_) | coin_store::UtxoQueryResult::Empty => None,
                    })
                    .ok_or_else(|| Error::Config("No native UTXO found".to_string()))?;

                let native_utxo = (*native_entry.outpoint(), native_entry.txout().clone());

                // Todo(Illia): use fee as separate utxo to spend (I mean, don't take fees from native asset utxo)
                let pst = contracts::sdk::transfer_native(native_utxo.clone(), to, *amount, *fee)?;

                let tx = pst.extract_tx()?;
                let utxos = &[native_utxo.1];

                let signature = wallet
                    .signer()
                    .sign_p2pk(&tx, utxos, 0, config.address_params(), liquid_genesis)?;
                let tx = finalize_p2pk_transaction(
                    tx,
                    utxos,
                    &wallet.signer().public_key(),
                    &signature,
                    0,
                    config.address_params(),
                    liquid_genesis,
                )?;

                Broadcaster::from(*broadcast).broadcast_tx(&tx).await?;

                {
                    let outs_to_add = [(0, tx.output[0].clone(), None)];

                    let spent_outpoints = &[native_utxo.0];
                    for outpoint in spent_outpoints {
                        wallet.store().mark_as_spent(*outpoint).await?;
                    }
                    let txid = tx.txid();

                    for (vout, tx_out, blinder) in outs_to_add {
                        #[allow(clippy::cast_possible_truncation)]
                        let new_outpoint = simplicityhl::elements::OutPoint::new(txid, vout as u32);
                        <_ as UtxoStore>::insert(wallet.store(), new_outpoint, tx_out, blinder).await?;
                    }
                }
            }
            BasicCommand::TransferAsset {
                asset,
                to,
                amount,
                fee,
                broadcast,
            } => {
                let liquid_genesis = *LIQUID_TESTNET_GENESIS;
                let wallet = self.get_wallet(&config).await?;

                let native_asset = *simplicityhl_core::LIQUID_TESTNET_BITCOIN_ASSET;
                let transfer_asset = AssetId::from_str(asset)?;

                let filters = {
                    let filter_native = coin_store::UtxoFilter::new()
                        .asset_id(native_asset)
                        .script_pubkey(wallet.signer().p2pk_address(config.address_params())?.script_pubkey())
                        .required_value(*fee)
                        .limit(1);
                    // Todo(Illia): add enum for cli which can take asset entropy both from name or as raw
                    //  (i mean transform asset into enum and create custom fetching before passing into function)
                    let filter_transfer = coin_store::UtxoFilter::new()
                        .asset_id(transfer_asset)
                        .script_pubkey(wallet.signer().p2pk_address(config.address_params())?.script_pubkey())
                        .required_value(*amount)
                        .limit(1);
                    [filter_native, filter_transfer]
                };

                let results: Vec<coin_store::UtxoQueryResult> =
                    <_ as UtxoStore>::query_utxos(wallet.store(), &filters).await?;

                let (asset_entry, fee_entry) =
                    {
                        let mut entries = results.into_iter();

                        // Todo(Illia): in future add token merging
                        let fee_entry = entries
                            .next()
                            .and_then(|r| match r {
                                coin_store::UtxoQueryResult::Found(entries) => entries.into_iter().next(),
                                coin_store::UtxoQueryResult::InsufficientValue(_)
                                | coin_store::UtxoQueryResult::Empty => None,
                            })
                            .ok_or_else(|| Error::Config("No native UTXO found".to_string()))?;
                        // Todo(Illia): in future add token merging
                        let asset_entry = entries
                            .next()
                            .and_then(|r| match r {
                                coin_store::UtxoQueryResult::Found(entries) => entries.into_iter().next(),
                                coin_store::UtxoQueryResult::InsufficientValue(_)
                                | coin_store::UtxoQueryResult::Empty => None,
                            })
                            .ok_or_else(|| Error::Config("No asset UTXO found".to_string()))?;
                        (asset_entry, fee_entry)
                    };

                let asset_utxo = (*asset_entry.outpoint(), asset_entry.txout().clone());
                let fee_utxo = (*fee_entry.outpoint(), fee_entry.txout().clone());

                let pst = contracts::sdk::transfer_asset(asset_utxo.clone(), fee_utxo.clone(), to, *amount, *fee)?;

                let tx = pst.extract_tx()?;
                let utxos = &[asset_utxo.1, fee_utxo.1];

                let signature = wallet
                    .signer()
                    .sign_p2pk(&tx, utxos, 0, config.address_params(), liquid_genesis)?;

                let tx = finalize_p2pk_transaction(
                    tx,
                    utxos,
                    &wallet.signer().public_key(),
                    &signature,
                    0,
                    config.address_params(),
                    liquid_genesis,
                )?;

                let signature = wallet
                    .signer()
                    .sign_p2pk(&tx, utxos, 1, config.address_params(), liquid_genesis)?;

                let tx = finalize_p2pk_transaction(
                    tx,
                    utxos,
                    &wallet.signer().public_key(),
                    &signature,
                    1,
                    config.address_params(),
                    liquid_genesis,
                )?;

                Broadcaster::from(*broadcast).broadcast_tx(&tx).await?;

                {
                    let outs_to_add = [
                        (0, tx.output[0].clone(), Some(PUBLIC_SECRET_BLINDER_KEY)),
                        (1, tx.output[1].clone(), None),
                        (2, tx.output[2].clone(), None),
                    ];

                    let spent_outpoints = &[asset_utxo.0, fee_utxo.0];
                    for outpoint in spent_outpoints {
                        wallet.store().mark_as_spent(*outpoint).await?;
                    }
                    let txid = tx.txid();

                    for (vout, tx_out, blinder) in outs_to_add {
                        #[allow(clippy::cast_possible_truncation)]
                        let new_outpoint = simplicityhl::elements::OutPoint::new(txid, vout as u32);
                        <_ as UtxoStore>::insert(wallet.store(), new_outpoint, tx_out, blinder).await?;
                    }
                }
            }
            BasicCommand::IssueAsset {
                name,
                amount: issue_amount,
                fee,
                broadcast,
            } => {
                let liquid_genesis = *LIQUID_TESTNET_GENESIS;
                let wallet = self.get_wallet(&config).await?;

                let native_asset = *simplicityhl_core::LIQUID_TESTNET_BITCOIN_ASSET;

                // Check whether asset is name already exists before all manipulations
                {
                    let filter = coin_store::AssetEntropyFilter::new().name(name.clone());
                    let results: Vec<coin_store::AssetIdQueryResult> =
                        <_ as AssetEntropyStore>::query(wallet.store(), &[filter]).await?;
                    if results.is_empty() {
                        Err(Error::Config("Failed to receive result on filter retrieval".into()))?;
                    } else if results.len() != 1 {
                        Err(Error::Config("Failed to receive result on filter retrieval".into()))?;
                    } else if let QueryResult::Found(_) = results[0] {
                        Err(Error::Config(format!("Name '{name}' for asset already exist")))?;
                    }
                }

                let filters = {
                    let filter_native = coin_store::UtxoFilter::new()
                        .asset_id(native_asset)
                        .script_pubkey(wallet.signer().p2pk_address(config.address_params())?.script_pubkey())
                        .required_value(*fee)
                        .limit(1);
                    [filter_native]
                };

                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &filters).await?;

                let fee_entry = {
                    let mut entries = results.into_iter();

                    // Todo(Illia): in future add token merging
                    entries
                        .next()
                        .and_then(|r| match r {
                            coin_store::UtxoQueryResult::Found(entries) => entries.into_iter().next(),
                            coin_store::UtxoQueryResult::InsufficientValue(_) | coin_store::UtxoQueryResult::Empty => {
                                None
                            }
                        })
                        .ok_or_else(|| Error::Config("No native UTXO found".to_string()))?
                };

                let fee_utxo = (*fee_entry.outpoint(), fee_entry.txout().clone());

                let pst = contracts::sdk::issue_asset(
                    &derive_public_blinder_key().public_key(),
                    fee_utxo.clone(),
                    *issue_amount,
                    *fee,
                )?;

                let asset_entropy = {
                    let (asset_id, reissuance_asset_id) = pst.inputs()[0].issuance_ids();
                    println!("Issued Asset id: {asset_id}, reissuance asset id: {reissuance_asset_id} ");
                    let asset_entropy = pst.inputs()[0].issuance_asset_entropy.expect("expected entropy");
                    AssetId::generate_asset_entropy(fee_utxo.0, ContractHash::from_byte_array(asset_entropy)).0
                };

                let tx = pst.extract_tx()?;
                let utxos = &[fee_utxo.1];

                let signature = wallet
                    .signer()
                    .sign_p2pk(&tx, utxos, 0, config.address_params(), liquid_genesis)?;

                let tx = finalize_p2pk_transaction(
                    tx,
                    utxos,
                    &wallet.signer().public_key(),
                    &signature,
                    0,
                    config.address_params(),
                    liquid_genesis,
                )?;

                let broadcaster = Broadcaster::from(*broadcast);
                broadcaster.broadcast_tx(&tx).await?;

                {
                    let outs_to_add = [
                        (0, tx.output[0].clone(), Some(PUBLIC_SECRET_BLINDER_KEY)),
                        (1, tx.output[1].clone(), None),
                        (2, tx.output[2].clone(), None),
                    ];

                    let spent_outpoints = &[fee_utxo.0];
                    for outpoint in spent_outpoints {
                        wallet.store().mark_as_spent(*outpoint).await?;
                    }
                    let txid = tx.txid();

                    for (vout, tx_out, blinder) in outs_to_add {
                        #[allow(clippy::cast_possible_truncation)]
                        let new_outpoint = simplicityhl::elements::OutPoint::new(txid, vout as u32);
                        <_ as UtxoStore>::insert(wallet.store(), new_outpoint, tx_out, blinder).await?;
                    }
                }

                if let Broadcaster::Online = broadcaster {
                    <_ as AssetEntropyStore>::insert(wallet.store(), name, asset_entropy).await?;
                }
            }
            BasicCommand::ReissueAsset {
                name,
                amount: reissue_amount,
                fee,
                broadcast,
            } => {
                let liquid_genesis = *LIQUID_TESTNET_GENESIS;
                let wallet = self.get_wallet(&config).await?;

                let native_asset = *simplicityhl_core::LIQUID_TESTNET_BITCOIN_ASSET;
                let blinding_key = derive_public_blinder_key().public_key();

                let entropy_midstate = {
                    let filter = coin_store::AssetEntropyFilter::new().name(name.clone());
                    let results: Vec<coin_store::AssetIdQueryResult> =
                        <_ as AssetEntropyStore>::query(wallet.store(), &[filter]).await?;
                    if results.is_empty() {
                        return Err(Error::Config(format!("No Asset entropy found for this name: {name}")));
                    } else if results.len() != 1 {
                        return Err(Error::Config(format!(
                            "Found more than one Asset entropy for this name: '{name}'"
                        )));
                    }
                    let asset_entropy = match results.into_iter().next().unwrap() {
                        QueryResult::Found(x) => x[0].asset_entropy,
                        QueryResult::Empty => {
                            return Err(Error::Config(format!("No Asset entropy found for this name: '{name}'")));
                        }
                    };
                    println!("asset entropy: {asset_entropy:X?}");
                    sha256::Midstate::from_byte_array(asset_entropy)
                };
                let reissuance_asset_id = AssetId::reissuance_token_from_entropy(entropy_midstate, false);
                println!("midstate: {entropy_midstate}, Issued Asset id: {reissuance_asset_id} ");

                let filters = {
                    let filter_reissue_asset = coin_store::UtxoFilter::new()
                        .asset_id(reissuance_asset_id)
                        .script_pubkey(wallet.signer().p2pk_address(config.address_params())?.script_pubkey())
                        .limit(1);
                    let filter_native = coin_store::UtxoFilter::new()
                        .asset_id(native_asset)
                        .script_pubkey(wallet.signer().p2pk_address(config.address_params())?.script_pubkey())
                        .required_value(*fee)
                        .limit(1);
                    [filter_reissue_asset, filter_native]
                };

                // Todo(Illia): add retrieving of token hex value from wollet?
                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &filters).await?;

                let (reissue_token_entry, fee_entry) =
                    {
                        let mut entries = results.into_iter();

                        // Todo(Illia): in future add merging
                        let reissue_token_entry = entries
                            .next()
                            .and_then(|r| match r {
                                coin_store::UtxoQueryResult::Found(entries) => entries.into_iter().next(),
                                coin_store::UtxoQueryResult::InsufficientValue(_)
                                | coin_store::UtxoQueryResult::Empty => None,
                            })
                            .ok_or_else(|| Error::Config("No reissue token UTXO found".to_string()))?;
                        // Todo(Illia): in future add merging
                        let fee_entry = entries
                            .next()
                            .and_then(|r| match r {
                                coin_store::UtxoQueryResult::Found(entries) => entries.into_iter().next(),
                                coin_store::UtxoQueryResult::InsufficientValue(_)
                                | coin_store::UtxoQueryResult::Empty => None,
                            })
                            .ok_or_else(|| Error::Config("No native UTXO found".to_string()))?;
                        (reissue_token_entry, fee_entry)
                    };

                let reissue_utxo = (*reissue_token_entry.outpoint(), reissue_token_entry.txout().clone());
                let reissue_utxo_secrets = *reissue_token_entry.secrets().unwrap();
                let fee_utxo = (*fee_entry.outpoint(), fee_entry.txout().clone());

                let pst = contracts::sdk::reissue_asset(
                    &blinding_key,
                    reissue_utxo.clone(),
                    reissue_utxo_secrets,
                    fee_utxo.clone(),
                    *reissue_amount,
                    *fee,
                    entropy_midstate,
                )?;

                let tx = pst.extract_tx()?;
                let utxos = &[reissue_utxo.1, fee_utxo.1];

                let signature = wallet
                    .signer()
                    .sign_p2pk(&tx, utxos, 0, config.address_params(), liquid_genesis)?;

                let tx = finalize_p2pk_transaction(
                    tx,
                    utxos,
                    &wallet.signer().public_key(),
                    &signature,
                    0,
                    config.address_params(),
                    liquid_genesis,
                )?;

                let signature = wallet
                    .signer()
                    .sign_p2pk(&tx, utxos, 1, config.address_params(), liquid_genesis)?;

                let tx = finalize_p2pk_transaction(
                    tx,
                    utxos,
                    &wallet.signer().public_key(),
                    &signature,
                    1,
                    config.address_params(),
                    liquid_genesis,
                )?;

                Broadcaster::from(*broadcast).broadcast_tx(&tx).await?;

                {
                    let outs_to_add = [
                        (0, tx.output[0].clone(), Some(PUBLIC_SECRET_BLINDER_KEY)),
                        (1, tx.output[1].clone(), None),
                        (2, tx.output[2].clone(), None),
                    ];

                    let spent_outpoints = &[reissue_utxo.0, fee_utxo.0];
                    for outpoint in spent_outpoints {
                        wallet.store().mark_as_spent(*outpoint).await?;
                    }
                    let txid = tx.txid();

                    for (vout, tx_out, blinder) in outs_to_add {
                        #[allow(clippy::cast_possible_truncation)]
                        let new_outpoint = simplicityhl::elements::OutPoint::new(txid, vout as u32);
                        <_ as UtxoStore>::insert(wallet.store(), new_outpoint, tx_out, blinder).await?;
                    }
                }
            }
        }

        Ok(())
    }
}
