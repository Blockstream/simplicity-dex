use crate::cli::interactive::{
    current_timestamp, extract_entries_from_result, format_relative_time, format_settlement_asset, get_wallet_assets,
    parse_expiry, prompt_amount, select_asset_interactive, truncate_with_ellipsis,
};
use crate::cli::tables::{
    display_active_option_offers_table, display_cancellable_option_offers_table,
    display_withdrawable_option_offers_table,
};
use crate::cli::{Cli, OptionOfferCommand};
use crate::config::Config;
use crate::error::Error;
use crate::fee::{PLACEHOLDER_FEE, estimate_fee_signed};
use crate::metadata::{ContractMetadata, HistoryEntry};
use crate::signing::sign_p2pk_inputs;

use std::collections::HashMap;

use coin_store::{UtxoFilter, UtxoQueryResult, UtxoStore};
use contracts::option_offer::{
    OPTION_OFFER_SOURCE, OptionOfferArguments, finalize_option_offer_transaction, get_option_offer_program,
};
use options_relay::{ActionCompletedEvent, ActionType, OptionOfferCreatedEvent};
use simplicityhl::elements::pset::serialize::Serialize;
use simplicityhl::simplicity::hex::DisplayHex;
use simplicityhl::tracker::TrackerLogLevel;
use simplicityhl_core::LIQUID_TESTNET_BITCOIN_ASSET;

pub const OPTION_OFFER_COLLATERAL_TAG: &str = "option_offer_collateral";

pub struct LocalOptionOfferData {
    pub(crate) option_offer_args: OptionOfferArguments,
    pub(crate) taproot_pubkey_gen: contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen,
    pub(crate) metadata: ContractMetadata,
    pub(crate) current_outpoint: simplicityhl::elements::OutPoint,
    pub(crate) current_value: u64,
}

pub struct LocalCancellableOptionOffer {
    pub(crate) option_offer_args: OptionOfferArguments,
    pub(crate) taproot_pubkey_gen: contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen,
    pub(crate) metadata: ContractMetadata,
    pub(crate) collateral_amount: u64,
    pub(crate) premium_amount: u64,
}

pub struct LocalWithdrawableOptionOffer {
    pub(crate) option_offer_args: OptionOfferArguments,
    pub(crate) taproot_pubkey_gen: contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen,
    pub(crate) metadata: ContractMetadata,
    pub(crate) settlement_amount: u64,
}

pub struct ActiveOptionOfferDisplay {
    pub(crate) index: usize,
    pub(crate) offering: String,
    pub(crate) price: String,
    pub(crate) wants: String,
    pub(crate) expires: String,
    pub(crate) seller: String,
}

pub struct CancellableOptionOfferDisplay {
    pub(crate) index: usize,
    pub(crate) collateral: String,
    pub(crate) premium: String,
    pub(crate) asset: String,
    pub(crate) expired: String,
    pub(crate) contract: String,
}

pub struct WithdrawableOptionOfferDisplay {
    pub(crate) index: usize,
    pub(crate) settlement: String,
    pub(crate) asset: String,
    pub(crate) contract: String,
}

