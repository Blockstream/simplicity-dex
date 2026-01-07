use std::collections::HashMap;

use crate::cli::interactive::{
    GRANTOR_TOKEN_TAG, OPTION_TOKEN_TAG, current_timestamp, extract_entries_from_result, format_relative_time,
    get_grantor_tokens_from_wallet, get_option_tokens_from_wallet, parse_expiry, prompt_amount,
    select_enriched_token_interactive,
};
use crate::cli::{Cli, OptionCommand};
use crate::config::Config;
use crate::error::Error;
use crate::fee::{PLACEHOLDER_FEE, estimate_fee_signed};
use crate::metadata::{ContractMetadata, HistoryEntry};
use crate::signing::sign_p2pk_inputs;
use crate::sync::add_history_entry;

use coin_store::{UtxoFilter, UtxoStore};
use contracts::options::{OPTION_SOURCE, OptionsArguments, finalize_options_transaction, get_options_program};
use contracts::sdk::taproot_pubkey_gen::{TaprootPubkeyGen, get_random_seed};
use options_relay::{ActionCompletedEvent, ActionType, OptionCreatedEvent};
use simplicityhl::elements::pset::serialize::Serialize;
use simplicityhl::elements::secp256k1_zkp::SECP256K1;
use simplicityhl::elements::{OutPoint, TxOut, TxOutSecrets};
use simplicityhl::simplicity::hex::DisplayHex;
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, derive_public_blinder_key};

