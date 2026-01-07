use crate::cli::interactive::{
    SWAP_COLLATERAL_TAG, current_timestamp, extract_entries_from_result, format_relative_time,
    get_grantor_tokens_from_wallet, parse_expiry, prompt_amount, select_enriched_token_interactive,
};
use crate::cli::{Cli, SwapCommand};
use crate::config::Config;
use crate::error::Error;
use crate::fee::{PLACEHOLDER_FEE, estimate_fee_signed};
use crate::metadata::{ContractMetadata, HistoryEntry};
use crate::signing::sign_p2pk_inputs;

use std::collections::HashMap;

use coin_store::{UtxoFilter, UtxoQueryResult, UtxoStore};
use contracts::options::OPTION_SOURCE;
use contracts::swap_with_change::{
    SWAP_WITH_CHANGE_SOURCE, SwapWithChangeArguments, finalize_swap_with_change_transaction,
    get_swap_with_change_program,
};
use options_relay::{ActionCompletedEvent, ActionType, SwapCreatedEvent};
use simplicityhl::elements::pset::serialize::Serialize;
use simplicityhl::simplicity::hex::DisplayHex;
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS};

impl Cli {
    #[allow(clippy::too_many_lines)]
    pub(crate) async fn run_swap(&self, config: Config, command: &SwapCommand) -> Result<(), Error> {
        let wallet = self.get_wallet(&config).await?;

        match command {
            SwapCommand::Create {
                grantor_token,
                premium_asset,
                premium_amount,
                expiry,
                fee,
                broadcast,
            } => {
                println!("Creating swap offer...");

                let user_script_pubkey = wallet.signer().p2pk_address(config.address_params())?.script_pubkey();

                let grantor_outpoint = if let Some(outpoint) = grantor_token {
                    *outpoint
                } else {
                    let grantor_entries =
                        get_grantor_tokens_from_wallet(&wallet, OPTION_SOURCE, &user_script_pubkey).await?;
                    if grantor_entries.is_empty() {
                        return Err(Error::Config(
                            "No grantor tokens found in wallet. Create an option first or import grantor tokens."
                                .to_string(),
                        ));
                    }
                    let selected =
                        select_enriched_token_interactive(&grantor_entries, "Select grantor token for swap")?;
                    *selected.entry.outpoint()
                };

                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let swap_expiry: u32 = match expiry {
                    Some(exp_str) => parse_expiry(exp_str)? as u32,
                    None => (current_timestamp() + 86400 * 30) as u32,
                };

                let premium_asset_id = premium_asset.unwrap_or(*LIQUID_TESTNET_BITCOIN_ASSET);

                println!("  Grantor token: {grantor_outpoint}");
                println!("  Premium: {premium_amount} of {premium_asset_id}");
                println!("  Expiry: {}", format_relative_time(i64::from(swap_expiry)));

                let grantor_txout = cli_helper::explorer::fetch_utxo(grantor_outpoint).await?;

                let collateral_asset = grantor_txout
                    .asset
                    .explicit()
                    .ok_or_else(|| Error::Config("Grantor token has confidential asset".to_string()))?;

                let collateral_value = grantor_txout
                    .value
                    .explicit()
                    .ok_or_else(|| Error::Config("Grantor token has confidential value".to_string()))?;

                let swap_args = SwapWithChangeArguments::new(
                    collateral_asset,
                    premium_asset_id,
                    *premium_amount,
                    swap_expiry,
                    wallet.signer().public_key().serialize(),
                );

                let fee_filter = UtxoFilter::new()
                    .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                    .script_pubkey(user_script_pubkey.clone())
                    .required_value(fee.unwrap_or(PLACEHOLDER_FEE));

                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[fee_filter]).await?;
                let fee_entries = extract_entries_from_result(&results[0]);

                if fee_entries.is_empty() {
                    return Err(Error::Config("No LBTC UTXOs found for fee".to_string()));
                }

                let fee_utxo = &fee_entries[0];

                let collateral_input = (grantor_outpoint, grantor_txout.clone());
                let fee_input = (*fee_utxo.outpoint(), fee_utxo.txout().clone());

                let actual_fee = estimate_fee_signed(
                    fee.as_ref(),
                    config.get_fee_rate(),
                    |f| {
                        let (pst, _) = contracts::sdk::build_swap_deposit(
                            collateral_input.clone(),
                            fee_input.clone(),
                            collateral_value,
                            f,
                            &swap_args,
                            config.address_params(),
                        )?;
                        Ok((pst, vec![collateral_input.1.clone(), fee_input.1.clone()]))
                    },
                    |tx, utxos| sign_p2pk_inputs(tx, utxos, &wallet, config.address_params(), 0),
                )?;

                println!("  Fee: {actual_fee} sats");

                let (pst, taproot_pubkey_gen) = contracts::sdk::build_swap_deposit(
                    collateral_input.clone(),
                    fee_input.clone(),
                    collateral_value,
                    actual_fee,
                    &swap_args,
                    config.address_params(),
                )?;

                let tx = pst.extract_tx()?;
                let utxos = vec![collateral_input.1.clone(), fee_input.1];

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.address_params(), 0)?;

                if *broadcast {
                    cli_helper::explorer::broadcast_tx(&tx).await?;
                    println!("Broadcasted: {}", tx.txid());

                    let swap_outpoint = simplicityhl::elements::OutPoint::new(tx.txid(), 0);

                    let publishing_client = self.get_publishing_client(&config).await?;

                    let swap_event =
                        SwapCreatedEvent::new(swap_args.clone(), swap_outpoint, taproot_pubkey_gen.clone());

                    let event_id = publishing_client.publish_swap_created(&swap_event).await?;
                    println!("Published to NOSTR: {event_id}");

                    let now = current_timestamp();
                    let history = vec![HistoryEntry::with_txid_and_nostr(
                        ActionType::SwapCreated.as_str(),
                        &tx.txid().to_string(),
                        &event_id.to_hex(),
                        now,
                    )];

                    let metadata = ContractMetadata::from_nostr_with_history(
                        event_id.to_hex(),
                        publishing_client.public_key().await?.to_hex(),
                        now,
                        history,
                    );
                    let metadata_bytes = metadata.to_bytes()?;

                    wallet
                        .store()
                        .add_contract(
                            SWAP_WITH_CHANGE_SOURCE,
                            swap_args.build_arguments(),
                            taproot_pubkey_gen.clone(),
                            Some(&metadata_bytes),
                        )
                        .await?;

                    wallet
                        .store()
                        .insert_contract_token(&taproot_pubkey_gen, collateral_asset, SWAP_COLLATERAL_TAG)
                        .await?;

                    wallet.store().insert_transaction(&tx, HashMap::default()).await?;

                    publishing_client.disconnect().await;
                } else {
                    println!("{}", tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
            SwapCommand::Take {
                swap_event,
                fee,
                broadcast,
            } => {
                struct LocalSwapData {
                    swap_args: SwapWithChangeArguments,
                    taproot_pubkey_gen: contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen,
                    metadata: ContractMetadata,
                    current_outpoint: simplicityhl::elements::OutPoint,
                    current_value: u64,
                }

                println!("Taking swap offer...");

                let swap_contracts =
                    <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), SWAP_WITH_CHANGE_SOURCE)
                        .await?;

                let mut active_swaps: Vec<LocalSwapData> = Vec::new();
                for (args_bytes, tpg_str, metadata_bytes) in swap_contracts {
                    let Ok((arguments, _)): Result<(simplicityhl::Arguments, usize), _> =
                        bincode::serde::decode_from_slice(&args_bytes, bincode::config::standard())
                    else {
                        continue;
                    };
                    let Ok(swap_args) = SwapWithChangeArguments::from_arguments(&arguments) else {
                        continue;
                    };

                    let Ok(taproot_pubkey_gen) = contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen::build_from_str(
                        &tpg_str,
                        &swap_args,
                        config.address_params(),
                        &contracts::swap_with_change::get_swap_with_change_address,
                    ) else {
                        continue;
                    };

                    let metadata = metadata_bytes
                        .as_ref()
                        .and_then(|b| ContractMetadata::from_bytes(b).ok())
                        .unwrap_or_default();

                    let collateral_asset = swap_args.get_collateral_asset_id();
                    let filter = UtxoFilter::new()
                        .taproot_pubkey_gen(taproot_pubkey_gen.clone())
                        .asset_id(collateral_asset);

                    if let Ok(results) = <_ as UtxoStore>::query_utxos(wallet.store(), &[filter]).await
                        && let Some((outpoint, value)) = match &results[0] {
                            UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _) => {
                                entries
                                    .first()
                                    .and_then(|entry| entry.value().map(|value| (*entry.outpoint(), value)))
                            }
                            UtxoQueryResult::Empty => None,
                        }
                    {
                        active_swaps.push(LocalSwapData {
                            swap_args,
                            taproot_pubkey_gen,
                            metadata,
                            current_outpoint: outpoint,
                            current_value: value,
                        });
                    }
                }

                let selected_swap = if let Some(event_id_str) = swap_event {
                    active_swaps
                        .into_iter()
                        .find(|s| {
                            s.metadata
                                .nostr_event_id
                                .as_ref()
                                .is_some_and(|id| id.starts_with(event_id_str))
                        })
                        .ok_or_else(|| Error::Config(format!("Swap event not found or fully taken: {event_id_str}")))?
                } else {
                    if active_swaps.is_empty() {
                        return Err(Error::Config(
                            "No active swap offers found. Run `sync nostr` first to sync swap events from relays, \
                             then `sync spent` to update UTXO status."
                                .to_string(),
                        ));
                    }

                    println!(
                        "  {:<3} | {:<12} | {:<10} | {:<14} | {:<15} | Seller",
                        "#", "Offering", "Price", "Wants", "Expires"
                    );
                    println!("{}", "-".repeat(90));
                    for (idx, swap) in active_swaps.iter().enumerate() {
                        let seller = swap.metadata.nostr_author.as_deref().unwrap_or("unknown");
                        let price = swap.swap_args.collateral_per_contract();
                        println!(
                            "  {:<3} | {:<12} | {:<10} | {:<14} | {:<15} | {}",
                            idx + 1,
                            swap.current_value,
                            price,
                            crate::cli::interactive::format_settlement_asset(&swap.swap_args.get_settlement_asset_id()),
                            format_relative_time(i64::from(swap.swap_args.expiry_time())),
                            crate::cli::interactive::truncate_with_ellipsis(seller, 12)
                        );
                    }
                    println!();

                    let selection =
                        crate::cli::interactive::prompt_selection("Select swap offer to take", active_swaps.len())
                            .map_err(Error::Io)?
                            .ok_or_else(|| Error::Config("Selection cancelled".to_string()))?;

                    active_swaps
                        .into_iter()
                        .nth(selection)
                        .ok_or_else(|| Error::Config("Invalid selection".to_string()))?
                };

                let args = &selected_swap.swap_args;
                let current_swap_outpoint = selected_swap.current_outpoint;
                let actual_collateral = selected_swap.current_value;

                let event_id_display = selected_swap.metadata.nostr_event_id.as_deref().unwrap_or("local");
                println!("  Swap event: {event_id_display}");
                println!("  Collateral available: {actual_collateral}");
                println!(
                    "  Price: {} (settlement per collateral)",
                    args.collateral_per_contract()
                );
                println!("  Expiry: {}", format_relative_time(i64::from(args.expiry_time())));

                let collateral_amount_to_receive =
                    prompt_amount("Amount of collateral to receive").map_err(Error::Io)?;

                if collateral_amount_to_receive > actual_collateral {
                    return Err(Error::Config(format!(
                        "Cannot receive {collateral_amount_to_receive} collateral, only {actual_collateral} available"
                    )));
                }

                let settlement_required = collateral_amount_to_receive
                    .checked_mul(args.collateral_per_contract())
                    .ok_or_else(|| Error::Config("Overflow calculating settlement amount".to_string()))?;

                println!("  Settlement required: {settlement_required}");

                let script_pubkey = wallet.signer().p2pk_address(config.address_params())?.script_pubkey();
                let settlement_asset = args.get_settlement_asset_id();

                let settlement_filter = UtxoFilter::new()
                    .asset_id(settlement_asset)
                    .script_pubkey(script_pubkey.clone())
                    .required_value(settlement_required);

                let fee_filter = UtxoFilter::new()
                    .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                    .script_pubkey(script_pubkey.clone())
                    .required_value(fee.unwrap_or(PLACEHOLDER_FEE));

                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[settlement_filter, fee_filter]).await?;