impl Cli {
    #[allow(clippy::too_many_lines)]
    pub(crate) async fn run_option_offer(&self, config: Config, command: &OptionOfferCommand) -> Result<(), Error> {
        let wallet = self.get_wallet(&config).await?;

        match command {
            OptionOfferCommand::Create {
                collateral_asset,
                collateral_amount,
                premium_asset,
                premium_amount,
                settlement_asset,
                settlement_amount,
                expiry,
                fee,
                broadcast,
            } => {
                println!("Creating option offer...");

                let user_script_pubkey = wallet.signer().p2pk_address(config.network())?.script_pubkey();

                let wallet_assets = get_wallet_assets(&wallet, &user_script_pubkey).await?;

                let collateral_asset_id = if let Some(asset) = collateral_asset {
                    *asset
                } else {
                    let selected = select_asset_interactive(&wallet_assets, "Select collateral asset", false)?;
                    selected.asset_id
                };

                let collateral_amt = if let Some(amt) = collateral_amount {
                    *amt
                } else {
                    prompt_amount("Enter collateral amount").map_err(Error::Io)?
                };

                if collateral_amt == 0 {
                    return Err(Error::Config("Collateral amount must be greater than 0".to_string()));
                }

                let premium_asset_id = if let Some(asset) = premium_asset {
                    *asset
                } else {
                    let selected = select_asset_interactive(&wallet_assets, "Select premium asset", true)?;
                    selected.asset_id
                };

                let total_premium = if let Some(amt) = premium_amount {
                    *amt
                } else {
                    prompt_amount("Enter total premium amount").map_err(Error::Io)?
                };

                let premium_per_collateral = if total_premium == 0 {
                    0
                } else {
                    if total_premium % collateral_amt != 0 {
                        return Err(Error::Config(format!(
                            "Premium amount ({total_premium}) must be evenly divisible by collateral amount ({collateral_amt}). \
                             Remainder: {}",
                            total_premium % collateral_amt
                        )));
                    }
                    total_premium / collateral_amt
                };

                let settlement_asset_id = if let Some(asset) = settlement_asset {
                    *asset
                } else {
                    let selected = select_asset_interactive(&wallet_assets, "Select settlement asset", true)?;
                    selected.asset_id
                };

                let settlement_amt = if let Some(amt) = settlement_amount {
                    *amt
                } else {
                    prompt_amount("Enter total settlement amount expected").map_err(Error::Io)?
                };

                let collateral_per_contract = if settlement_amt == 0 {
                    return Err(Error::Config("Settlement amount must be greater than 0".to_string()));
                } else {
                    if settlement_amt % collateral_amt != 0 {
                        return Err(Error::Config(format!(
                            "Settlement amount ({settlement_amt}) must be evenly divisible by collateral amount ({collateral_amt}). \
                             Remainder: {}",
                            settlement_amt % collateral_amt
                        )));
                    }
                    settlement_amt / collateral_amt
                };

                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let offer_expiry: u32 = parse_expiry(expiry)? as u32;

                println!();
                println!(
                    "  Collateral: {collateral_amt} of {}",
                    format_settlement_asset(&collateral_asset_id)
                );
                println!(
                    "  Premium: {total_premium} of {} (rate: {premium_per_collateral} per collateral)",
                    format_settlement_asset(&premium_asset_id)
                );
                println!(
                    "  Settlement: {} of {} (rate: {collateral_per_contract} per collateral)",
                    settlement_amt,
                    format_settlement_asset(&settlement_asset_id)
                );
                println!("  Expiry: {}", format_relative_time(i64::from(offer_expiry)));

                let option_offer_args = OptionOfferArguments::new(
                    collateral_asset_id,
                    premium_asset_id,
                    settlement_asset_id,
                    collateral_per_contract,
                    premium_per_collateral,
                    offer_expiry,
                    wallet.signer().public_key().serialize(),
                );

                let collateral_filter = UtxoFilter::new()
                    .asset_id(collateral_asset_id)
                    .script_pubkey(user_script_pubkey.clone())
                    .required_value(collateral_amt);

                let premium_filter = UtxoFilter::new()
                    .asset_id(premium_asset_id)
                    .script_pubkey(user_script_pubkey.clone())
                    .required_value(total_premium);

                let fee_filter = UtxoFilter::new()
                    .asset_id(*LIQUID_TESTNET_BITCOIN_ASSET)
                    .script_pubkey(user_script_pubkey.clone())
                    .required_value(fee.unwrap_or(PLACEHOLDER_FEE));

                let results =
                    <_ as UtxoStore>::query_utxos(wallet.store(), &[collateral_filter, premium_filter, fee_filter])
                        .await?;

                let collateral_entries = extract_entries_from_result(&results[0]);
                let premium_entries = extract_entries_from_result(&results[1]);
                let fee_entries = extract_entries_from_result(&results[2]);

                if collateral_entries.is_empty() {
                    return Err(Error::Config(format!(
                        "No collateral UTXOs found for asset {}",
                        format_settlement_asset(&collateral_asset_id)
                    )));
                }
                if premium_entries.is_empty() {
                    return Err(Error::Config(format!(
                        "No premium UTXOs found for asset {}. Need {total_premium}",
                        format_settlement_asset(&premium_asset_id)
                    )));
                }
                if fee_entries.is_empty() {
                    return Err(Error::Config("No LBTC UTXOs found for fee".to_string()));
                }

                let collateral_utxo = &collateral_entries[0];
                let premium_utxo = &premium_entries[0];
                let fee_utxo = &fee_entries[0];

                let collateral_input = (*collateral_utxo.outpoint(), collateral_utxo.txout().clone());
                let premium_input = (*premium_utxo.outpoint(), premium_utxo.txout().clone());
                let fee_input = (*fee_utxo.outpoint(), fee_utxo.txout().clone());

                let actual_fee = estimate_fee_signed(
                    fee.as_ref(),
                    config.get_fee_rate(),
                    |f| {
                        let (pst, _) = contracts::sdk::build_option_offer_deposit(
                            collateral_input.clone(),
                            premium_input.clone(),
                            fee_input.clone(),
                            collateral_amt,
                            f,
                            &option_offer_args,
                            config.network(),
                        )?;
                        Ok((
                            pst,
                            vec![collateral_input.1.clone(), premium_input.1.clone(), fee_input.1.clone()],
                        ))
                    },
                    |tx, utxos| sign_p2pk_inputs(tx, utxos, &wallet, config.network(), 0),
                )?;

                println!("  Fee: {actual_fee} sats");

                let (pst, taproot_pubkey_gen) = contracts::sdk::build_option_offer_deposit(
                    collateral_input.clone(),
                    premium_input.clone(),
                    fee_input.clone(),
                    collateral_amt,
                    actual_fee,
                    &option_offer_args,
                    config.network(),
                )?;

                let tx = pst.extract_tx()?;
                let utxos = vec![collateral_input.1.clone(), premium_input.1, fee_input.1];

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.network(), 0)?;

                if *broadcast {
                    cli_helper::explorer::broadcast_tx(&tx).await?;
                    println!("Broadcasted: {}", tx.txid());

                    let offer_outpoint = simplicityhl::elements::OutPoint::new(tx.txid(), 0);

                    let publishing_client = self.get_publishing_client(&config).await?;

                    let offer_event = OptionOfferCreatedEvent::new(
                        option_offer_args.clone(),
                        offer_outpoint,
                        taproot_pubkey_gen.clone(),
                    );

                    let event_id = publishing_client.publish_option_offer_created(&offer_event).await?;
                    println!("Published to NOSTR: {event_id}");

                    let now = current_timestamp();
                    let history = vec![HistoryEntry::with_txid_and_nostr(
                        ActionType::OptionOfferCreated.as_str(),
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
                            OPTION_OFFER_SOURCE,
                            option_offer_args.build_arguments(),
                            taproot_pubkey_gen.clone(),
                            Some(&metadata_bytes),
                        )
                        .await?;

                    wallet
                        .store()
                        .insert_contract_token(&taproot_pubkey_gen, collateral_asset_id, OPTION_OFFER_COLLATERAL_TAG)
                        .await?;

                    wallet.store().insert_transaction(&tx, HashMap::default()).await?;

                    publishing_client.disconnect().await;
                } else {
                    println!("{}", tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
            OptionOfferCommand::Take {
                offer_event,
                fee,
                broadcast,
            } => {
                println!("Taking option offer...");

                let offer_contracts =
                    <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), OPTION_OFFER_SOURCE)
                        .await?;

                let mut active_offers: Vec<LocalOptionOfferData> = Vec::new();
                for (args_bytes, tpg_str, metadata_bytes) in offer_contracts {
                    let Ok((arguments, _)): Result<(simplicityhl::Arguments, usize), _> =
                        bincode::serde::decode_from_slice(&args_bytes, bincode::config::standard())
                    else {
                        continue;
                    };
                    let Ok(option_offer_args) = OptionOfferArguments::from_arguments(&arguments) else {
                        continue;
                    };

                    let Ok(taproot_pubkey_gen) = contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen::build_from_str(
                        &tpg_str,
                        &option_offer_args,
                        config.network(),
                        &contracts::option_offer::get_option_offer_address,
                    ) else {
                        continue;
                    };

                    let metadata = metadata_bytes
                        .as_ref()
                        .and_then(|b| ContractMetadata::from_bytes(b).ok())
                        .unwrap_or_default();

                    let collateral_asset = option_offer_args.get_collateral_asset_id();
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
                        active_offers.push(LocalOptionOfferData {
                            option_offer_args,
                            taproot_pubkey_gen,
                            metadata,
                            current_outpoint: outpoint,
                            current_value: value,
                        });
                    }
                }

                let selected_offer = if let Some(event_id_str) = offer_event {
                    active_offers
                        .into_iter()
                        .find(|s| {
                            s.metadata
                                .nostr_event_id
                                .as_ref()
                                .is_some_and(|id| id.starts_with(event_id_str))
                        })
                        .ok_or_else(|| {
                            Error::Config(format!("Option offer event not found or fully taken: {event_id_str}"))
                        })?
                } else {
                    if active_offers.is_empty() {
                        return Err(Error::Config(
                            "No active option offers found. Run `sync nostr` first to sync events from relays, \
                             then `sync spent` to update UTXO status."
                                .to_string(),
                        ));
                    }

                    let active_offer_displays = build_active_option_offers_displays(&active_offers);
                    display_active_option_offers_table(&active_offer_displays);
                    println!();

                    let selection =
                        crate::cli::interactive::prompt_selection("Select option offer to take", active_offers.len())
                            .map_err(Error::Io)?
                            .ok_or_else(|| Error::Config("Selection cancelled".to_string()))?;

                    active_offers
                        .into_iter()
                        .nth(selection)
                        .ok_or_else(|| Error::Config("Invalid selection".to_string()))?
                };

                let args = &selected_offer.option_offer_args;
                let current_offer_outpoint = selected_offer.current_outpoint;
                let actual_collateral = selected_offer.current_value;

                let event_id_display = selected_offer.metadata.nostr_event_id.as_deref().unwrap_or("local");
                println!("  Offer event: {event_id_display}");
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

                let script_pubkey = wallet.signer().p2pk_address(config.network())?.script_pubkey();
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

                let collateral_txout = cli_helper::explorer::fetch_utxo(current_offer_outpoint).await?;

                let premium_outpoint =
                    simplicityhl::elements::OutPoint::new(current_offer_outpoint.txid, current_offer_outpoint.vout + 1);
                let premium_txout = cli_helper::explorer::fetch_utxo(premium_outpoint).await?;

                let collateral_input = (current_offer_outpoint, collateral_txout.clone());
                let premium_input = (premium_outpoint, premium_txout.clone());
                let settlement_input = (*settlement_utxo.outpoint(), settlement_utxo.txout().clone());
                let fee_input = (*fee_utxo.outpoint(), fee_utxo.txout().clone());

                let actual_fee = if let Some(f) = fee {
                    *f
                } else {
                    let (pst, branch) = contracts::sdk::build_option_offer_exercise(
                        collateral_input.clone(),
                        premium_input.clone(),
                        settlement_input.clone(),
                        fee_input.clone(),
                        collateral_amount_to_receive,
                        PLACEHOLDER_FEE,
                        args,
                        script_pubkey.clone(),
                    )?;
                    let mut tx = pst.extract_tx()?;
                    let utxos = vec![
                        collateral_txout.clone(),
                        premium_txout.clone(),
                        settlement_input.1.clone(),
                        fee_input.1.clone(),
                    ];
                    let offer_program = get_option_offer_program(args)?;
                    tx = finalize_option_offer_transaction(
                        tx,
                        &selected_offer.taproot_pubkey_gen.get_x_only_pubkey(),
                        &offer_program,
                        &utxos,
                        0,
                        &branch,
                        config.network(),
                        TrackerLogLevel::None,
                    )?;
                    tx = finalize_option_offer_transaction(
                        tx,
                        &selected_offer.taproot_pubkey_gen.get_x_only_pubkey(),
                        &offer_program,
                        &utxos,
                        1,
                        &branch,
                        config.network(),
                        TrackerLogLevel::None,
                    )?;
                    let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.network(), 2)?;
                    let signed_weight = tx.weight();
                    let fee_rate = config.get_fee_rate();
                    let estimated = crate::fee::calculate_fee(signed_weight, fee_rate);
                    println!(
                        "Estimated fee: {estimated} sats (signed weight: {signed_weight}, rate: {fee_rate} sats/kvb)"
                    );
                    estimated
                };

                println!("  Fee: {actual_fee} sats");

                let (pst, branch) = contracts::sdk::build_option_offer_exercise(
                    collateral_input.clone(),
                    premium_input.clone(),
                    settlement_input.clone(),
                    fee_input.clone(),
                    collateral_amount_to_receive,
                    actual_fee,
                    args,
                    script_pubkey.clone(),
                )?;

                let mut tx = pst.extract_tx()?;
                let utxos = vec![
                    collateral_txout.clone(),
                    premium_txout.clone(),
                    settlement_input.1.clone(),
                    fee_input.1.clone(),
                ];

                let offer_program = get_option_offer_program(args)?;
                tx = finalize_option_offer_transaction(
                    tx,
                    &selected_offer.taproot_pubkey_gen.get_x_only_pubkey(),
                    &offer_program,
                    &utxos,
                    0,
                    &branch,
                    config.network(),
                    TrackerLogLevel::None,
                )?;

                tx = finalize_option_offer_transaction(
                    tx,
                    &selected_offer.taproot_pubkey_gen.get_x_only_pubkey(),
                    &offer_program,
                    &utxos,
                    1,
                    &branch,
                    config.network(),
                    TrackerLogLevel::None,
                )?;

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.network(), 2)?;

                if *broadcast {
                    cli_helper::explorer::broadcast_tx(&tx).await?;
                    println!("Broadcasted: {}", tx.txid());

                    if let Some(ref nostr_event_id) = selected_offer.metadata.nostr_event_id
                        && let Ok(event_id) = nostr::EventId::from_hex(nostr_event_id)
                    {
                        let publishing_client = self.get_publishing_client(&config).await?;

                        let action_event = ActionCompletedEvent::new(
                            event_id,
                            ActionType::OptionOfferExercised,
                            simplicityhl::elements::OutPoint::new(tx.txid(), 0),
                        );

                        let published_id = publishing_client.publish_action_completed(&action_event).await?;
                        println!("Published action to NOSTR: {published_id}");

                        publishing_client.disconnect().await;
                    }

                    wallet.store().insert_transaction(&tx, HashMap::default()).await?;

                    let entry = HistoryEntry::with_txid(
                        ActionType::OptionOfferExercised.as_str(),
                        &tx.txid().to_string(),
                        current_timestamp(),
                    );
                    crate::sync::add_history_entry(wallet.store(), &selected_offer.taproot_pubkey_gen, entry).await?;
                } else {
                    println!("{}", tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
            OptionOfferCommand::Cancel {
                offer_event,
                fee,
                broadcast,
            } => {
                println!("Cancelling option offer (reclaiming collateral + premium after expiry)...");

                let offer_contracts =
                    <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), OPTION_OFFER_SOURCE)
                        .await?;

                if offer_contracts.is_empty() {
                    return Err(Error::Config(
                        "No option offer contracts found in local database. Create an offer first or run `sync nostr` to import."
                            .to_string(),
                    ));
                }

                println!("Checking offer status...");

                let mut cancellable_offers: Vec<LocalCancellableOptionOffer> = Vec::new();

                for (args_bytes, tpg_str, metadata_bytes) in offer_contracts {
                    let Ok((arguments, _)): Result<(simplicityhl::Arguments, usize), _> =
                        bincode::serde::decode_from_slice(&args_bytes, bincode::config::standard())
                    else {
                        continue;
                    };
                    let Ok(option_offer_args) = OptionOfferArguments::from_arguments(&arguments) else {
                        continue;
                    };

                    let is_expired = current_timestamp() > i64::from(option_offer_args.expiry_time());
                    if !is_expired {
                        continue; // Skip non-expired offers
                    }

                    let Ok(taproot_pubkey_gen) = contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen::build_from_str(
                        &tpg_str,
                        &option_offer_args,
                        config.network(),
                        &contracts::option_offer::get_option_offer_address,
                    ) else {
                        continue;
                    };

                    let metadata = metadata_bytes
                        .as_ref()
                        .and_then(|b| ContractMetadata::from_bytes(b).ok())
                        .unwrap_or_default();

                    let collateral_asset = option_offer_args.get_collateral_asset_id();
                    let filter = UtxoFilter::new()
                        .taproot_pubkey_gen(taproot_pubkey_gen.clone())
                        .asset_id(collateral_asset);

                    if let Ok(results) = <_ as UtxoStore>::query_utxos(wallet.store(), &[filter]).await
                        && let UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _) =
                            &results[0]
                        && let Some(entry) = entries.first()
                        && let Some(collateral_value) = entry.value()
                    {
                        // Calculate premium: collateral * premium_per_collateral rate
                        let premium_amount = collateral_value * option_offer_args.premium_per_collateral();
                        cancellable_offers.push(LocalCancellableOptionOffer {
                            option_offer_args,
                            taproot_pubkey_gen,
                            metadata,
                            collateral_amount: collateral_value,
                            premium_amount,
                        });
                    }
                }

                if cancellable_offers.is_empty() {
                    return Err(Error::Config(
                        "No cancellable offers found. Offers must be expired and still have collateral. Run `sync utxos` first.".to_string(),
                    ));
                }

                let cancellable_offer_displays = build_cancellable_option_offers_displays(&cancellable_offers);
                display_cancellable_option_offers_table(&cancellable_offer_displays);
                println!();

                let selected = if let Some(event_id_str) = offer_event {
                    cancellable_offers
                        .into_iter()
                        .find(|cs| {
                            cs.metadata
                                .nostr_event_id
                                .as_ref()
                                .is_some_and(|id| id.starts_with(event_id_str))
                        })
                        .ok_or_else(|| Error::Config(format!("Offer event not found: {event_id_str}")))?
                } else {
                    let selection = crate::cli::interactive::prompt_selection(
                        "Select option offer to cancel",
                        cancellable_offers.len(),
                    )
                    .map_err(Error::Io)?
                    .ok_or_else(|| Error::Config("Selection cancelled".to_string()))?;

                    cancellable_offers
                        .into_iter()
                        .nth(selection)
                        .ok_or_else(|| Error::Config("Invalid selection".to_string()))?
                };

                let args = &selected.option_offer_args;
                let taproot_pubkey_gen = &selected.taproot_pubkey_gen;

                if let Some(ref event_id) = selected.metadata.nostr_event_id {
                    println!("  Offer event: {event_id}");
                }

                let initial_fee = fee.unwrap_or(PLACEHOLDER_FEE);

                let script_pubkey = wallet.signer().p2pk_address(config.network())?.script_pubkey();
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
                let offer_entry = match &results[0] {
                    UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _) => {
                        entries.first().ok_or_else(|| Error::Config(
                            "No collateral UTXO found at contract address. Offer may have been taken. Run `sync utxos` to update.".to_string()
                        ))?
                    }
                    UtxoQueryResult::Empty => {
                        return Err(Error::Config(
                            "No collateral UTXO found at contract address. Offer may have been taken. Run `sync utxos` to update.".to_string()
                        ));
                    }
                };

