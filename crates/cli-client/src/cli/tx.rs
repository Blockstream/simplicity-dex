use crate::cli::{Cli, TxCommand};
use crate::config::Config;
use crate::error::Error;
use crate::fee::{PLACEHOLDER_FEE, estimate_fee_signed};
use crate::signing::sign_p2pk_inputs;

use std::collections::HashMap;

use coin_store::{UtxoQueryResult, UtxoStore};

use simplicityhl::elements::TxOut;
use simplicityhl::elements::hashes::Hash;
use simplicityhl::elements::issuance::ContractHash;
use simplicityhl::elements::pset::serialize::Serialize;
use simplicityhl::elements::pset::{Input, Output, PartiallySignedTransaction};
use simplicityhl::elements::secp256k1_zkp::{self as secp256k1, Keypair};
use simplicityhl::simplicity::hex::DisplayHex;
use simplicityhl_core::LIQUID_TESTNET_BITCOIN_ASSET;

impl Cli {
    #[allow(clippy::too_many_lines)]
    pub(crate) async fn run_tx(&self, config: Config, command: &TxCommand) -> Result<(), Error> {
        match command {
            TxCommand::SplitNative { count, fee, broadcast } => {
                let wallet = self.get_wallet(&config).await?;

                let filter = coin_store::UtxoFilter::new()
                    .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                    .script_pubkey(wallet.signer().p2pk_address(config.network())?.script_pubkey());

                let results: Vec<UtxoQueryResult> = <_ as UtxoStore>::query_utxos(wallet.store(), &[filter]).await?;

                let native_entry = results
                    .into_iter()
                    .next()
                    .and_then(|r| match r {
                        UtxoQueryResult::Found(entries, _) => entries.into_iter().next(),
                        UtxoQueryResult::InsufficientValue(_, _) => {
                            eprintln!("No single UTXO large enough. Try using 'merge' command first.");
                            None
                        }
                        UtxoQueryResult::Empty => None,
                    })
                    .ok_or_else(|| Error::Config("No native UTXO found".to_string()))?;

                let fee_utxo = (*native_entry.outpoint(), native_entry.txout().clone());

                let actual_fee = estimate_fee_signed(
                    fee.as_ref(),
                    config.get_fee_rate(),
                    |f| {
                        let pst = contracts::sdk::split_native_any(fee_utxo.clone(), *count, f)?;
                        Ok((pst, vec![fee_utxo.1.clone()]))
                    },
                    |tx, utxos| sign_p2pk_inputs(tx, utxos, &wallet, config.network(), 0),
                )?;

                let pst = contracts::sdk::split_native_any(fee_utxo.clone(), *count, actual_fee)?;
                let tx = pst.extract_tx()?;
                let utxos = vec![fee_utxo.1];

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.network(), 0)?;

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
            TxCommand::Merge {
                asset_id,
                count,
                fee,
                broadcast,
            } => {
                if *count < 2 {
                    return Err(Error::Config("Need at least 2 UTXOs to merge".to_string()));
                }

                let wallet = self.get_wallet(&config).await?;
                let script_pubkey = wallet.signer().p2pk_address(config.network())?.script_pubkey();

                let target_asset = asset_id.unwrap_or(*LIQUID_TESTNET_BITCOIN_ASSET);
                let is_native = target_asset == *LIQUID_TESTNET_BITCOIN_ASSET;

                #[allow(clippy::cast_possible_wrap)]
                let asset_filter = coin_store::UtxoFilter::new()
                    .asset_id(target_asset)
                    .script_pubkey(script_pubkey.clone())
                    .limit(*count as i64);

                let results: Vec<UtxoQueryResult> =
                    <_ as UtxoStore>::query_utxos(wallet.store(), &[asset_filter]).await?;

                let entries: Vec<_> = results
                    .into_iter()
                    .next()
                    .and_then(|r| match r {
                        UtxoQueryResult::Found(entries, _) => Some(entries),
                        UtxoQueryResult::InsufficientValue(entries, _) => {
                            eprintln!("Only found {} UTXOs for merge.", entries.len());
                            Some(entries)
                        }
                        UtxoQueryResult::Empty => None,
                    })
                    .ok_or_else(|| Error::Config(format!("No UTXOs found for asset {target_asset}")))?;

                if entries.len() < 2 {
                    return Err(Error::Config(format!(
                        "Need at least 2 UTXOs to merge, found {}",
                        entries.len()
                    )));
                }

                let total_asset_value: u64 = entries.iter().filter_map(coin_store::UtxoEntry::value).sum();

                let build_merge_pset = |actual_fee: u64,
                                        fee_entry: Option<&coin_store::UtxoEntry>|
                 -> Result<(PartiallySignedTransaction, Vec<TxOut>), Error> {
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
                            .checked_sub(actual_fee)
                            .ok_or_else(|| Error::Config("Fee exceeds total UTXO value".to_string()))?;
                        pst.add_output(Output::new_explicit(
                            script_pubkey.clone(),
                            output_value,
                            *LIQUID_TESTNET_BITCOIN_ASSET,
                            None,
                        ));
                    } else if let Some(fee_e) = fee_entry {
                        let Some(fee_input_value) = fee_e.value() else {
                            return Err(Error::Config("Unexpected confidential value".to_string()));
                        };
                        let mut fee_input = Input::from_prevout(*fee_e.outpoint());
                        fee_input.witness_utxo = Some(fee_e.txout().clone());
                        pst.add_input(fee_input);
                        utxos.push(fee_e.txout().clone());

                        pst.add_output(Output::new_explicit(
                            script_pubkey.clone(),
                            total_asset_value,
                            target_asset,
                            None,
                        ));

                        if fee_input_value > actual_fee {
                            pst.add_output(Output::new_explicit(
                                script_pubkey.clone(),
                                fee_input_value - actual_fee,
                                *LIQUID_TESTNET_BITCOIN_ASSET,
                                None,
                            ));
                        }
                    }

                    pst.add_output(Output::from_txout(TxOut::new_fee(
                        actual_fee,
                        *LIQUID_TESTNET_BITCOIN_ASSET,
                    )));
                    Ok((pst, utxos))
                };

                let fee_entry_opt = if is_native {
                    None
                } else {
                    let fee_filter = coin_store::UtxoFilter::new()
                        .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                        .script_pubkey(script_pubkey.clone())
                        .required_value(fee.unwrap_or(PLACEHOLDER_FEE));

                    let fee_results: Vec<UtxoQueryResult> =
                        <_ as UtxoStore>::query_utxos(wallet.store(), &[fee_filter]).await?;

                    Some(fee_results
                        .into_iter()
                        .next()
                        .and_then(|r| match r {
                            UtxoQueryResult::Found(entries, _) => entries.into_iter().next(),
                            UtxoQueryResult::InsufficientValue(entries, _) => {
                                let available: u64 = entries.iter().filter_map(coin_store::UtxoEntry::value).sum();
                                eprintln!(
                                    "Insufficient LBTC for fee: have {available} sats. Try using 'merge' command first."
                                );
                                None
                            }
                            UtxoQueryResult::Empty => None,
                        })
                        .ok_or_else(|| Error::Config("No LBTC UTXO found to pay fee".to_string()))?)
                };

                let actual_fee = estimate_fee_signed(
                    fee.as_ref(),
                    config.get_fee_rate(),
                    |f| build_merge_pset(f, fee_entry_opt.as_ref()),
                    |tx, utxos| sign_p2pk_inputs(tx, utxos, &wallet, config.network(), 0),
                )?;

                if !is_native && let Some(ref fee_e) = fee_entry_opt {
                    let Some(fee_input_value) = fee_e.value() else {
                        return Err(Error::Config("Unexpected confidential value".to_string()));
                    };
                    if fee_input_value < actual_fee {
                        return Err(Error::Config(format!(
                            "Fee UTXO value ({fee_input_value} sats) is less than required fee ({actual_fee} sats)"
                        )));
                    }
                }

                let (pst, utxos) = build_merge_pset(actual_fee, fee_entry_opt.as_ref())?;

                if is_native {
                    println!(
                        "Merging {} native UTXOs ({} sats) -> 1 UTXO ({} sats)",
                        entries.len(),
                        total_asset_value,
                        total_asset_value - actual_fee
                    );
                } else {
                    println!(
                        "Merging {} UTXOs of asset {} ({} units) -> 1 UTXO",
                        entries.len(),
                        target_asset,
                        total_asset_value
                    );
                }

                let tx = pst.extract_tx()?;
                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.network(), 0)?;

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
            TxCommand::Transfer {
                asset_id,
                to,
                amount,
                fee,
                broadcast,
            } => {
                let wallet = self.get_wallet(&config).await?;
                let script_pubkey = wallet.signer().p2pk_address(config.network())?.script_pubkey();

                let target_asset = asset_id.unwrap_or(*LIQUID_TESTNET_BITCOIN_ASSET);
                let is_native = target_asset == *LIQUID_TESTNET_BITCOIN_ASSET;

                let required_amount = if is_native {
                    *amount + fee.unwrap_or(PLACEHOLDER_FEE)
                } else {
                    *amount
                };

                let asset_filter = coin_store::UtxoFilter::new()
                    .asset_id(target_asset)
                    .script_pubkey(script_pubkey.clone())
                    .required_value(required_amount);

                let results: Vec<UtxoQueryResult> =
                    <_ as UtxoStore>::query_utxos(wallet.store(), &[asset_filter]).await?;

                let entries: Vec<_> = results
                    .into_iter()
                    .next()
                    .and_then(|r| match r {
                        UtxoQueryResult::Found(entries, _) => Some(entries),
                        UtxoQueryResult::InsufficientValue(entries, _) => {
                            let available: u64 = entries.iter().filter_map(coin_store::UtxoEntry::value).sum();
                            eprintln!(
                                "Insufficient funds: have {available} sats, need {required_amount} sats. Try using 'merge' command first."
                            );
                            None
                        }
                        UtxoQueryResult::Empty => None,
                    })
                    .ok_or_else(|| Error::Config(format!("No UTXOs found for asset {target_asset}")))?;

                let total_asset_value: u64 = entries.iter().filter_map(coin_store::UtxoEntry::value).sum();

                let build_transfer_pset = |actual_fee: u64,
                                           fee_entry: Option<&coin_store::UtxoEntry>|
                 -> Result<(PartiallySignedTransaction, Vec<TxOut>), Error> {
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
                            .checked_sub(*amount + actual_fee)
                            .ok_or_else(|| Error::Config("Fee + amount exceeds total UTXO value".to_string()))?;

                        if change > 0 {
                            pst.add_output(Output::new_explicit(
                                script_pubkey.clone(),
                                change,
                                *LIQUID_TESTNET_BITCOIN_ASSET,
                                None,
                            ));
                        }
                    } else if let Some(fee_e) = fee_entry {
                        let Some(fee_input_value) = fee_e.value() else {
                            return Err(Error::Config("Unexpected confidential value".to_string()));
                        };

                        let mut fee_input = Input::from_prevout(*fee_e.outpoint());
                        fee_input.witness_utxo = Some(fee_e.txout().clone());
                        pst.add_input(fee_input);
                        utxos.push(fee_e.txout().clone());

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

                        if fee_input_value > actual_fee {
                            pst.add_output(Output::new_explicit(
                                script_pubkey.clone(),
                                fee_input_value - actual_fee,
                                *LIQUID_TESTNET_BITCOIN_ASSET,
                                None,
                            ));
                        }
                    }

                    pst.add_output(Output::from_txout(TxOut::new_fee(
                        actual_fee,
                        *LIQUID_TESTNET_BITCOIN_ASSET,
                    )));
                    Ok((pst, utxos))
                };

                let fee_entry_opt = if is_native {
                    None
                } else {
                    let fee_filter = coin_store::UtxoFilter::new()
                        .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                        .script_pubkey(script_pubkey.clone())
                        .required_value(fee.unwrap_or(PLACEHOLDER_FEE));

                    let fee_results: Vec<UtxoQueryResult> =
                        <_ as UtxoStore>::query_utxos(wallet.store(), &[fee_filter]).await?;

                    Some(fee_results
                        .into_iter()
                        .next()
                        .and_then(|r| match r {
                            UtxoQueryResult::Found(entries, _) => entries.into_iter().next(),
                            UtxoQueryResult::InsufficientValue(entries, _) => {
                                let available: u64 = entries.iter().filter_map(coin_store::UtxoEntry::value).sum();
                                eprintln!(
                                    "Insufficient LBTC for fee: have {available} sats. Try using 'merge' command first."
                                );
                                None
                            }
                            UtxoQueryResult::Empty => None,
                        })
                        .ok_or_else(|| Error::Config("No LBTC UTXO found to pay fee".to_string()))?)
                };

                let actual_fee = estimate_fee_signed(
                    fee.as_ref(),
                    config.get_fee_rate(),
                    |f| build_transfer_pset(f, fee_entry_opt.as_ref()),
                    |tx, utxos| sign_p2pk_inputs(tx, utxos, &wallet, config.network(), 0),
                )?;

                if is_native && total_asset_value < *amount + actual_fee {
                    return Err(Error::Config(format!(
                        "Insufficient funds: have {total_asset_value} sats, need {} sats (amount + fee)",
                        *amount + actual_fee
                    )));
                }

                if !is_native && let Some(ref fee_e) = fee_entry_opt {
                    let Some(fee_input_value) = fee_e.value() else {
                        return Err(Error::Config("Unexpected confidential value".to_string()));
                    };
                    if fee_input_value < actual_fee {
                        return Err(Error::Config(format!(
                            "Fee UTXO value ({fee_input_value} sats) is less than required fee ({actual_fee} sats)"
                        )));
                    }
                }

                let (pst, utxos) = build_transfer_pset(actual_fee, fee_entry_opt.as_ref())?;

                if is_native {
                    println!("Transferring {amount} sats LBTC to {to}");
                } else {
                    println!("Transferring {amount} units of asset {target_asset} to {to}");
                }

                let tx = pst.extract_tx()?;
                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.network(), 0)?;

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
            TxCommand::IssueAsset { amount, fee, broadcast } => {
                let wallet = self.get_wallet(&config).await?;
                let script_pubkey = wallet.signer().p2pk_address(config.network())?.script_pubkey();

                let fee_filter = coin_store::UtxoFilter::new()
                    .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                    .script_pubkey(script_pubkey)
                    .required_value(fee.unwrap_or(PLACEHOLDER_FEE));

                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[fee_filter]).await?;