impl Cli {
    #[allow(clippy::too_many_lines)]
    pub(crate) async fn run_option(&self, config: Config, command: &OptionCommand) -> Result<(), Error> {
        let wallet = self.get_wallet(&config).await?;

        match command {
            OptionCommand::Create {
                collateral_asset,
                total_collateral,
                num_contracts,
                settlement_asset,
                total_strike,
                expiry,
                fee,
                broadcast,
            } => {
                println!("Creating option contract...");

                if *num_contracts == 0 {
                    return Err(Error::Config("num-contracts must be greater than 0".to_string()));
                }
                if *total_collateral % *num_contracts != 0 {
                    return Err(Error::Config(format!(
                        "total-collateral ({total_collateral}) must be divisible by num-contracts ({num_contracts})"
                    )));
                }
                if *total_strike % *num_contracts != 0 {
                    return Err(Error::Config(format!(
                        "total-strike ({total_strike}) must be divisible by num-contracts ({num_contracts})"
                    )));
                }

                let collateral_per_contract = *total_collateral / *num_contracts;
                let settlement_per_contract = *total_strike / *num_contracts;

                let expiry_time = parse_expiry(expiry)?;
                let start_time = current_timestamp();

                println!("  Total collateral: {total_collateral} of {collateral_asset}");
                println!("  Total strike: {total_strike} of {settlement_asset}");
                println!("  Number of contracts: {num_contracts}");
                println!("  Per-contract collateral: {collateral_per_contract}");
                println!("  Per-contract strike: {settlement_per_contract}");
                println!("  Expiry: {} ({})", expiry, format_relative_time(expiry_time));

                let script_pubkey = wallet.signer().p2pk_address(config.address_params())?.script_pubkey();
                let is_lbtc_collateral = *collateral_asset == *LIQUID_TESTNET_BITCOIN_ASSET;

                let initial_fee = fee.unwrap_or(PLACEHOLDER_FEE);

                let lbtc_required = if is_lbtc_collateral {
                    initial_fee * 3 + *total_collateral
                } else {
                    initial_fee * 3
                };

                let lbtc_fee_filter = UtxoFilter::new()
                    .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                    .script_pubkey(script_pubkey.clone())
                    .required_value(lbtc_required)
                    .limit(3);

                let lbtc_results = <_ as UtxoStore>::query_utxos(wallet.store(), &[lbtc_fee_filter]).await?;
                let lbtc_entries = extract_entries_from_result(&lbtc_results[0]);

                if lbtc_entries.len() < 3 {
                    return Err(Error::Config(
                        "Need at least 3 LBTC UTXOs for option creation. Use 'tx split-native' first.".to_string(),
                    ));
                }

                let coll_query_results;

                let (collateral_outpoint, collateral_txout, funding_fee_utxo) = if is_lbtc_collateral {
                    (*lbtc_entries[2].outpoint(), lbtc_entries[2].txout().clone(), None)
                } else {
                    let collateral_filter = UtxoFilter::new()
                        .asset_id(*collateral_asset)
                        .script_pubkey(script_pubkey.clone())
                        .required_value(*total_collateral);
                    coll_query_results = <_ as UtxoStore>::query_utxos(wallet.store(), &[collateral_filter]).await?;

                    let coll_entries = extract_entries_from_result(&coll_query_results[0]);
                    let coll_entry = coll_entries.first().ok_or_else(|| {
                        Error::Config(format!("No UTXOs found for collateral asset {collateral_asset}"))
                    })?;

                    (
                        *coll_entry.outpoint(),
                        coll_entry.txout().clone(),
                        Some((*lbtc_entries[2].outpoint(), lbtc_entries[2].txout().clone())),
                    )
                };

                let first_fee_utxo = (*lbtc_entries[0].outpoint(), lbtc_entries[0].txout().clone());
                let second_fee_utxo = (*lbtc_entries[1].outpoint(), lbtc_entries[1].txout().clone());

                let issuance_asset_entropy = get_random_seed();

                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let args = OptionsArguments::new(
                    start_time as u32,
                    expiry_time as u32,
                    collateral_per_contract,
                    settlement_per_contract,
                    *collateral_asset,
                    *settlement_asset,
                    issuance_asset_entropy,
                    (first_fee_utxo.0, first_fee_utxo.1.value.is_confidential()),
                    (second_fee_utxo.0, second_fee_utxo.1.value.is_confidential()),
                );

                let blinding_keypair = derive_public_blinder_key();

                let actual_fee = estimate_fee_signed(
                    fee.as_ref(),
                    config.get_fee_rate(),
                    |f| {
                        let (pst, _) = contracts::sdk::build_option_creation(
                            &blinding_keypair.public_key(),
                            first_fee_utxo.clone(),
                            second_fee_utxo.clone(),
                            &args,
                            issuance_asset_entropy,
                            f,
                            config.address_params(),
                        )?;
                        Ok((pst, vec![first_fee_utxo.1.clone(), second_fee_utxo.1.clone()]))
                    },
                    |tx, utxos| sign_p2pk_inputs(tx, utxos, &wallet, config.address_params(), 0),
                )?;

                println!("  Fee: {actual_fee} sats per transaction");

                let (pst, taproot_pubkey_gen) = contracts::sdk::build_option_creation(
                    &blinding_keypair.public_key(),
                    first_fee_utxo.clone(),
                    second_fee_utxo.clone(),
                    &args,
                    issuance_asset_entropy,
                    actual_fee,
                    config.address_params(),
                )?;

                let creation_tx = pst.extract_tx()?;

                let option_secrets: TxOutSecrets = creation_tx.output[0]
                    .unblind(SECP256K1, blinding_keypair.secret_key())
                    .map_err(|e| Error::Config(format!("Failed to unblind option token output: {e}")))?;
                let grantor_secrets: TxOutSecrets = creation_tx.output[1]
                    .unblind(SECP256K1, blinding_keypair.secret_key())
                    .map_err(|e| Error::Config(format!("Failed to unblind grantor token output: {e}")))?;
                let creation_utxos = vec![first_fee_utxo.1.clone(), second_fee_utxo.1.clone()];

                let creation_tx = sign_p2pk_inputs(creation_tx, &creation_utxos, &wallet, config.address_params(), 0)?;

                let creation_txid = creation_tx.txid();

                let option_token_utxo = (
                    OutPoint::new(creation_txid, 0),
                    creation_tx.output[0].clone(),
                    option_secrets,
                );
                let grantor_token_utxo = (
                    OutPoint::new(creation_txid, 1),
                    creation_tx.output[1].clone(),
                    grantor_secrets,
                );
                let collateral_utxo = (collateral_outpoint, collateral_txout);

                let (funding_pst, option_branch) = contracts::sdk::build_option_funding(
                    &blinding_keypair.public_key(),
                    option_token_utxo.clone(),
                    grantor_token_utxo.clone(),
                    collateral_utxo.clone(),
                    funding_fee_utxo.as_ref(),
                    &args,
                    *total_collateral,
                    actual_fee,
                )?;

                let mut funding_tx = funding_pst.extract_tx()?;
                let mut funding_utxos: Vec<TxOut> = vec![
                    option_token_utxo.1.clone(),
                    grantor_token_utxo.1.clone(),
                    collateral_utxo.1.clone(),
                ];
                if let Some((_, fee_txout)) = &funding_fee_utxo {
                    funding_utxos.push(fee_txout.clone());
                }

                let options_program = get_options_program(&args)?;
                for i in 0..2 {
                    funding_tx = finalize_options_transaction(
                        funding_tx,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &options_program,
                        &funding_utxos,
                        i,
                        option_branch,
                        config.address_params(),
                        *LIQUID_TESTNET_GENESIS,
                    )?;
                }

                let funding_tx = sign_p2pk_inputs(funding_tx, &funding_utxos, &wallet, config.address_params(), 2)?;

                if *broadcast {
                    cli_helper::explorer::broadcast_tx(&creation_tx).await?;
                    println!("Creation tx: {}", creation_tx.txid());

                    cli_helper::explorer::broadcast_tx(&funding_tx).await?;
                    println!("Funding tx: {}", funding_tx.txid());

                    let publishing_client = self.get_publishing_client(&config).await?;
                    let funding_outpoint = OutPoint::new(funding_tx.txid(), 0);
                    let option_event =
                        OptionCreatedEvent::new(args.clone(), funding_outpoint, taproot_pubkey_gen.clone());
                    let nostr_event_id = publishing_client.publish_option_created(&option_event).await?;
                    println!("Published to NOSTR: {nostr_event_id}");

                    let funded_action =
                        ActionCompletedEvent::new(nostr_event_id, ActionType::OptionFunded, funding_outpoint);
                    let funded_event_id = publishing_client.publish_action_completed(&funded_action).await?;
                    println!("Published funding action: {funded_event_id}");

                    let history = vec![
                        HistoryEntry::with_txid_and_nostr(
                            ActionType::OptionCreated.as_str(),
                            &creation_tx.txid().to_string(),
                            &nostr_event_id.to_hex(),
                            start_time,
                        ),
                        HistoryEntry::with_txid_and_nostr(
                            ActionType::OptionFunded.as_str(),
                            &funding_tx.txid().to_string(),
                            &funded_event_id.to_hex(),
                            start_time,
                        ),
                    ];

                    let metadata = ContractMetadata::from_nostr_with_history(
                        nostr_event_id.to_hex(),
                        publishing_client.public_key().await?.to_hex(),
                        start_time,
                        history,
                    );
                    let metadata_bytes = metadata.to_bytes()?;

                    wallet
                        .store()
                        .add_contract(
                            OPTION_SOURCE,
                            args.build_option_arguments(),
                            taproot_pubkey_gen.clone(),
                            Some(&metadata_bytes),
                        )
                        .await?;

                    let mut blinder_keys = HashMap::new();
                    blinder_keys.insert(0, blinding_keypair);
                    wallet
                        .store()
                        .insert_transaction(&creation_tx, blinder_keys.clone())
                        .await?;
                    blinder_keys.insert(1, blinding_keypair);
                    wallet.store().insert_transaction(&funding_tx, blinder_keys).await?;

                    let (option_token_id, _) = args.get_option_token_ids();
                    let (grantor_token_id, _) = args.get_grantor_token_ids();

                    wallet
                        .store()
                        .insert_contract_token(&taproot_pubkey_gen, option_token_id, OPTION_TOKEN_TAG)
                        .await?;
                    wallet
                        .store()
                        .insert_contract_token(&taproot_pubkey_gen, grantor_token_id, GRANTOR_TOKEN_TAG)
                        .await?;

                    println!("  Option token: {option_token_id}");
                    println!("  Grantor token: {grantor_token_id}");
                    println!("  Contract address: {}", taproot_pubkey_gen.address);

                    publishing_client.disconnect().await;
                } else {
                    println!("Creation tx: {}", creation_tx.serialize().to_lower_hex_string());
                    println!("Funding tx: {}", funding_tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
            OptionCommand::Exercise {
                option_token,
                fee,
                broadcast,
            } => {
                println!("Exercising option...");

                let script_pubkey = wallet.signer().p2pk_address(config.address_params())?.script_pubkey();
                let option_entries = get_option_tokens_from_wallet(&wallet, OPTION_SOURCE, &script_pubkey).await?;
                if option_entries.is_empty() {
                    return Err(Error::Config("No option contract tokens found".to_string()));
                }

                let mut contracts_with_collateral = std::collections::HashSet::new();
                let mut checked_contracts = std::collections::HashSet::new();

                for entry in &option_entries {
                    if checked_contracts.contains(&entry.taproot_pubkey_gen_str) {
                        continue;
                    }
                    checked_contracts.insert(entry.taproot_pubkey_gen_str.clone());

                    let tpg = TaprootPubkeyGen::build_from_str(
                        &entry.taproot_pubkey_gen_str,
                        &entry.option_arguments,
                        wallet.params(),
                        &contracts::options::get_options_address,
                    )?;

                    let collateral_asset_id = entry.option_arguments.get_collateral_asset_id();
                    let collateral_filter = UtxoFilter::new().taproot_pubkey_gen(tpg).asset_id(collateral_asset_id);

                    if let Ok(results) = <_ as UtxoStore>::query_utxos(wallet.store(), &[collateral_filter]).await {
                        let collateral_entries = extract_entries_from_result(&results[0]);
                        if !collateral_entries.is_empty() {
                            contracts_with_collateral.insert(entry.taproot_pubkey_gen_str.clone());
                        }
                    }
                }

                let mut seen_contracts = std::collections::HashSet::new();
                let entries_with_collateral: Vec<_> = option_entries
                    .into_iter()
                    .filter(|e| {
                        if contracts_with_collateral.contains(&e.taproot_pubkey_gen_str)
                            && !seen_contracts.contains(&e.taproot_pubkey_gen_str)
                        {
                            seen_contracts.insert(e.taproot_pubkey_gen_str.clone());
                            true
                        } else {
                            false
                        }
                    })
                    .collect();

                if entries_with_collateral.is_empty() {
                    return Err(Error::Config(
                        "No exercisable options found. Collateral may have been expired/claimed, \
                        or not yet synced. Run 'sync full' to update state."
                            .to_string(),
                    ));
                }

                let enriched_entry = if let Some(_outpoint) = option_token {
                    entries_with_collateral
                        .iter()
                        .find(|e| Some(e.entry.outpoint()) == option_token.as_ref())
                        .ok_or_else(|| Error::Config("Option token not found or no collateral available".to_string()))?
                } else {
                    println!("  (Showing one entry per contract with collateral available)");
                    select_enriched_token_interactive(
                        &entries_with_collateral,
                        "Select contract to exercise options from",
                    )?
                };

                let option_entry = &enriched_entry.entry;
                let option_arguments = enriched_entry.option_arguments.clone();
                let taproot_pubkey_gen = TaprootPubkeyGen::build_from_str(
                    &enriched_entry.taproot_pubkey_gen_str,
                    &option_arguments,
                    wallet.params(),
                    &contracts::options::get_options_address,
                )?;

                let option_token_amount = option_entry.value().unwrap_or(0);
                println!("  Option tokens available: {option_token_amount}");

                let amount_to_burn = prompt_amount("Amount of option tokens to exercise").map_err(Error::Io)?;

                if amount_to_burn > option_token_amount {
                    return Err(Error::Config(format!(
                        "Cannot burn {amount_to_burn} tokens, only {option_token_amount} available"
                    )));
                }

                println!("  Burning: {amount_to_burn} option tokens");

                let fee_filter = UtxoFilter::new()
                    .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                    .script_pubkey(script_pubkey.clone())
                    .required_value(fee.unwrap_or(PLACEHOLDER_FEE));

                let settlement_asset_id = option_arguments.get_settlement_asset_id();
                let settlement_required = amount_to_burn * option_arguments.settlement_per_contract();

                let settlement_filter = UtxoFilter::new()
                    .asset_id(settlement_asset_id)
                    .script_pubkey(script_pubkey.clone())
                    .required_value(settlement_required);

                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[fee_filter, settlement_filter]).await?;
                let fee_entries = extract_entries_from_result(&results[0]);
                let settlement_entries = extract_entries_from_result(&results[1]);

                if fee_entries.is_empty() {
                    return Err(Error::Config("No LBTC UTXOs found for fee".to_string()));
                }
                if settlement_entries.is_empty() {
                    return Err(Error::Config(format!(
                        "No settlement asset UTXOs found. Need {settlement_required} of {settlement_asset_id}"
                    )));
                }

                let fee_utxo = &fee_entries[0];
                let settlement_utxo = &settlement_entries[0];

                let collateral_filter = UtxoFilter::new()
                    .taproot_pubkey_gen(taproot_pubkey_gen.clone())
                    .asset_id(option_arguments.get_collateral_asset_id());

                let collateral_results = <_ as UtxoStore>::query_utxos(wallet.store(), &[collateral_filter]).await?;
                let collateral_entries = extract_entries_from_result(&collateral_results[0]);

                if collateral_entries.is_empty() {
                    return Err(Error::Config("No collateral found at contract address".to_string()));
                }

                let collateral_entry = &collateral_entries[0];

                let collateral_input = (*collateral_entry.outpoint(), collateral_entry.txout().clone());
                let option_input = (*option_entry.outpoint(), option_entry.txout().clone());
                let settlement_input = (*settlement_utxo.outpoint(), settlement_utxo.txout().clone());
                let fee_input = (*fee_utxo.outpoint(), fee_utxo.txout().clone());

                let actual_fee = if let Some(f) = fee {
                    *f
                } else {
                    let (pst, branch) = contracts::sdk::build_option_exercise(
                        collateral_input.clone(),
                        option_input.clone(),
                        settlement_input.clone(),
                        fee_input.clone(),
                        amount_to_burn,
                        PLACEHOLDER_FEE,
                        &option_arguments,
                    )?;
                    let mut tx = pst.extract_tx()?;
                    let utxos = vec![
                        collateral_input.1.clone(),
                        option_input.1.clone(),
                        settlement_input.1.clone(),
                        fee_input.1.clone(),
                    ];
                    let options_program = get_options_program(&option_arguments)?;
                    tx = finalize_options_transaction(
                        tx,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &options_program,
                        &utxos,
                        0,
                        branch,
                        config.address_params(),
                        *LIQUID_TESTNET_GENESIS,
                    )?;
                    let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.address_params(), 1)?;
                    let signed_weight = tx.weight();
                    let fee_rate = config.get_fee_rate();
                    let estimated = crate::fee::calculate_fee(signed_weight, fee_rate);
                    println!(
                        "Estimated fee: {estimated} sats (signed weight: {signed_weight}, rate: {fee_rate} sats/kvb)"
                    );
                    estimated
                };

                println!("  Fee: {actual_fee} sats");

                let (pst, option_branch) = contracts::sdk::build_option_exercise(
                    collateral_input.clone(),
                    option_input.clone(),
                    settlement_input.clone(),
                    fee_input.clone(),
                    amount_to_burn,
                    actual_fee,
                    &option_arguments,
                )?;

                let mut tx = pst.extract_tx()?;
                let utxos = vec![collateral_input.1, option_input.1, settlement_input.1, fee_input.1];

                let options_program = get_options_program(&option_arguments)?;
                tx = finalize_options_transaction(
                    tx,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &options_program,
                    &utxos,
                    0,
                    option_branch,
                    config.address_params(),
                    *LIQUID_TESTNET_GENESIS,
                )?;

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.address_params(), 1)?;

                if *broadcast {
                    cli_helper::explorer::broadcast_tx(&tx).await?;
                    println!("Broadcasted: {}", tx.txid());

                    if let Some(metadata) =
                        crate::sync::get_contract_metadata(wallet.store(), &taproot_pubkey_gen).await?
                        && let Some(ref nostr_event_id) = metadata.nostr_event_id
                        && let Ok(event_id) = nostr::EventId::from_hex(nostr_event_id)
                    {
                        let publishing_client = self.get_publishing_client(&config).await?;

                        let action_event = ActionCompletedEvent::new(
                            event_id,
                            ActionType::OptionExercised,
                            OutPoint::new(tx.txid(), 0),
                        );

                        let published_id = publishing_client.publish_action_completed(&action_event).await?;
                        println!("Published action to NOSTR: {published_id}");

                        publishing_client.disconnect().await;
                    }

                    wallet.store().insert_transaction(&tx, HashMap::default()).await?;

                    let entry = HistoryEntry::with_txid(
                        ActionType::OptionExercised.as_str(),
                        &tx.txid().to_string(),
                        current_timestamp(),
                    );
                    add_history_entry(wallet.store(), &taproot_pubkey_gen, entry).await?;
                } else {
                    println!("{}", tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
            OptionCommand::Expire {
                grantor_token,
                fee,
                broadcast,
            } => {
                println!("Expiring option...");

                let script_pubkey = wallet.signer().p2pk_address(config.address_params())?.script_pubkey();
                let grantor_entries = get_grantor_tokens_from_wallet(&wallet, OPTION_SOURCE, &script_pubkey).await?;
                if grantor_entries.is_empty() {
                    return Err(Error::Config("No grantor tokens found".to_string()));
                }

                let mut contracts_with_collateral = std::collections::HashSet::new();
                let mut checked_contracts = std::collections::HashSet::new();

                for entry in &grantor_entries {
                    if checked_contracts.contains(&entry.taproot_pubkey_gen_str) {
                        continue;
                    }
                    checked_contracts.insert(entry.taproot_pubkey_gen_str.clone());

                    let tpg = TaprootPubkeyGen::build_from_str(
                        &entry.taproot_pubkey_gen_str,
                        &entry.option_arguments,
                        wallet.params(),
                        &contracts::options::get_options_address,
                    )?;

                    let collateral_asset_id = entry.option_arguments.get_collateral_asset_id();
                    let collateral_filter = UtxoFilter::new().taproot_pubkey_gen(tpg).asset_id(collateral_asset_id);

                    if let Ok(results) = <_ as UtxoStore>::query_utxos(wallet.store(), &[collateral_filter]).await {
                        let collateral_entries = extract_entries_from_result(&results[0]);
                        if !collateral_entries.is_empty() {
                            contracts_with_collateral.insert(entry.taproot_pubkey_gen_str.clone());
                        }
                    }
                }

                let mut seen_contracts = std::collections::HashSet::new();
                let entries_with_collateral: Vec<_> = grantor_entries
                    .into_iter()
                    .filter(|e| {
                        if contracts_with_collateral.contains(&e.taproot_pubkey_gen_str)
                            && !seen_contracts.contains(&e.taproot_pubkey_gen_str)
                        {
                            seen_contracts.insert(e.taproot_pubkey_gen_str.clone());
                            true
                        } else {
                            false
                        }
                    })
                    .collect();

                if entries_with_collateral.is_empty() {
                    return Err(Error::Config(
                        "No expirable options found. Collateral may have already been claimed, \
                        exercised, or not yet synced. Run 'sync full' to update state."
                            .to_string(),
                    ));
                }

                let enriched_entry = if let Some(outpoint) = grantor_token {
                    entries_with_collateral
                        .iter()
                        .find(|e| e.entry.outpoint() == outpoint)
                        .ok_or_else(|| {
                            Error::Config("Grantor token not found or no collateral available".to_string())
                        })?
                } else {
                    println!("  (Showing one entry per contract with collateral available)");
                    select_enriched_token_interactive(
                        &entries_with_collateral,
                        "Select contract to expire options from",
                    )?
                };

                let grantor_entry = &enriched_entry.entry;
                let option_arguments = enriched_entry.option_arguments.clone();
                let taproot_pubkey_gen = TaprootPubkeyGen::build_from_str(
                    &enriched_entry.taproot_pubkey_gen_str,
                    &option_arguments,
                    wallet.params(),
                    &contracts::options::get_options_address,
                )?;

                let grantor_token_amount = grantor_entry.value().unwrap_or(0);
                println!("  Grantor tokens available: {grantor_token_amount}");
                let amount_to_burn = prompt_amount("Amount of grantor tokens to burn for expiry").map_err(Error::Io)?;

                if amount_to_burn > grantor_token_amount {
                    return Err(Error::Config(format!(
                        "Cannot burn {amount_to_burn} tokens, only {grantor_token_amount} available"
                    )));
                }

                println!("  Grantor token: {}", grantor_entry.outpoint());
                println!("  Burning: {amount_to_burn} grantor tokens");

                let initial_fee = fee.unwrap_or(PLACEHOLDER_FEE);
                let fee_filter = UtxoFilter::new()
                    .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                    .script_pubkey(script_pubkey.clone())
                    .required_value(initial_fee);

                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[fee_filter]).await?;
                let fee_entries = extract_entries_from_result(&results[0]);

                if fee_entries.is_empty() {
                    return Err(Error::Config("No LBTC UTXOs found for fee".to_string()));
                }

                let fee_utxo = &fee_entries[0];

                let collateral_filter = UtxoFilter::new()
                    .taproot_pubkey_gen(taproot_pubkey_gen.clone())
                    .asset_id(option_arguments.get_collateral_asset_id());

                let collateral_results = <_ as UtxoStore>::query_utxos(wallet.store(), &[collateral_filter]).await?;
                let collateral_entries = extract_entries_from_result(&collateral_results[0]);

                if collateral_entries.is_empty() {
                    return Err(Error::Config(format!(
                        "No collateral found at contract address {}. \
                        The collateral may have been exercised, already expired, or not yet synced. \
                        Try running 'sync full' to update your local state.",
                        taproot_pubkey_gen.address
                    )));
                }

                let collateral_entry = &collateral_entries[0];

                let collateral_input = (*collateral_entry.outpoint(), collateral_entry.txout().clone());
                let grantor_input = (*grantor_entry.outpoint(), grantor_entry.txout().clone());
                let fee_input = (*fee_utxo.outpoint(), fee_utxo.txout().clone());

                let actual_fee = if let Some(f) = fee {
                    *f
                } else {
                    let (pst, branch) = contracts::sdk::build_option_expiry(
                        collateral_input.clone(),
                        grantor_input.clone(),
                        fee_input.clone(),
                        amount_to_burn,
                        PLACEHOLDER_FEE,
                        &option_arguments,
                    )?;
                    let mut tx = pst.extract_tx()?;
                    let utxos = vec![collateral_input.1.clone(), grantor_input.1.clone(), fee_input.1.clone()];
                    let options_program = get_options_program(&option_arguments)?;
                    tx = finalize_options_transaction(
                        tx,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &options_program,
                        &utxos,
                        0,
                        branch,
                        config.address_params(),
                        *LIQUID_TESTNET_GENESIS,
                    )?;
                    let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.address_params(), 1)?;
                    let signed_weight = tx.weight();
                    let fee_rate = config.get_fee_rate();
                    let estimated = crate::fee::calculate_fee(signed_weight, fee_rate);
                    println!(
                        "Estimated fee: {estimated} sats (signed weight: {signed_weight}, rate: {fee_rate} sats/kvb)"
                    );
                    estimated
                };

                println!("  Fee: {actual_fee} sats");

                let (pst, option_branch) = contracts::sdk::build_option_expiry(
                    collateral_input.clone(),
                    grantor_input.clone(),
                    fee_input.clone(),
                    amount_to_burn,
                    actual_fee,
                    &option_arguments,
                )?;

                let mut tx = pst.extract_tx()?;
                let utxos = vec![collateral_input.1, grantor_input.1, fee_input.1];

                let options_program = get_options_program(&option_arguments)?;
                tx = finalize_options_transaction(
                    tx,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &options_program,
                    &utxos,
                    0,
                    option_branch,
                    config.address_params(),
                    *LIQUID_TESTNET_GENESIS,
                )?;

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.address_params(), 1)?;

                if *broadcast {
                    cli_helper::explorer::broadcast_tx(&tx).await?;
                    println!("Broadcasted: {}", tx.txid());

                    if let Some(metadata) =
                        crate::sync::get_contract_metadata(wallet.store(), &taproot_pubkey_gen).await?
                        && let Some(ref nostr_event_id) = metadata.nostr_event_id
                        && let Ok(event_id) = nostr::EventId::from_hex(nostr_event_id)
                    {
                        let publishing_client = self.get_publishing_client(&config).await?;

                        let action_event =
                            ActionCompletedEvent::new(event_id, ActionType::OptionExpired, OutPoint::new(tx.txid(), 0));

                        let published_id = publishing_client.publish_action_completed(&action_event).await?;
                        println!("Published action to NOSTR: {published_id}");

                        publishing_client.disconnect().await;
                    }

                    wallet.store().insert_transaction(&tx, HashMap::default()).await?;

                    let entry = HistoryEntry::with_txid(
                        ActionType::OptionExpired.as_str(),
                        &tx.txid().to_string(),
                        current_timestamp(),
                    );
                    add_history_entry(wallet.store(), &taproot_pubkey_gen, entry).await?;
                } else {
                    println!("{}", tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
            OptionCommand::Settlement {
                grantor_token,
                fee,
                broadcast,
            } => {
                println!("Claiming settlement...");

                let script_pubkey = wallet.signer().p2pk_address(config.address_params())?.script_pubkey();
                let grantor_entries = get_grantor_tokens_from_wallet(&wallet, OPTION_SOURCE, &script_pubkey).await?;
                if grantor_entries.is_empty() {
                    return Err(Error::Config("No grantor tokens found".to_string()));
                }

                let mut contracts_with_settlement = std::collections::HashSet::new();
                let mut checked_contracts = std::collections::HashSet::new();

                for entry in &grantor_entries {
                    if checked_contracts.contains(&entry.taproot_pubkey_gen_str) {
                        continue;
                    }
                    checked_contracts.insert(entry.taproot_pubkey_gen_str.clone());

                    let tpg = TaprootPubkeyGen::build_from_str(
                        &entry.taproot_pubkey_gen_str,
                        &entry.option_arguments,
                        wallet.params(),
                        &contracts::options::get_options_address,
                    )?;

                    let settlement_asset_id = entry.option_arguments.get_settlement_asset_id();
                    let settlement_filter = UtxoFilter::new().taproot_pubkey_gen(tpg).asset_id(settlement_asset_id);

                    if let Ok(results) = <_ as UtxoStore>::query_utxos(wallet.store(), &[settlement_filter]).await {
                        let settlement_entries = extract_entries_from_result(&results[0]);
                        if !settlement_entries.is_empty() {
                            contracts_with_settlement.insert(entry.taproot_pubkey_gen_str.clone());
                        }
                    }
                }

                let mut seen_contracts = std::collections::HashSet::new();
                let entries_with_settlement: Vec<_> = grantor_entries
                    .into_iter()
                    .filter(|e| {
                        if contracts_with_settlement.contains(&e.taproot_pubkey_gen_str)
                            && !seen_contracts.contains(&e.taproot_pubkey_gen_str)
                        {
                            seen_contracts.insert(e.taproot_pubkey_gen_str.clone());
                            true
                        } else {
                            false
                        }
                    })
                    .collect();

                if entries_with_settlement.is_empty() {
                    return Err(Error::Config(
                        "No settlement available. Options may not have been exercised yet, \
                        or all settlement has been claimed. Run 'sync full' to update state."
                            .to_string(),
                    ));
                }

                let enriched_entry = if let Some(outpoint) = grantor_token {
                    entries_with_settlement
                        .iter()
                        .find(|e| e.entry.outpoint() == outpoint)
                        .ok_or_else(|| {
                            Error::Config("Grantor token not found or no settlement available".to_string())
                        })?
                } else {
                    println!("  (Showing one entry per contract with settlement available)");
                    select_enriched_token_interactive(
                        &entries_with_settlement,
                        "Select contract to claim settlement from",
                    )?
                };

                let grantor_entry = &enriched_entry.entry;
                let option_arguments = enriched_entry.option_arguments.clone();
                let taproot_pubkey_gen = TaprootPubkeyGen::build_from_str(
                    &enriched_entry.taproot_pubkey_gen_str,
                    &option_arguments,
                    wallet.params(),
                    &contracts::options::get_options_address,
                )?;

                let grantor_token_amount = grantor_entry.value().unwrap_or(0);
                println!("  Grantor tokens available: {grantor_token_amount}");
                let amount_to_burn =
                    prompt_amount("Amount of grantor tokens to burn for settlement").map_err(Error::Io)?;

                if amount_to_burn > grantor_token_amount {
                    return Err(Error::Config(format!(
                        "Cannot burn {amount_to_burn} tokens, only {grantor_token_amount} available"
                    )));
                }

                println!("  Grantor token: {}", grantor_entry.outpoint());
                println!("  Burning: {amount_to_burn} grantor tokens");

                let initial_fee = fee.unwrap_or(PLACEHOLDER_FEE);
                let fee_filter = UtxoFilter::new()
                    .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                    .script_pubkey(script_pubkey.clone())
                    .required_value(initial_fee);

                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[fee_filter]).await?;
                let fee_entries = extract_entries_from_result(&results[0]);

                if fee_entries.is_empty() {
                    return Err(Error::Config("No LBTC UTXOs found for fee".to_string()));
                }

                let fee_utxo = &fee_entries[0];

                let settlement_asset_id = option_arguments.get_settlement_asset_id();
                let settlement_filter = UtxoFilter::new()
                    .taproot_pubkey_gen(taproot_pubkey_gen.clone())
                    .asset_id(settlement_asset_id);

                let settlement_results = <_ as UtxoStore>::query_utxos(wallet.store(), &[settlement_filter]).await?;
                let settlement_entries = extract_entries_from_result(&settlement_results[0]);

                if settlement_entries.is_empty() {
                    return Err(Error::Config(format!(
                        "No settlement asset found at contract address {}. \
                        Options may not have been exercised yet, or settlement was already claimed. \
                        Try running 'sync full' to update your local state.",
                        taproot_pubkey_gen.address
                    )));
                }

                let settlement_entry = &settlement_entries[0];
                let settlement_available = settlement_entry.value().unwrap_or(0);
                let settlement_needed = amount_to_burn * option_arguments.settlement_per_contract();

                println!("  Settlement available at contract: {settlement_available}");
                println!("  Settlement to claim: {settlement_needed}");

                if settlement_needed > settlement_available {
                    return Err(Error::Config(format!(
                        "Insufficient settlement at contract. Need {settlement_needed}, available {settlement_available}"
                    )));
                }

                let settlement_input = (*settlement_entry.outpoint(), settlement_entry.txout().clone());
                let grantor_input = (*grantor_entry.outpoint(), grantor_entry.txout().clone());
                let fee_input = (*fee_utxo.outpoint(), fee_utxo.txout().clone());

                let actual_fee = if let Some(f) = fee {
                    *f
                } else {
                    let (pst, branch) = contracts::sdk::build_option_settlement(
                        settlement_input.clone(),
                        grantor_input.clone(),
                        fee_input.clone(),
                        amount_to_burn,
                        PLACEHOLDER_FEE,
                        &option_arguments,
                    )?;
                    let mut tx = pst.extract_tx()?;
                    let utxos = vec![settlement_input.1.clone(), grantor_input.1.clone(), fee_input.1.clone()];
                    let options_program = get_options_program(&option_arguments)?;
                    tx = finalize_options_transaction(
                        tx,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &options_program,
                        &utxos,
                        0,
                        branch,
                        config.address_params(),
                        *LIQUID_TESTNET_GENESIS,
                    )?;
                    let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.address_params(), 1)?;
                    let signed_weight = tx.weight();
                    let fee_rate = config.get_fee_rate();
                    let estimated = crate::fee::calculate_fee(signed_weight, fee_rate);
                    println!(
                        "Estimated fee: {estimated} sats (signed weight: {signed_weight}, rate: {fee_rate} sats/kvb)"
                    );
                    estimated
                };

                println!("  Fee: {actual_fee} sats");

                let (pst, option_branch) = contracts::sdk::build_option_settlement(
                    settlement_input.clone(),
                    grantor_input.clone(),
                    fee_input.clone(),
                    amount_to_burn,
                    actual_fee,
                    &option_arguments,
                )?;

                let mut tx = pst.extract_tx()?;
                let utxos = vec![settlement_input.1, grantor_input.1, fee_input.1];

                let options_program = get_options_program(&option_arguments)?;
                tx = finalize_options_transaction(
                    tx,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &options_program,
                    &utxos,
                    0,
                    option_branch,
                    config.address_params(),
                    *LIQUID_TESTNET_GENESIS,
                )?;

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.address_params(), 1)?;

                if *broadcast {
                    cli_helper::explorer::broadcast_tx(&tx).await?;
                    println!("Broadcasted: {}", tx.txid());

                    if let Some(metadata) =
                        crate::sync::get_contract_metadata(wallet.store(), &taproot_pubkey_gen).await?
                        && let Some(ref nostr_event_id) = metadata.nostr_event_id
                        && let Ok(event_id) = nostr::EventId::from_hex(nostr_event_id)
                    {
                        let publishing_client = self.get_publishing_client(&config).await?;

                        let action_event = ActionCompletedEvent::new(
                            event_id,
                            ActionType::SettlementClaimed,
                            OutPoint::new(tx.txid(), 0),
                        );

                        let published_id = publishing_client.publish_action_completed(&action_event).await?;
                        println!("Published action to NOSTR: {published_id}");

                        publishing_client.disconnect().await;
                    }

                    wallet.store().insert_transaction(&tx, HashMap::default()).await?;

                    let entry = HistoryEntry::with_txid(
                        ActionType::SettlementClaimed.as_str(),
                        &tx.txid().to_string(),
                        current_timestamp(),
                    );
                    add_history_entry(wallet.store(), &taproot_pubkey_gen, entry).await?;
                } else {
                    println!("{}", tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
            OptionCommand::Cancel {
                option_token,
                fee,
                broadcast,
            } => {
                println!("Cancelling option...");

                let user_script_pubkey = wallet.signer().p2pk_address(config.address_params())?.script_pubkey();
                let token_entries = get_option_tokens_from_wallet(&wallet, OPTION_SOURCE, &user_script_pubkey).await?;
                if token_entries.is_empty() {
                    return Err(Error::Config("No option tokens found".to_string()));
                }

                let enriched_entry = match option_token {
                    Some(outpoint) => token_entries
                        .iter()
                        .find(|e| e.entry.outpoint() == outpoint)
                        .ok_or_else(|| Error::Config("Option token not found in wallet".to_string()))?,
                    None => select_enriched_token_interactive(&token_entries, "Select option token to cancel")?,
                };

                let option_entry = &enriched_entry.entry;
                let option_arguments = enriched_entry.option_arguments.clone();
                let taproot_pubkey_gen = TaprootPubkeyGen::build_from_str(
                    &enriched_entry.taproot_pubkey_gen_str,
                    &option_arguments,
                    wallet.params(),
                    &contracts::options::get_options_address,
                )?;

                let (_option_token_id, _) = option_arguments.get_option_token_ids();
                let (grantor_token_id, _) = option_arguments.get_grantor_token_ids();

                let grantor_filter = UtxoFilter::new()
                    .asset_id(grantor_token_id)
                    .script_pubkey(user_script_pubkey.clone());

                let grantor_results = <_ as UtxoStore>::query_utxos(wallet.store(), &[grantor_filter]).await?;
                let grantor_entries = extract_entries_from_result(&grantor_results[0]);

                if grantor_entries.is_empty() {
                    return Err(Error::Config(
                        "Cannot cancel: need both option and grantor tokens. Grantor token not found.".to_string(),
                    ));
                }

                let grantor_entry = &grantor_entries[0];

                let option_token_amount = option_entry.value().unwrap_or(0);
                let grantor_token_amount = grantor_entry.value().unwrap_or(0);
                let max_burn = option_token_amount.min(grantor_token_amount);

                println!("  Option tokens available: {option_token_amount}");
                println!("  Grantor tokens available: {grantor_token_amount}");
                let amount_to_burn =
                    prompt_amount(&format!("Amount of tokens to burn (max {max_burn})")).map_err(Error::Io)?;

                if amount_to_burn > max_burn {
                    return Err(Error::Config(format!(
                        "Cannot burn {amount_to_burn} tokens, max available is {max_burn}"
                    )));
                }

                println!("  Burning: {amount_to_burn} tokens");

                let initial_fee = fee.unwrap_or(PLACEHOLDER_FEE);

                let script_pubkey = wallet.signer().p2pk_address(config.address_params())?.script_pubkey();
                let fee_filter = UtxoFilter::new()
                    .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                    .script_pubkey(script_pubkey.clone())
                    .required_value(initial_fee);

                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[fee_filter]).await?;
                let fee_entries = extract_entries_from_result(&results[0]);

                if fee_entries.is_empty() {
                    return Err(Error::Config("No LBTC UTXOs found for fee".to_string()));
                }

                let fee_utxo = &fee_entries[0];

                let collateral_filter = UtxoFilter::new()
                    .taproot_pubkey_gen(taproot_pubkey_gen.clone())
                    .asset_id(option_arguments.get_collateral_asset_id());

                let collateral_results = <_ as UtxoStore>::query_utxos(wallet.store(), &[collateral_filter]).await?;
                let collateral_entries = extract_entries_from_result(&collateral_results[0]);

                if collateral_entries.is_empty() {
                    return Err(Error::Config("No collateral found at contract address".to_string()));
                }

                let collateral_entry = &collateral_entries[0];

                let collateral_input = (*collateral_entry.outpoint(), collateral_entry.txout().clone());
                let option_input = (*option_entry.outpoint(), option_entry.txout().clone());
                let grantor_input = (*grantor_entry.outpoint(), grantor_entry.txout().clone());
                let fee_input = (*fee_utxo.outpoint(), fee_utxo.txout().clone());

                let actual_fee = if let Some(f) = fee {
                    *f
                } else {
                    let (pst, branch) = contracts::sdk::build_option_cancellation(
                        collateral_input.clone(),
                        option_input.clone(),
                        grantor_input.clone(),
                        fee_input.clone(),
                        &option_arguments,
                        amount_to_burn,
                        PLACEHOLDER_FEE,
                    )?;
                    let mut tx = pst.extract_tx()?;
                    let utxos = vec![
                        collateral_input.1.clone(),
                        option_input.1.clone(),
                        grantor_input.1.clone(),
                        fee_input.1.clone(),
                    ];
                    let options_program = get_options_program(&option_arguments)?;
                    tx = finalize_options_transaction(
                        tx,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &options_program,
                        &utxos,
                        0,
                        branch,
                        config.address_params(),
                        *LIQUID_TESTNET_GENESIS,
                    )?;
                    let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.address_params(), 1)?;
                    let signed_weight = tx.weight();
                    let fee_rate = config.get_fee_rate();
                    let estimated = crate::fee::calculate_fee(signed_weight, fee_rate);
                    println!(
                        "Estimated fee: {estimated} sats (signed weight: {signed_weight}, rate: {fee_rate} sats/kvb)"
                    );
                    estimated
                };

                println!("  Fee: {actual_fee} sats");

                let (pst, option_branch) = contracts::sdk::build_option_cancellation(
                    collateral_input.clone(),
                    option_input.clone(),
                    grantor_input.clone(),
                    fee_input.clone(),
                    &option_arguments,
                    amount_to_burn,
                    actual_fee,
                )?;

                let mut tx = pst.extract_tx()?;
                let utxos = vec![collateral_input.1, option_input.1, grantor_input.1, fee_input.1];

                let options_program = get_options_program(&option_arguments)?;
                tx = finalize_options_transaction(
                    tx,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &options_program,
                    &utxos,
                    0,
                    option_branch,
                    config.address_params(),
                    *LIQUID_TESTNET_GENESIS,
                )?;

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.address_params(), 1)?;

                if *broadcast {
                    cli_helper::explorer::broadcast_tx(&tx).await?;
                    println!("Broadcasted: {}", tx.txid());

                    if let Some(metadata) =
                        crate::sync::get_contract_metadata(wallet.store(), &taproot_pubkey_gen).await?
                        && let Some(ref nostr_event_id) = metadata.nostr_event_id
                        && let Ok(event_id) = nostr::EventId::from_hex(nostr_event_id)
                    {
                        let publishing_client = self.get_publishing_client(&config).await?;

                        let action_event = ActionCompletedEvent::new(
                            event_id,
                            ActionType::OptionCancelled,
                            OutPoint::new(tx.txid(), 0),
                        );

                        let published_id = publishing_client.publish_action_completed(&action_event).await?;
                        println!("Published action to NOSTR: {published_id}");

                        publishing_client.disconnect().await;
                    }

                    wallet.store().insert_transaction(&tx, HashMap::default()).await?;

                    let entry = HistoryEntry::with_txid(
                        ActionType::OptionCancelled.as_str(),
                        &tx.txid().to_string(),
                        current_timestamp(),
                    );
                    add_history_entry(wallet.store(), &taproot_pubkey_gen, entry).await?;
                } else {
                    println!("{}", tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
        }
    }
}
