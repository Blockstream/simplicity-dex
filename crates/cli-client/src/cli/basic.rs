use crate::cli::{BasicCommand, Cli};
use crate::config::Config;
use crate::error::Error;

use std::collections::HashMap;

use coin_store::{UtxoQueryResult, UtxoStore};

use simplicityhl::elements::TxOut;
use simplicityhl::elements::hashes::Hash;
use simplicityhl::elements::issuance::ContractHash;
use simplicityhl::elements::pset::serialize::Serialize;
use simplicityhl::elements::pset::{Input, Output, PartiallySignedTransaction};
use simplicityhl::elements::secp256k1_zkp::{self as secp256k1, Keypair};
use simplicityhl::simplicity::hex::DisplayHex;
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, finalize_p2pk_transaction};

impl Cli {
    #[allow(clippy::too_many_lines)]
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
                        coin_store::UtxoQueryResult::InsufficientValue(_, _) => {
                            eprintln!("No single UTXO large enough. Try using 'merge' command first.");
                            None
                        }
                        coin_store::UtxoQueryResult::Empty => None,
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
            BasicCommand::Merge {
                asset_id,
                count,
                fee,
                broadcast,
            } => {
                if *count < 2 {
                    return Err(Error::Config("Need at least 2 UTXOs to merge".to_string()));
                }

                let wallet = self.get_wallet(&config).await?;
                let script_pubkey = wallet.signer().p2pk_address(config.address_params())?.script_pubkey();

                let target_asset = asset_id.unwrap_or(*LIQUID_TESTNET_BITCOIN_ASSET);
                let is_native = target_asset == *LIQUID_TESTNET_BITCOIN_ASSET;

                #[allow(clippy::cast_possible_wrap)]
                let asset_filter = coin_store::UtxoFilter::new()
                    .asset_id(target_asset)
                    .script_pubkey(script_pubkey.clone())
                    .limit(*count as i64);

                let results: Vec<coin_store::UtxoQueryResult> =
                    <_ as UtxoStore>::query_utxos(wallet.store(), &[asset_filter]).await?;

                let entries: Vec<_> = results
                    .into_iter()
                    .next()
                    .and_then(|r| match r {
                        coin_store::UtxoQueryResult::Found(entries, _) => Some(entries),
                        coin_store::UtxoQueryResult::InsufficientValue(entries, _) => {
                            eprintln!("Only found {} UTXOs for merge.", entries.len());
                            Some(entries)
                        }
                        coin_store::UtxoQueryResult::Empty => None,
                    })
                    .ok_or_else(|| Error::Config(format!("No UTXOs found for asset {target_asset}")))?;

                if entries.len() < 2 {
                    return Err(Error::Config(format!(
                        "Need at least 2 UTXOs to merge, found {}",
                        entries.len()
                    )));
                }

                let total_asset_value: u64 = entries.iter().filter_map(coin_store::UtxoEntry::value).sum();
                let mut pst = PartiallySignedTransaction::new_v2();

                let mut utxos: Vec<TxOut> = entries
                    .iter()
                    .map(|e| {
                        let mut input = Input::from_prevout(*e.outpoint());
                        input.witness_utxo = Some(e.txout().clone());
                        pst.add_input(input);
                        e.txout().clone()
                    })
                    .collect();

                if is_native {
                    let output_value = total_asset_value
                        .checked_sub(*fee)
                        .ok_or_else(|| Error::Config("Fee exceeds total UTXO value".to_string()))?;

                    pst.add_output(Output::new_explicit(
                        script_pubkey,
                        output_value,
                        *LIQUID_TESTNET_BITCOIN_ASSET,
                        None,
                    ));

                    println!(
                        "Merging {} native UTXOs ({} sats) -> 1 UTXO ({} sats)",
                        entries.len(),
                        total_asset_value,
                        output_value
                    );
                } else {
                    let fee_filter = coin_store::UtxoFilter::new()
                        .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                        .script_pubkey(script_pubkey.clone())
                        .required_value(*fee);

                    let fee_results: Vec<coin_store::UtxoQueryResult> =
                        <_ as UtxoStore>::query_utxos(wallet.store(), &[fee_filter]).await?;

                    let fee_entry = fee_results
                        .into_iter()
                        .next()
                        .and_then(|r| match r {
                            coin_store::UtxoQueryResult::Found(entries, _) => entries.into_iter().next(),
                            coin_store::UtxoQueryResult::InsufficientValue(entries, _) => {
                                let available: u64 = entries.iter().filter_map(coin_store::UtxoEntry::value).sum();
                                eprintln!(
                                    "Insufficient LBTC for fee: have {available} sats, need {fee} sats. Try using 'merge' command first."
                                );
                                None
                            }
                            coin_store::UtxoQueryResult::Empty => None,
                        })
                        .ok_or_else(|| Error::Config(format!("No LBTC UTXO found to pay fee of {fee} sats")))?;

                    let Some(fee_input_value) = fee_entry.value() else {
                        return Err(Error::Config("Unexpected confidential value".to_string()));
                    };

                    let mut fee_input = Input::from_prevout(*fee_entry.outpoint());
                    fee_input.witness_utxo = Some(fee_entry.txout().clone());
                    pst.add_input(fee_input);
                    utxos.push(fee_entry.txout().clone());

                    pst.add_output(Output::new_explicit(
                        script_pubkey.clone(),
                        total_asset_value,
                        target_asset,
                        None,
                    ));

                    if fee_input_value > *fee {
                        pst.add_output(Output::new_explicit(
                            script_pubkey,
                            fee_input_value - *fee,
                            *LIQUID_TESTNET_BITCOIN_ASSET,
                            None,
                        ));
                    }

                    println!(
                        "Merging {} UTXOs of asset {} ({} units) -> 1 UTXO",
                        entries.len(),
                        target_asset,
                        total_asset_value
                    );
                }

                pst.add_output(Output::from_txout(TxOut::new_fee(*fee, *LIQUID_TESTNET_BITCOIN_ASSET)));

                let mut tx = pst.extract_tx()?;

                for (i, _) in utxos.iter().enumerate() {
                    let signature =
                        wallet
                            .signer()
                            .sign_p2pk(&tx, &utxos, i, config.address_params(), *LIQUID_TESTNET_GENESIS)?;

                    tx = finalize_p2pk_transaction(
                        tx,
                        &utxos,
                        &wallet.signer().public_key(),
                        &signature,
                        i,
                        config.address_params(),
                        *LIQUID_TESTNET_GENESIS,
                    )?;
                }

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
            BasicCommand::Transfer {
                asset_id,
                to,
                amount,
                fee,
                broadcast,
            } => {
                let wallet = self.get_wallet(&config).await?;
                let script_pubkey = wallet.signer().p2pk_address(config.address_params())?.script_pubkey();

                let target_asset = asset_id.unwrap_or(*LIQUID_TESTNET_BITCOIN_ASSET);
                let is_native = target_asset == *LIQUID_TESTNET_BITCOIN_ASSET;

                let required_amount = if is_native { *amount + *fee } else { *amount };

                let asset_filter = coin_store::UtxoFilter::new()
                    .asset_id(target_asset)
                    .script_pubkey(script_pubkey.clone())
                    .required_value(required_amount);

                let results: Vec<coin_store::UtxoQueryResult> =
                    <_ as UtxoStore>::query_utxos(wallet.store(), &[asset_filter]).await?;

                let entries: Vec<_> = results
                    .into_iter()
                    .next()
                    .and_then(|r| match r {
                        coin_store::UtxoQueryResult::Found(entries, _) => Some(entries),
                        coin_store::UtxoQueryResult::InsufficientValue(entries, _) => {
                            let available: u64 = entries.iter().filter_map(coin_store::UtxoEntry::value).sum();
                            eprintln!(
                                "Insufficient funds: have {available} sats, need {required_amount} sats. Try using 'merge' command first."
                            );
                            None
                        }
                        coin_store::UtxoQueryResult::Empty => None,
                    })
                    .ok_or_else(|| Error::Config(format!("No UTXOs found for asset {target_asset}")))?;

                let total_asset_value: u64 = entries.iter().filter_map(coin_store::UtxoEntry::value).sum();
                let mut pst = PartiallySignedTransaction::new_v2();

                let mut utxos: Vec<TxOut> = entries
                    .iter()
                    .map(|e| {
                        let mut input = Input::from_prevout(*e.outpoint());
                        input.witness_utxo = Some(e.txout().clone());
                        pst.add_input(input);
                        e.txout().clone()
                    })
                    .collect();

                if is_native {
                    pst.add_output(Output::new_explicit(
                        to.script_pubkey(),
                        *amount,
                        *LIQUID_TESTNET_BITCOIN_ASSET,
                        None,
                    ));

                    let change = total_asset_value
                        .checked_sub(*amount + *fee)
                        .ok_or_else(|| Error::Config("Fee + amount exceeds total UTXO value".to_string()))?;

                    if change > 0 {
                        pst.add_output(Output::new_explicit(
                            script_pubkey,
                            change,
                            *LIQUID_TESTNET_BITCOIN_ASSET,
                            None,
                        ));
                    }

                    println!("Transferring {amount} sats LBTC to {to}");
                } else {
                    let fee_filter = coin_store::UtxoFilter::new()
                        .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                        .script_pubkey(script_pubkey.clone())
                        .required_value(*fee);

                    let fee_results: Vec<coin_store::UtxoQueryResult> =
                        <_ as UtxoStore>::query_utxos(wallet.store(), &[fee_filter]).await?;

                    let fee_entry = fee_results
                        .into_iter()
                        .next()
                        .and_then(|r| match r {
                            coin_store::UtxoQueryResult::Found(entries, _) => entries.into_iter().next(),
                            coin_store::UtxoQueryResult::InsufficientValue(entries, _) => {
                                let available: u64 = entries.iter().filter_map(coin_store::UtxoEntry::value).sum();
                                eprintln!(
                                    "Insufficient LBTC for fee: have {available} sats, need {fee} sats. Try using 'merge' command first."
                                );
                                None
                            }
                            coin_store::UtxoQueryResult::Empty => None,
                        })
                        .ok_or_else(|| Error::Config(format!("No LBTC UTXO found to pay fee of {fee} sats")))?;

                    let Some(fee_input_value) = fee_entry.value() else {
                        return Err(Error::Config("Unexpected confidential value".to_string()));
                    };

                    let mut fee_input = Input::from_prevout(*fee_entry.outpoint());
                    fee_input.witness_utxo = Some(fee_entry.txout().clone());
                    pst.add_input(fee_input);
                    utxos.push(fee_entry.txout().clone());

                    pst.add_output(Output::new_explicit(to.script_pubkey(), *amount, target_asset, None));

                    let asset_change = total_asset_value - *amount;
                    if asset_change > 0 {
                        pst.add_output(Output::new_explicit(
                            script_pubkey.clone(),
                            asset_change,
                            target_asset,
                            None,
                        ));
                    }

                    if fee_input_value > *fee {
                        pst.add_output(Output::new_explicit(
                            script_pubkey,
                            fee_input_value - *fee,
                            *LIQUID_TESTNET_BITCOIN_ASSET,
                            None,
                        ));
                    }

                    println!("Transferring {amount} units of asset {target_asset} to {to}");
                }

                pst.add_output(Output::from_txout(TxOut::new_fee(*fee, *LIQUID_TESTNET_BITCOIN_ASSET)));

                let mut tx = pst.extract_tx()?;

                for (i, _) in utxos.iter().enumerate() {
                    let signature =
                        wallet
                            .signer()
                            .sign_p2pk(&tx, &utxos, i, config.address_params(), *LIQUID_TESTNET_GENESIS)?;

                    tx = finalize_p2pk_transaction(
                        tx,
                        &utxos,
                        &wallet.signer().public_key(),
                        &signature,
                        i,
                        config.address_params(),
                        *LIQUID_TESTNET_GENESIS,
                    )?;
                }

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
            BasicCommand::IssueAsset { amount, fee, broadcast } => {
                let wallet = self.get_wallet(&config).await?;
                let script_pubkey = wallet.signer().p2pk_address(config.address_params())?.script_pubkey();

                let fee_filter = coin_store::UtxoFilter::new()
                    .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                    .script_pubkey(script_pubkey)
                    .required_value(*fee);

                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[fee_filter]).await?;

                let fee_entry = results
                    .into_iter()
                    .next()
                    .and_then(|r| match r {
                        coin_store::UtxoQueryResult::Found(entries, _) => entries.into_iter().next(),
                        coin_store::UtxoQueryResult::InsufficientValue(entries, _) => {
                            let available: u64 = entries.iter().filter_map(coin_store::UtxoEntry::value).sum();
                            eprintln!(
                                "Insufficient LBTC for fee: have {available} sats, need {fee} sats. Try using 'merge' command first."
                            );
                            None
                        }
                        coin_store::UtxoQueryResult::Empty => None,
                    })
                    .ok_or_else(|| Error::Config(format!("No LBTC UTXO found to pay fee of {fee} sats")))?;

                let fee_utxo = (*fee_entry.outpoint(), fee_entry.txout().clone());

                let blinding_keypair = Keypair::new(secp256k1::SECP256K1, &mut secp256k1::rand::thread_rng());

                let pst = contracts::sdk::issue_asset(&blinding_keypair.public_key(), fee_utxo.clone(), *amount, *fee)?;

                let (asset_id, token_id) = pst.inputs()[0].issuance_ids();
                let asset_entropy_bytes = pst.inputs()[0]
                    .issuance_asset_entropy
                    .ok_or_else(|| Error::Config("Missing asset entropy in PST".to_string()))?;
                let contract_hash = ContractHash::from_byte_array(asset_entropy_bytes);
                let entropy = simplicityhl::elements::issuance::AssetId::generate_asset_entropy(
                    *fee_entry.outpoint(),
                    contract_hash,
                );

                let mut tx = pst.extract_tx()?;
                let utxos = &[fee_utxo.1];

                let signature =
                    wallet
                        .signer()
                        .sign_p2pk(&tx, utxos, 0, config.address_params(), *LIQUID_TESTNET_GENESIS)?;

                tx = finalize_p2pk_transaction(
                    tx,
                    utxos,
                    &wallet.signer().public_key(),
                    &signature,
                    0,
                    config.address_params(),
                    *LIQUID_TESTNET_GENESIS,
                )?;

                println!("Asset ID: {asset_id}");
                println!("Reissuance Token ID: {token_id}");
                println!("Asset Entropy: {}", entropy.to_byte_array().to_lower_hex_string());

                match broadcast {
                    false => {
                        println!("{}", tx.serialize().to_lower_hex_string());
                    }
                    true => {
                        cli_helper::explorer::broadcast_tx(&tx).await?;

                        println!("Broadcasted: {}", tx.txid());

                        let mut blinder_keys = HashMap::new();
                        blinder_keys.insert(0, blinding_keypair);
                        wallet.store().insert_transaction(&tx, blinder_keys).await?;
                    }
                }
            }
            BasicCommand::ReissueAsset {
                asset_id,
                amount,
                fee,
                broadcast,
            } => {
                let wallet = self.get_wallet(&config).await?;
                let script_pubkey = wallet.signer().p2pk_address(config.address_params())?.script_pubkey();

                let asset_filter = coin_store::UtxoFilter::new()
                    .asset_id(*asset_id)
                    .script_pubkey(script_pubkey.clone())
                    .include_entropy()
                    .limit(1);

                let asset_results = <_ as UtxoStore>::query_utxos(wallet.store(), &[asset_filter]).await?;

                let asset_entry = asset_results
                    .into_iter()
                    .next()
                    .and_then(|r| match r {
                        coin_store::UtxoQueryResult::Found(entries, _)
                        | coin_store::UtxoQueryResult::InsufficientValue(entries, _) => entries.into_iter().next(),
                        coin_store::UtxoQueryResult::Empty => None,
                    })
                    .ok_or_else(|| Error::Config(format!("No UTXO found for asset {asset_id}")))?;

                let (_, token_id) = asset_entry.issuance_ids().ok_or_else(|| {
                    Error::Config(format!(
                        "No issuance data found for asset {asset_id}. Was this asset issued by this wallet?"
                    ))
                })?;

                let entropy = asset_entry
                    .entropy()
                    .0
                    .ok_or_else(|| Error::Config("Missing entropy".to_string()))?;

                let token_filter = coin_store::UtxoFilter::new()
                    .asset_id(token_id)
                    .script_pubkey(script_pubkey.clone())
                    .limit(1);

                let fee_filter = coin_store::UtxoFilter::new()
                    .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                    .script_pubkey(script_pubkey)
                    .required_value(*fee)
                    .limit(1);

                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[token_filter, fee_filter]).await?;

                let token_entry = match &results[0] {
                    UtxoQueryResult::Found(entries, _) => &entries[0],
                    UtxoQueryResult::InsufficientValue(entries, _) if !entries.is_empty() => &entries[0],
                    _ => return Err(Error::Config(format!("No reissuance token UTXO found for {token_id}"))),
                };

                let token_secrets = token_entry
                    .secrets()
                    .ok_or_else(|| Error::Config("Reissuance token must be confidential".to_string()))?;

                let fee_entry = match &results[1] {
                    UtxoQueryResult::Found(entries, _) => &entries[0],
                    UtxoQueryResult::InsufficientValue(entries, _) => {
                        let available: u64 = entries.iter().filter_map(coin_store::UtxoEntry::value).sum();
                        eprintln!(
                            "Insufficient LBTC for fee: have {available} sats, need {fee} sats. Try using 'merge' command first."
                        );
                        return Err(Error::Config(format!("No LBTC UTXO found to pay fee of {fee} sats")));
                    }
                    UtxoQueryResult::Empty => {
                        return Err(Error::Config(format!("No LBTC UTXO found to pay fee of {fee} sats")));
                    }
                };

                let token_utxo = (*token_entry.outpoint(), token_entry.txout().clone());
                let fee_utxo = (*fee_entry.outpoint(), fee_entry.txout().clone());

                let blinding_keypair = Keypair::new(secp256k1::SECP256K1, &mut secp256k1::rand::thread_rng());

                let pst = contracts::sdk::reissue_asset(
                    &blinding_keypair.public_key(),
                    token_utxo.clone(),
                    *token_secrets,
                    fee_utxo.clone(),
                    *amount,
                    *fee,
                    entropy,
                )?;

                let mut tx = pst.extract_tx()?;
                let utxos = vec![token_utxo.1, fee_utxo.1];

                for i in 0..2 {
                    let signature =
                        wallet
                            .signer()
                            .sign_p2pk(&tx, &utxos, i, config.address_params(), *LIQUID_TESTNET_GENESIS)?;

                    tx = finalize_p2pk_transaction(
                        tx,
                        &utxos,
                        &wallet.signer().public_key(),
                        &signature,
                        i,
                        config.address_params(),
                        *LIQUID_TESTNET_GENESIS,
                    )?;
                }

                println!("Reissuing {amount} units of asset {asset_id}");

                match broadcast {
                    false => {
                        println!("{}", tx.serialize().to_lower_hex_string());
                    }
                    true => {
                        cli_helper::explorer::broadcast_tx(&tx).await?;
                        println!("Broadcasted: {}", tx.txid());

                        let mut blinder_keys = HashMap::new();
                        blinder_keys.insert(0, blinding_keypair);
                        wallet.store().insert_transaction(&tx, blinder_keys).await?;
                    }
                }
            }
        }

        Ok(())
    }
}
