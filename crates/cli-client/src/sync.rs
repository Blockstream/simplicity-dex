use std::collections::HashMap;

use coin_store::{Store, UtxoStore};
use options_relay::{ActionType, OptionCreatedEvent, SwapCreatedEvent};
use simplicityhl_core::derive_public_blinder_key;

use crate::cli::{GRANTOR_TOKEN_TAG, OPTION_TOKEN_TAG, SWAP_COLLATERAL_TAG};
use crate::error::Error;
use crate::explorer::fetch_transaction;
use crate::metadata::ContractMetadata;
use crate::metadata::HistoryEntry;

pub async fn sync_option_event(
    store: &Store,
    event: &OptionCreatedEvent,
    source: &str,
    arguments: simplicityhl::Arguments,
) -> Result<(), Error> {
    #[allow(clippy::cast_possible_wrap)]
    let created_at = event.created_at.as_secs() as i64;

    let history = vec![HistoryEntry::with_txid_and_nostr(
        ActionType::OptionCreated.as_str(),
        &event.utxo.txid.to_string(),
        &event.event_id.to_hex(),
        created_at,
    )];

    let metadata =
        ContractMetadata::from_nostr_with_history(event.event_id.to_hex(), event.pubkey.to_hex(), created_at, history);

    let metadata_bytes = metadata.to_bytes()?;

    store
        .add_contract(
            source,
            arguments,
            event.taproot_pubkey_gen.clone(),
            Some(&metadata_bytes),
        )
        .await?;

    let (option_token_id, _) = event.options_args.get_option_token_ids();
    let (grantor_token_id, _) = event.options_args.get_grantor_token_ids();

    store
        .insert_contract_token(&event.taproot_pubkey_gen, option_token_id, OPTION_TOKEN_TAG)
        .await?;
    store
        .insert_contract_token(&event.taproot_pubkey_gen, grantor_token_id, GRANTOR_TOKEN_TAG)
        .await?;

    if let Err(e) = sync_utxo_with_public_blinder(store, event.utxo).await {
        tracing::debug!("Could not sync option UTXO {}: {} (soft failure)", event.utxo, e);
    }

    Ok(())
}

/// Attempt to fetch and insert a transaction using the public blinder key.
/// This allows others to fund options even without the private blinder key.
/// Fails softly if the transaction cannot be fetched or unblinded.
///
/// Fetches the full transaction and uses `insert_transaction` which:
/// - Marks all input UTXOs as spent (if they exist in our store)
/// - Inserts all non-fee outputs
/// - Handles asset issuance entropy
pub async fn sync_utxo_with_public_blinder(
    store: &Store,
    outpoint: simplicityhl::elements::OutPoint,
) -> Result<(), Error> {
    let tx = fetch_transaction(outpoint.txid)?;

    let blinder_keypair = derive_public_blinder_key();
    let mut blinder_keys = HashMap::new();
    blinder_keys.insert(outpoint.vout as usize, blinder_keypair);

    store.insert_transaction(&tx, blinder_keys).await?;

    Ok(())
}

pub async fn sync_swap_event(
    store: &Store,
    event: &SwapCreatedEvent,
    source: &str,
    arguments: simplicityhl::Arguments,
    parent_option_event_id: Option<String>,
) -> Result<(), Error> {
    #[allow(clippy::cast_possible_wrap)]
    let created_at = event.created_at.as_secs() as i64;

    let history = vec![HistoryEntry::with_txid_and_nostr(
        ActionType::SwapCreated.as_str(),
        &event.utxo.txid.to_string(),
        &event.event_id.to_hex(),
        created_at,
    )];

    let metadata = parent_option_event_id.map_or_else(
        || {
            ContractMetadata::from_nostr_with_history(
                event.event_id.to_hex(),
                event.pubkey.to_hex(),
                created_at,
                history.clone(),
            )
        },
        |parent_id| {
            let mut meta = ContractMetadata::from_nostr_with_parent(
                event.event_id.to_hex(),
                event.pubkey.to_hex(),
                created_at,
                parent_id,
            );
            meta.history.clone_from(&history);
            meta
        },
    );

    let metadata_bytes = metadata.to_bytes()?;

    store
        .add_contract(
            source,
            arguments,
            event.taproot_pubkey_gen.clone(),
            Some(&metadata_bytes),
        )
        .await?;

    let collateral_asset = event.swap_args.get_collateral_asset_id();
    store
        .insert_contract_token(&event.taproot_pubkey_gen, collateral_asset, SWAP_COLLATERAL_TAG)
        .await?;

    if let Err(e) = sync_utxo_with_public_blinder(store, event.utxo).await {
        tracing::debug!("Could not sync swap UTXO {}: {} (soft failure)", event.utxo, e);
    }

    Ok(())
}

pub async fn get_contract_metadata(
    store: &Store,
    taproot_pubkey_gen: &contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen,
) -> Result<Option<ContractMetadata>, Error> {
    let metadata_bytes = store.get_contract_metadata(taproot_pubkey_gen).await?;

    Ok(metadata_bytes.map(|bytes| {
        // Gracefully handle metadata decode failures - use default if corrupted
        ContractMetadata::from_bytes(&bytes).unwrap_or_default()
    }))
}

pub async fn update_contract_metadata(
    store: &Store,
    taproot_pubkey_gen: &contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen,
    metadata: &ContractMetadata,
) -> Result<(), Error> {
    let metadata_bytes = metadata.to_bytes()?;
    store
        .update_contract_metadata(taproot_pubkey_gen, &metadata_bytes)
        .await?;
    Ok(())
}

pub async fn add_history_entry(
    store: &Store,
    taproot_pubkey_gen: &contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen,
    entry: HistoryEntry,
) -> Result<(), Error> {
    if let Some(mut metadata) = get_contract_metadata(store, taproot_pubkey_gen).await? {
        metadata.add_history(entry);
        update_contract_metadata(store, taproot_pubkey_gen, &metadata).await?;
    }
    Ok(())
}

/// Add a history entry only if it doesn't already exist (avoids duplicates).
/// Returns true if the entry was added, false if it was a duplicate.
pub async fn add_history_entry_if_new(
    store: &Store,
    taproot_pubkey_gen: &contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen,
    entry: HistoryEntry,
) -> Result<bool, Error> {
    if let Some(mut metadata) = get_contract_metadata(store, taproot_pubkey_gen).await? {
        let added = metadata.add_history_if_new(entry);
        if added {
            update_contract_metadata(store, taproot_pubkey_gen, &metadata).await?;
        }
        Ok(added)
    } else {
        Ok(false)
    }
}