                let fee_entry = results
                    .into_iter()
                    .next()
                    .and_then(|r| match r {
                        UtxoQueryResult::Found(entries, _) => entries.into_iter().next(),
                        UtxoQueryResult::InsufficientValue(entries, _) => {
                            let available: u64 = entries.iter().filter_map(coin_store::UtxoEntry::value).sum();
                            eprintln!(
                                "Insufficient LBTC for fee: have {available} sats. Try using 'merge' command first."
                            );
                            None
                        }
                        UtxoQueryResult::Empty => None,
                    })
                    .ok_or_else(|| Error::Config("No LBTC UTXO found to pay fee".to_string()))?;

                let fee_utxo = (*fee_entry.outpoint(), fee_entry.txout().clone());

                let blinding_keypair = Keypair::new(secp256k1::SECP256K1, &mut secp256k1::rand::thread_rng());

                let actual_fee = estimate_fee_signed(
                    fee.as_ref(),
                    config.get_fee_rate(),
                    |f| {
                        let pst =
                            contracts::sdk::issue_asset(&blinding_keypair.public_key(), fee_utxo.clone(), *amount, f)?;
                        Ok((pst, vec![fee_utxo.1.clone()]))
                    },
                    |tx, utxos| sign_p2pk_inputs(tx, utxos, &wallet, config.network(), 0),
                )?;