                let current_outpoint = *offer_entry.outpoint();
                let collateral_txout = offer_entry.txout().clone();

                let premium_outpoint =
                    simplicityhl::elements::OutPoint::new(current_outpoint.txid, current_outpoint.vout + 1);
                let premium_txout = cli_helper::explorer::fetch_utxo(premium_outpoint).await?;

                let collateral_input = (current_outpoint, collateral_txout.clone());
                let premium_input = (premium_outpoint, premium_txout.clone());

                let actual_fee = if let Some(f) = fee {
                    *f
                } else {
                    let pst = contracts::sdk::build_option_offer_expiry(
                        collateral_input.clone(),
                        premium_input.clone(),
                        fee_input.clone(),
                        PLACEHOLDER_FEE,
                        args,
                        script_pubkey.clone(),
                    )?;
                    let mut tx = pst.extract_tx()?;
                    let utxos = vec![collateral_txout.clone(), premium_txout.clone(), fee_input.1.clone()];
                    let offer_program = get_option_offer_program(args)?;
                    let signature = wallet.signer().sign_contract(
                        &tx,
                        &offer_program,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &utxos,
                        0,
                        config.network(),
                    )?;
                    let branch = contracts::option_offer::build_witness::OptionOfferBranch::Expiry {
                        schnorr_signature: signature,
                    };
                    tx = finalize_option_offer_transaction(
                        tx,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &offer_program,
                        &utxos,
                        0,
                        &branch,
                        config.network(),
                        TrackerLogLevel::None,
                    )?;
                    let signature = wallet.signer().sign_contract(
                        &tx,
                        &offer_program,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &utxos,
                        1,
                        config.network(),
                    )?;
                    let branch = contracts::option_offer::build_witness::OptionOfferBranch::Expiry {
                        schnorr_signature: signature,
                    };
                    tx = finalize_option_offer_transaction(
                        tx,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &offer_program,
                        &utxos,
                        1,
                        &branch,
                        config.network(),
                        TrackerLogLevel::None,
                    )?;
                    let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.network(), 2)?;
                    let signed_weight = tx.weight();
                    let fee_rate = config.get_fee_rate();
                    let estimated = crate::fee::calculate_fee(signed_weight, fee_rate);
                    println!(
                        "Estimated fee: {estimated} sats (signed weight: {signed_weight}, rate: {fee_rate} sats/kvb)"
                    );
                    estimated
                };

                println!("  Fee: {actual_fee} sats");

                let pst = contracts::sdk::build_option_offer_expiry(
                    collateral_input.clone(),
                    premium_input.clone(),
                    fee_input.clone(),
                    actual_fee,
                    args,
                    script_pubkey.clone(),
                )?;

                let mut tx = pst.extract_tx()?;
                let utxos = vec![collateral_txout.clone(), premium_txout.clone(), fee_input.1.clone()];
                let offer_program = get_option_offer_program(args)?;

                let signature = wallet.signer().sign_contract(
                    &tx,
                    &offer_program,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &utxos,
                    0,
                    config.network(),
                )?;

                let branch = contracts::option_offer::build_witness::OptionOfferBranch::Expiry {
                    schnorr_signature: signature,
                };

                tx = finalize_option_offer_transaction(
                    tx,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &offer_program,
                    &utxos,
                    0,
                    &branch,
                    config.network(),
                    TrackerLogLevel::None,
                )?;

                let signature = wallet.signer().sign_contract(
                    &tx,
                    &offer_program,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &utxos,
                    1,
                    config.network(),
                )?;

                let branch = contracts::option_offer::build_witness::OptionOfferBranch::Expiry {
                    schnorr_signature: signature,
                };

                tx = finalize_option_offer_transaction(
                    tx,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &offer_program,
                    &utxos,
                    1,
                    &branch,
                    config.network(),
                    TrackerLogLevel::None,
                )?;

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.network(), 2)?;

                if *broadcast {
                    cli_helper::explorer::broadcast_tx(&tx).await?;
                    println!("Broadcasted: {}", tx.txid());

                    if let Some(ref nostr_event_id) = selected.metadata.nostr_event_id
                        && let Ok(event_id) = nostr::EventId::from_hex(nostr_event_id)
                    {
                        let publishing_client = self.get_publishing_client(&config).await?;

                        let action_event = ActionCompletedEvent::new(
                            event_id,
                            ActionType::OptionOfferCancelled,
                            simplicityhl::elements::OutPoint::new(tx.txid(), 0),
                        );

                        let published_id = publishing_client.publish_action_completed(&action_event).await?;
                        println!("Published cancellation to NOSTR: {published_id}");

                        publishing_client.disconnect().await;
                    }

                    wallet.store().insert_transaction(&tx, HashMap::default()).await?;

                    let entry = HistoryEntry::with_txid(
                        ActionType::OptionOfferCancelled.as_str(),
                        &tx.txid().to_string(),
                        current_timestamp(),
                    );
                    crate::sync::add_history_entry(wallet.store(), taproot_pubkey_gen, entry).await?;
                } else {
                    println!("{}", tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
            OptionOfferCommand::Withdraw {
                offer_event,
                fee,
                broadcast,
            } => {
                println!("Withdrawing settlement from option offer (claiming payment after offer was taken)...");

                let offer_contracts =
                    <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), OPTION_OFFER_SOURCE)
                        .await?;

                if offer_contracts.is_empty() {
                    return Err(Error::Config(
                        "No option offer contracts found in local database. Create an offer first or run `sync nostr` to import."
                            .to_string(),
                    ));
                }

                println!("Checking offer status...");

                let mut withdrawable_offers: Vec<LocalWithdrawableOptionOffer> = Vec::new();

                for (args_bytes, tpg_str, metadata_bytes) in offer_contracts {
                    let Ok((arguments, _)): Result<(simplicityhl::Arguments, usize), _> =
                        bincode::serde::decode_from_slice(&args_bytes, bincode::config::standard())
                    else {
                        continue;
                    };
                    let Ok(option_offer_args) = OptionOfferArguments::from_arguments(&arguments) else {
                        continue;
                    };

                    let Ok(taproot_pubkey_gen) = contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen::build_from_str(
                        &tpg_str,
                        &option_offer_args,
                        config.network(),
                        &contracts::option_offer::get_option_offer_address,
                    ) else {
                        continue;
                    };

                    let metadata = metadata_bytes
                        .as_ref()
                        .and_then(|b| ContractMetadata::from_bytes(b).ok())
                        .unwrap_or_default();

                    let settlement_asset = option_offer_args.get_settlement_asset_id();
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
                        let contract_user_pubkey = option_offer_args.user_pubkey();
                        if wallet_pubkey.serialize() == contract_user_pubkey {
                            withdrawable_offers.push(LocalWithdrawableOptionOffer {
                                option_offer_args,
                                taproot_pubkey_gen,
                                metadata,
                                settlement_amount: value,
                            });
                        }
                    }
                }

                if withdrawable_offers.is_empty() {
                    return Err(Error::Config(
                        "No withdrawable offers found. Either:\n  - No offers have been taken yet\n  - The taken offers belong to a different wallet (pubkey mismatch)\n  - Run `sync utxos` to update"
                            .to_string(),
                    ));
                }

                let withdrawable_offer_displays = build_withdrawable_option_offers_displays(&withdrawable_offers);
                display_withdrawable_option_offers_table(&withdrawable_offer_displays);
                println!();

                let selected = if let Some(event_id_str) = offer_event {
                    withdrawable_offers
                        .into_iter()
                        .find(|ws| {
                            ws.metadata
                                .nostr_event_id
                                .as_ref()
                                .is_some_and(|id| id.starts_with(event_id_str))
                        })
                        .ok_or_else(|| Error::Config(format!("Offer event not found: {event_id_str}")))?
                } else {
                    let selection = crate::cli::interactive::prompt_selection(
                        "Select offer to withdraw from",
                        withdrawable_offers.len(),
                    )
                    .map_err(Error::Io)?
                    .ok_or_else(|| Error::Config("Selection cancelled".to_string()))?;

                    withdrawable_offers
                        .into_iter()
                        .nth(selection)
                        .ok_or_else(|| Error::Config("Invalid selection".to_string()))?
                };

                let args = &selected.option_offer_args;
                let taproot_pubkey_gen = &selected.taproot_pubkey_gen;

                if let Some(ref event_id) = selected.metadata.nostr_event_id {
                    println!("  Offer event: {event_id}");
                }

                let initial_fee = fee.unwrap_or(PLACEHOLDER_FEE);

                let script_pubkey = wallet.signer().p2pk_address(config.network())?.script_pubkey();
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
                let offer_entry = match &results[0] {
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

                let current_outpoint = *offer_entry.outpoint();
                let offer_txout = offer_entry.txout().clone();
                let offer_input = (current_outpoint, offer_txout.clone());

                let actual_fee = if let Some(f) = fee {
                    *f
                } else {
                    let pst = contracts::sdk::build_option_offer_withdraw(
                        offer_input.clone(),
                        fee_input.clone(),
                        PLACEHOLDER_FEE,
                        args,
                        script_pubkey.clone(),
                    )?;
                    let mut tx = pst.extract_tx()?;
                    let utxos = vec![offer_txout.clone(), fee_input.1.clone()];
                    let offer_program = get_option_offer_program(args)?;
                    let signature = wallet.signer().sign_contract(
                        &tx,
                        &offer_program,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &utxos,
                        0,
                        config.network(),
                    )?;
                    let branch = contracts::option_offer::build_witness::OptionOfferBranch::Withdraw {
                        schnorr_signature: signature,
                    };
                    tx = finalize_option_offer_transaction(
                        tx,
                        &taproot_pubkey_gen.get_x_only_pubkey(),
                        &offer_program,
                        &utxos,
                        0,
                        &branch,
                        config.network(),
                        TrackerLogLevel::None,
                    )?;
                    let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.network(), 1)?;
                    let signed_weight = tx.weight();
                    let fee_rate = config.get_fee_rate();
                    let estimated = crate::fee::calculate_fee(signed_weight, fee_rate);
                    println!(
                        "Estimated fee: {estimated} sats (signed weight: {signed_weight}, rate: {fee_rate} sats/kvb)"
                    );
                    estimated
                };

                println!("  Fee: {actual_fee} sats");

                let pst = contracts::sdk::build_option_offer_withdraw(
                    offer_input.clone(),
                    fee_input.clone(),
                    actual_fee,
                    args,
                    script_pubkey.clone(),
                )?;

                let mut tx = pst.extract_tx()?;
                let utxos = vec![offer_txout.clone(), fee_input.1.clone()];
                let offer_program = get_option_offer_program(args)?;

                let signature = wallet.signer().sign_contract(
                    &tx,
                    &offer_program,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &utxos,
                    0,
                    config.network(),
                )?;

                let branch = contracts::option_offer::build_witness::OptionOfferBranch::Withdraw {
                    schnorr_signature: signature,
                };

                tx = finalize_option_offer_transaction(
                    tx,
                    &taproot_pubkey_gen.get_x_only_pubkey(),
                    &offer_program,
                    &utxos,
                    0,
                    &branch,
                    config.network(),
                    TrackerLogLevel::None,
                )?;

                let tx = sign_p2pk_inputs(tx, &utxos, &wallet, config.network(), 1)?;

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

                    let entry =
                        HistoryEntry::with_txid("option_offer_withdrawn", &tx.txid().to_string(), current_timestamp());
                    crate::sync::add_history_entry(wallet.store(), taproot_pubkey_gen, entry).await?;
                } else {
                    println!("{}", tx.serialize().to_lower_hex_string());
                }

                Ok(())
            }
        }
    }
}