                let settlement_entries = match &results[0] {
                    UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _) => entries,
                    UtxoQueryResult::Empty => {
                        return Err(Error::Config(format!(
                            "No settlement UTXOs found for asset {settlement_asset}"
                        )));
                    }
                };

                let fee_entries = extract_entries_from_result(&results[1]);

                if settlement_entries.is_empty() {
                    return Err(Error::Config(format!(
                        "No settlement UTXOs found for asset {settlement_asset}. Need {settlement_required} sats."
                    )));
                }
                if fee_entries.is_empty() {
                    return Err(Error::Config("No LBTC UTXOs found for fee".to_string()));
                }

                let settlement_utxo = &settlement_entries[0];
                let fee_utxo = if settlement_asset == *LIQUID_TESTNET_BITCOIN_ASSET {
                    fee_entries
                        .iter()
                        .find(|entry| entry.outpoint() != settlement_utxo.outpoint())
                        .ok_or_else(|| {
                            Error::Config(
                                "Need two separate LBTC UTXOs: one for settlement and one for fee. \
                                 Please split your LBTC UTXO or fund with additional LBTC."
                                    .to_string(),
                            )
                        })?
                } else {
                    &fee_entries[0]
                };

                let swap_txout = cli_helper::explorer::fetch_utxo(current_swap_outpoint).await?;

                let swap_input = (current_swap_outpoint, swap_txout.clone());
                let settlement_input = (*settlement_utxo.outpoint(), settlement_utxo.txout().clone());
                let fee_input = (*fee_utxo.outpoint(), fee_utxo.txout().clone());

                let actual_fee = if let Some(f) = fee {
                    *f
                } else {
                    let (pst, branch) = contracts::sdk::build_swap_exercise(
                        swap_input.clone(),
                        settlement_input.clone(),
                        fee_input.clone(),
                        collateral_amount_to_receive,
                        PLACEHOLDER_FEE,
                        args,
                        script_pubkey.clone(),
                    )?;
                    let mut tx = pst.extract_tx()?;
                    let utxos = vec![swap_txout.clone(), settlement_input.1.clone(), fee_input.1.clone()];
                    let swap_program = get_swap_with_change_program(args)?;
                    tx = finalize_swap_with_change_transaction(
                        tx,
                        &selected_swap.taproot_pubkey_gen.get_x_only_pubkey(),
                        &swap_program,
                        &utxos,
                        0,
                        &branch,
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

                let (pst, branch) = contracts::sdk::build_swap_exercise(
                    swap_input.clone(),
                    settlement_input.clone(),
                    fee_input.clone(),
                    collateral_amount_to_receive,
                    actual_fee,
                    args,
                    script_pubkey.clone(),
                )?;

                let mut tx = pst.extract_tx()?;
                let utxos = vec![swap_txout.clone(), settlement_input.1.clone(), fee_input.1.clone()];

                let swap_program = get_swap_with_change_program(args)?;
                tx = finalize_swap_with_change_transaction(
                    tx,
                    &selected_swap.taproot_pubkey_gen.get_x_only_pubkey(),
                    &swap_program,
                    &utxos,
                    0,
                    &branch,
                    config.address_params(),
                    *LIQUID_TESTNET_GENESIS,
                )?;

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.address_params(), 1)?;

                if *broadcast {
                    cli_helper::explorer::broadcast_tx(&tx).await?;
                    println!("Broadcasted: {}", tx.txid());

                    if let Some(ref nostr_event_id) = selected_swap.metadata.nostr_event_id
                        && let Ok(event_id) = nostr::EventId::from_hex(nostr_event_id)
                    {
                        let publishing_client = self.get_publishing_client(&config).await?;

                        let action_event = ActionCompletedEvent::new(
                            event_id,
                            ActionType::SwapExercised,
                            simplicityhl::elements::OutPoint::new(tx.txid(), 0),
                        );

                        let published_id = publishing_client.publish_action_completed(&action_event).await?;
                        println!("Published action to NOSTR: {published_id}");

                        publishing_client.disconnect().await;
                    }

                    wallet.store().insert_transaction(&tx, HashMap::default()).await?;

                    let entry = HistoryEntry::with_txid(
                        ActionType::SwapExercised.as_str(),
                        &tx.txid().to_string(),
                        current_timestamp(),
                    );
                    crate::sync::add_history_entry(wallet.store(), &selected_swap.taproot_pubkey_gen, entry).await?;
                } else {
                    println!("{}", tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
            SwapCommand::Cancel {
                swap_event,
                fee,
                broadcast,
            } => {
                struct LocalCancellableSwap {
                    swap_args: SwapWithChangeArguments,
                    taproot_pubkey_gen: contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen,
                    metadata: ContractMetadata,
                }

                println!("Cancelling swap offer (reclaiming collateral after expiry)...");

                let swap_contracts =
                    <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), SWAP_WITH_CHANGE_SOURCE)
                        .await?;

                if swap_contracts.is_empty() {
                    return Err(Error::Config(
                        "No swap contracts found in local database. Create a swap first or run `sync nostr` to import."
                            .to_string(),
                    ));
                }

                println!("Checking swap status...");

                let mut cancellable_swaps: Vec<LocalCancellableSwap> = Vec::new();

                for (args_bytes, tpg_str, metadata_bytes) in swap_contracts {
                    let Ok((arguments, _)): Result<(simplicityhl::Arguments, usize), _> =
                        bincode::serde::decode_from_slice(&args_bytes, bincode::config::standard())
                    else {
                        continue;
                    };
                    let Ok(swap_args) = SwapWithChangeArguments::from_arguments(&arguments) else {
                        continue;
                    };

                    let is_expired = current_timestamp() > i64::from(swap_args.expiry_time());
                    if !is_expired {
                        continue; // Skip non-expired swaps
                    }

                    let Ok(taproot_pubkey_gen) = contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen::build_from_str(
                        &tpg_str,
                        &swap_args,
                        config.address_params(),
                        &contracts::swap_with_change::get_swap_with_change_address,
                    ) else {
                        continue;
                    };

                    let metadata = metadata_bytes
                        .as_ref()
                        .and_then(|b| ContractMetadata::from_bytes(b).ok())
                        .unwrap_or_default();

                    let collateral_asset = swap_args.get_collateral_asset_id();
                    let filter = UtxoFilter::new()
                        .taproot_pubkey_gen(taproot_pubkey_gen.clone())
                        .asset_id(collateral_asset);

                    if let Ok(results) = <_ as UtxoStore>::query_utxos(wallet.store(), &[filter]).await {
                        let has_collateral = matches!(&results[0],
                            UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _)
                            if !entries.is_empty()
                        );
                        if has_collateral {
                            cancellable_swaps.push(LocalCancellableSwap {
                                swap_args,
                                taproot_pubkey_gen,
                                metadata,
                            });
                        }
                    }
                }

                if cancellable_swaps.is_empty() {
                    return Err(Error::Config(
                        "No cancellable swaps found. Swaps must be expired and still have collateral. Run `sync utxos` first.".to_string(),
                    ));
                }

                println!(
                    "  {:<3} | {:<12} | {:<14} | {:<20} | Contract",
                    "#", "Collateral", "Asset", "Expired"
                );
                println!("{}", "-".repeat(80));
                for (idx, cs) in cancellable_swaps.iter().enumerate() {
                    let asset_display =
                        crate::cli::interactive::format_settlement_asset(&cs.swap_args.get_collateral_asset_id());
                    let expiry_time = cs.swap_args.expiry_time();
                    let contract_short = cs.metadata.nostr_event_id.as_ref().map_or_else(
                        || crate::cli::interactive::truncate_with_ellipsis(&cs.taproot_pubkey_gen.to_string(), 16),
                        |id| crate::cli::interactive::truncate_with_ellipsis(id, 16),
                    );
                    println!(
                        "  {:<3} | {:<12} | {:<14} | {:<20} | {}",
                        idx + 1,
                        "available",
                        asset_display,
                        format!("expired ({})", expiry_time),
                        contract_short
                    );
                }
                println!();

                let selected = if let Some(event_id_str) = swap_event {
                    cancellable_swaps
                        .into_iter()
                        .find(|cs| {
                            cs.metadata
                                .nostr_event_id
                                .as_ref()
                                .is_some_and(|id| id.starts_with(event_id_str))
                        })
                        .ok_or_else(|| Error::Config(format!("Swap event not found: {event_id_str}")))?
                } else {
                    let selection = crate::cli::interactive::prompt_selection(
                        "Select swap offer to cancel",
                        cancellable_swaps.len(),
                    )
                    .map_err(Error::Io)?
                    .ok_or_else(|| Error::Config("Selection cancelled".to_string()))?;

                    cancellable_swaps
                        .into_iter()
                        .nth(selection)
                        .ok_or_else(|| Error::Config("Invalid selection".to_string()))?
                };

                let args = &selected.swap_args;
                let taproot_pubkey_gen = &selected.taproot_pubkey_gen;

                if let Some(ref event_id) = selected.metadata.nostr_event_id {
                    println!("  Swap event: {event_id}");
                }

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
                let fee_input = (*fee_utxo.outpoint(), fee_utxo.txout().clone());

                let collateral_asset = args.get_collateral_asset_id();
                let filter = UtxoFilter::new()
                    .taproot_pubkey_gen(taproot_pubkey_gen.clone())
                    .asset_id(collateral_asset);

                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[filter]).await?;
                let swap_entry = match &results[0] {
                    UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _) => {
                        entries.first().ok_or_else(|| Error::Config(
                            "No collateral UTXO found at contract address. Swap may have been taken. Run `sync utxos` to update.".to_string()
                        ))?
                    }
                    UtxoQueryResult::Empty => {
                        return Err(Error::Config(
                            "No collateral UTXO found at contract address. Swap may have been taken. Run `sync utxos` to update.".to_string()
                        ));
                    }
                };

                let current_outpoint = *swap_entry.outpoint();
                let swap_txout = swap_entry.txout().clone();
                let swap_input = (current_outpoint, swap_txout.clone());

                let actual_fee = if let Some(f) = fee {
                    *f
                } else {
                    let pst = contracts::sdk::build_swap_expiry(
                        swap_input.clone(),
                        fee_input.clone(),
                        PLACEHOLDER_FEE,
                        args,
                        script_pubkey.clone(),
                    )?;
                    let mut tx = pst.extract_tx()?;
                    let utxos = vec![swap_txout.clone(), fee_input.1.clone()];
                    let swap_program = get_swap_with_change_program(args)?;
                    let signature = wallet.signer().sign_contract(
                        &tx,
                        &swap_program,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &utxos,
                        0,
                        config.address_params(),
                        *LIQUID_TESTNET_GENESIS,
                    )?;
                    let branch = contracts::swap_with_change::build_witness::SwapWithChangeBranch::Expiry {
                        schnorr_signature: signature,
                    };
                    tx = finalize_swap_with_change_transaction(
                        tx,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &swap_program,
                        &utxos,
                        0,
                        &branch,
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

                let pst = contracts::sdk::build_swap_expiry(
                    swap_input.clone(),
                    fee_input.clone(),
                    actual_fee,
                    args,
                    script_pubkey.clone(),
                )?;

                let mut tx = pst.extract_tx()?;
                let utxos = vec![swap_txout.clone(), fee_input.1.clone()];
                let swap_program = get_swap_with_change_program(args)?;

                let signature = wallet.signer().sign_contract(
                    &tx,
                    &swap_program,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &utxos,
                    0,
                    config.address_params(),
                    *LIQUID_TESTNET_GENESIS,
                )?;

                let branch = contracts::swap_with_change::build_witness::SwapWithChangeBranch::Expiry {
                    schnorr_signature: signature,
                };

                tx = finalize_swap_with_change_transaction(
                    tx,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &swap_program,
                    &utxos,
                    0,
                    &branch,
                    config.address_params(),
                    *LIQUID_TESTNET_GENESIS,
                )?;

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.address_params(), 1)?;

                if *broadcast {
                    cli_helper::explorer::broadcast_tx(&tx).await?;
                    println!("Broadcasted: {}", tx.txid());

                    if let Some(ref nostr_event_id) = selected.metadata.nostr_event_id
                        && let Ok(event_id) = nostr::EventId::from_hex(nostr_event_id)
                    {
                        let publishing_client = self.get_publishing_client(&config).await?;

                        let action_event = ActionCompletedEvent::new(
                            event_id,
                            ActionType::SwapCancelled,
                            simplicityhl::elements::OutPoint::new(tx.txid(), 0),
                        );

                        let published_id = publishing_client.publish_action_completed(&action_event).await?;
                        println!("Published cancellation to NOSTR: {published_id}");

                        publishing_client.disconnect().await;
                    }

                    wallet.store().insert_transaction(&tx, HashMap::default()).await?;

                    let entry = HistoryEntry::with_txid(
                        ActionType::SwapCancelled.as_str(),
                        &tx.txid().to_string(),
                        current_timestamp(),
                    );
                    crate::sync::add_history_entry(wallet.store(), taproot_pubkey_gen, entry).await?;
                } else {
                    println!("{}", tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
            SwapCommand::Withdraw {
                swap_event,
                fee,
                broadcast,
            } => {
                struct LocalWithdrawableSwap {
                    swap_args: SwapWithChangeArguments,
                    taproot_pubkey_gen: contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen,
                    metadata: ContractMetadata,
                    settlement_amount: u64,
                }

                println!("Withdrawing settlement from swap (claiming payment after swap was taken)...");

                let swap_contracts =
                    <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), SWAP_WITH_CHANGE_SOURCE)
                        .await?;

                if swap_contracts.is_empty() {
                    return Err(Error::Config(
                        "No swap contracts found in local database. Create a swap first or run `sync nostr` to import."
                            .to_string(),
                    ));
                }

                println!("Checking swap status...");

                let mut withdrawable_swaps: Vec<LocalWithdrawableSwap> = Vec::new();

                for (args_bytes, tpg_str, metadata_bytes) in swap_contracts {
                    let Ok((arguments, _)): Result<(simplicityhl::Arguments, usize), _> =
                        bincode::serde::decode_from_slice(&args_bytes, bincode::config::standard())
                    else {
                        continue;
                    };
                    let Ok(swap_args) = SwapWithChangeArguments::from_arguments(&arguments) else {
                        continue;
                    };

                    let Ok(taproot_pubkey_gen) = contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen::build_from_str(
                        &tpg_str,
                        &swap_args,
                        config.address_params(),
                        &contracts::swap_with_change::get_swap_with_change_address,
                    ) else {
                        continue;
                    };

                    let metadata = metadata_bytes
                        .as_ref()
                        .and_then(|b| ContractMetadata::from_bytes(b).ok())
                        .unwrap_or_default();

                    let settlement_asset = swap_args.get_settlement_asset_id();
                    let filter = UtxoFilter::new()
                        .taproot_pubkey_gen(taproot_pubkey_gen.clone())
                        .asset_id(settlement_asset);

                    if let Ok(results) = <_ as UtxoStore>::query_utxos(wallet.store(), &[filter]).await
                        && let UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _) =
                            &results[0]
                        && let Some(entry) = entries.first()
                        && let Some(value) = entry.value()
                    {
                        let wallet_pubkey = wallet.signer().public_key();
                        let contract_user_pubkey = swap_args.user_pubkey();
                        if wallet_pubkey.serialize() == contract_user_pubkey {
                            withdrawable_swaps.push(LocalWithdrawableSwap {
                                swap_args,
                                taproot_pubkey_gen,
                                metadata,
                                settlement_amount: value,
                            });
                        }
                    }
                }

                if withdrawable_swaps.is_empty() {
                    return Err(Error::Config(
                        "No withdrawable swaps found. Either:\n  - No swaps have been taken yet\n  - The taken swaps belong to a different wallet (pubkey mismatch)\n  - Run `sync utxos` to update"
                            .to_string(),
                    ));
                }

                println!(
                    "  {:<3} | {:<20} | {:<14} | Contract",
                    "#", "Settlement Available", "Asset"
                );
                println!("{}", "-".repeat(70));
                for (idx, ws) in withdrawable_swaps.iter().enumerate() {
                    let asset_display =
                        crate::cli::interactive::format_settlement_asset(&ws.swap_args.get_settlement_asset_id());
                    let contract_short = ws.metadata.nostr_event_id.as_ref().map_or_else(
                        || crate::cli::interactive::truncate_with_ellipsis(&ws.taproot_pubkey_gen.to_string(), 16),
                        |id| crate::cli::interactive::truncate_with_ellipsis(id, 16),
                    );
                    println!(
                        "  {:<3} | {:<20} | {:<14} | {}",
                        idx + 1,
                        ws.settlement_amount,
                        asset_display,
                        contract_short
                    );
                }
                println!();

                let selected = if let Some(event_id_str) = swap_event {
                    withdrawable_swaps
                        .into_iter()
                        .find(|ws| {
                            ws.metadata
                                .nostr_event_id
                                .as_ref()
                                .is_some_and(|id| id.starts_with(event_id_str))
                        })
                        .ok_or_else(|| Error::Config(format!("Swap event not found: {event_id_str}")))?
                } else {
                    let selection = crate::cli::interactive::prompt_selection(
                        "Select swap to withdraw from",
                        withdrawable_swaps.len(),
                    )
                    .map_err(Error::Io)?
                    .ok_or_else(|| Error::Config("Selection cancelled".to_string()))?;

                    withdrawable_swaps
                        .into_iter()
                        .nth(selection)
                        .ok_or_else(|| Error::Config("Invalid selection".to_string()))?
                };

                let args = &selected.swap_args;
                let taproot_pubkey_gen = &selected.taproot_pubkey_gen;

                if let Some(ref event_id) = selected.metadata.nostr_event_id {
                    println!("  Swap event: {event_id}");
                }

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
                let fee_input = (*fee_utxo.outpoint(), fee_utxo.txout().clone());

                let settlement_asset = args.get_settlement_asset_id();
                let filter = UtxoFilter::new()
                    .taproot_pubkey_gen(taproot_pubkey_gen.clone())
                    .asset_id(settlement_asset);

                let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[filter]).await?;
                let swap_entry = match &results[0] {
                    UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _) => {
                        entries.first().ok_or_else(|| {
                            Error::Config(
                                "No settlement UTXO found at contract address. Run `sync utxos` to update.".to_string(),
                            )
                        })?
                    }
                    UtxoQueryResult::Empty => {
                        return Err(Error::Config(
                            "No settlement UTXO found at contract address. Run `sync utxos` to update.".to_string(),
                        ));
                    }
                };

                let current_outpoint = *swap_entry.outpoint();
                let swap_txout = swap_entry.txout().clone();
                let swap_input = (current_outpoint, swap_txout.clone());

                let actual_fee = if let Some(f) = fee {
                    *f
                } else {
                    let pst = contracts::sdk::build_swap_withdraw(
                        swap_input.clone(),
                        fee_input.clone(),
                        PLACEHOLDER_FEE,
                        args,
                        script_pubkey.clone(),
                    )?;
                    let mut tx = pst.extract_tx()?;
                    let utxos = vec![swap_txout.clone(), fee_input.1.clone()];
                    let swap_program = get_swap_with_change_program(args)?;
                    let signature = wallet.signer().sign_contract(
                        &tx,
                        &swap_program,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &utxos,
                        0,
                        config.address_params(),
                        *LIQUID_TESTNET_GENESIS,
                    )?;
                    let branch = contracts::swap_with_change::build_witness::SwapWithChangeBranch::Withdraw {
                        schnorr_signature: signature,
                    };
                    tx = finalize_swap_with_change_transaction(
                        tx,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &swap_program,
                        &utxos,
                        0,
                        &branch,
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

                let pst = contracts::sdk::build_swap_withdraw(
                    swap_input.clone(),
                    fee_input.clone(),
                    actual_fee,
                    args,
                    script_pubkey.clone(),
                )?;

                let mut tx = pst.extract_tx()?;
                let utxos = vec![swap_txout.clone(), fee_input.1.clone()];
                let swap_program = get_swap_with_change_program(args)?;

                let signature = wallet.signer().sign_contract(
                    &tx,
                    &swap_program,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &utxos,
                    0,
                    config.address_params(),
                    *LIQUID_TESTNET_GENESIS,
                )?;

                let branch = contracts::swap_with_change::build_witness::SwapWithChangeBranch::Withdraw {
                    schnorr_signature: signature,
                };

                tx = finalize_swap_with_change_transaction(
                    tx,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &swap_program,
                    &utxos,
                    0,
                    &branch,
                    config.address_params(),
                    *LIQUID_TESTNET_GENESIS,
                )?;

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.address_params(), 1)?;

                if *broadcast {
                    cli_helper::explorer::broadcast_tx(&tx).await?;
                    println!("Broadcasted: {}", tx.txid());

                    if let Some(ref nostr_event_id) = selected.metadata.nostr_event_id
                        && let Ok(event_id) = nostr::EventId::from_hex(nostr_event_id)
                    {
                        let publishing_client = self.get_publishing_client(&config).await?;

                        let action_event = ActionCompletedEvent::new(
                            event_id,
                            ActionType::SettlementClaimed,
                            simplicityhl::elements::OutPoint::new(tx.txid(), 0),
                        );

                        let published_id = publishing_client.publish_action_completed(&action_event).await?;
                        println!("Published withdrawal to NOSTR: {published_id}");

                        publishing_client.disconnect().await;
                    }

                    wallet.store().insert_transaction(&tx, HashMap::default()).await?;

                    let entry = HistoryEntry::with_txid("swap_withdrawn", &tx.txid().to_string(), current_timestamp());
                    crate::sync::add_history_entry(wallet.store(), taproot_pubkey_gen, entry).await?;
                } else {
                    println!("{}", tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
        }
    }
}