                if let Some(fee_input_value) = fee_entry.value()
                    && fee_input_value < actual_fee
                {
                    return Err(Error::Config(format!(
                        "Fee UTXO value ({fee_input_value} sats) is less than required fee ({actual_fee} sats)"
                    )));
                }

                let pst =
                    contracts::sdk::issue_asset(&blinding_keypair.public_key(), fee_utxo.clone(), *amount, actual_fee)?;

                let (asset_id, token_id) = pst.inputs()[0].issuance_ids();
                let asset_entropy_bytes = pst.inputs()[0]
                    .issuance_asset_entropy
                    .ok_or_else(|| Error::Config("Missing asset entropy in PST".to_string()))?;
                let contract_hash = ContractHash::from_byte_array(asset_entropy_bytes);
                let entropy = simplicityhl::elements::issuance::AssetId::generate_asset_entropy(
                    *fee_entry.outpoint(),
                    contract_hash,
                );

                let tx = pst.extract_tx()?;
                let utxos = vec![fee_utxo.1];

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.network(), 0)?;

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
            TxCommand::ReissueAsset {
                asset_id,
                amount,
                fee,
                broadcast,
            } => {
                let wallet = self.get_wallet(&config).await?;
                let script_pubkey = wallet.signer().p2pk_address(config.network())?.script_pubkey();

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
                        UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _) => {
                            entries.into_iter().next()
                        }
                        UtxoQueryResult::Empty => None,
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
                    .required_value(fee.unwrap_or(PLACEHOLDER_FEE))
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
                        eprintln!("Insufficient LBTC for fee: have {available} sats. Try using 'merge' command first.");
                        return Err(Error::Config("No LBTC UTXO found to pay fee".to_string()));
                    }
                    UtxoQueryResult::Empty => {
                        return Err(Error::Config("No LBTC UTXO found to pay fee".to_string()));
                    }
                };

                let token_utxo = (*token_entry.outpoint(), token_entry.txout().clone());
                let fee_utxo = (*fee_entry.outpoint(), fee_entry.txout().clone());

                let blinding_keypair = Keypair::new(secp256k1::SECP256K1, &mut secp256k1::rand::thread_rng());

                let actual_fee = estimate_fee_signed(
                    fee.as_ref(),
                    config.get_fee_rate(),
                    |f| {
                        let pst = contracts::sdk::reissue_asset(
                            &blinding_keypair.public_key(),
                            token_utxo.clone(),
                            *token_secrets,
                            fee_utxo.clone(),
                            *amount,
                            f,
                            entropy,
                        )?;
                        Ok((pst, vec![token_utxo.1.clone(), fee_utxo.1.clone()]))
                    },
                    |tx, utxos| sign_p2pk_inputs(tx, utxos, &wallet, config.network(), 0),
                )?;

                if let Some(fee_input_value) = fee_entry.value()
                    && fee_input_value < actual_fee
                {
                    return Err(Error::Config(format!(
                        "Fee UTXO value ({fee_input_value} sats) is less than required fee ({actual_fee} sats)"
                    )));
                }

                let pst = contracts::sdk::reissue_asset(
                    &blinding_keypair.public_key(),
                    token_utxo.clone(),
                    *token_secrets,
                    fee_utxo.clone(),
                    *amount,
                    actual_fee,
                    entropy,
                )?;

                let tx = pst.extract_tx()?;
                let utxos = vec![token_utxo.1, fee_utxo.1];

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.network(), 0)?;

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