fn build_active_option_offers_displays(active_offers: &[LocalOptionOfferData]) -> Vec<ActiveOptionOfferDisplay> {
    active_offers
        .iter()
        .enumerate()
        .map(|(idx, offer)| {
            let seller = offer.metadata.nostr_author.as_deref().unwrap_or("unknown");
            let price = offer.option_offer_args.collateral_per_contract();
            ActiveOptionOfferDisplay {
                index: idx + 1,
                offering: offer.current_value.to_string(),
                price: price.to_string(),
                wants: format_settlement_asset(&offer.option_offer_args.get_settlement_asset_id()),
                expires: format_relative_time(i64::from(offer.option_offer_args.expiry_time())),
                seller: truncate_with_ellipsis(seller, 12),
            }
        })
        .collect()
}

fn build_cancellable_option_offers_displays(
    cancellable_offers: &[LocalCancellableOptionOffer],
) -> Vec<CancellableOptionOfferDisplay> {
    cancellable_offers
        .iter()
        .enumerate()
        .map(|(idx, cs)| {
            let expiry_time = cs.option_offer_args.expiry_time();
            let contract_short = cs.metadata.nostr_event_id.as_ref().map_or_else(
                || truncate_with_ellipsis(&cs.taproot_pubkey_gen.to_string(), 16),
                |id| truncate_with_ellipsis(id, 16),
            );
            let premium_display = if cs.premium_amount > 0 {
                format!(
                    "{} {}",
                    cs.premium_amount,
                    format_settlement_asset(&cs.option_offer_args.get_premium_asset_id())
                )
            } else {
                "0".to_string()
            };
            CancellableOptionOfferDisplay {
                index: idx + 1,
                collateral: cs.collateral_amount.to_string(),
                premium: premium_display,
                asset: format_settlement_asset(&cs.option_offer_args.get_collateral_asset_id()),
                expired: format!("expired ({expiry_time})"),
                contract: contract_short,
            }
        })
        .collect()
}

fn build_withdrawable_option_offers_displays(
    withdrawable_offers: &[LocalWithdrawableOptionOffer],
) -> Vec<WithdrawableOptionOfferDisplay> {
    withdrawable_offers
        .iter()
        .enumerate()
        .map(|(idx, ws)| {
            let contract_short = ws.metadata.nostr_event_id.as_ref().map_or_else(
                || truncate_with_ellipsis(&ws.taproot_pubkey_gen.to_string(), 16),
                |id| truncate_with_ellipsis(id, 16),
            );
            WithdrawableOptionOfferDisplay {
                index: idx + 1,
                settlement: ws.settlement_amount.to_string(),
                asset: format_settlement_asset(&ws.option_offer_args.get_settlement_asset_id()),
                contract: contract_short,
            }
        })
        .collect()
}
