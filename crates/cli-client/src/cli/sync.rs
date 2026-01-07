use std::collections::{HashMap, HashSet};

use coin_store::UtxoStore;
use contracts::options::OPTION_SOURCE;
use contracts::swap_with_change::SWAP_WITH_CHANGE_SOURCE;
use options_relay::{OptionCreatedEvent, SwapCreatedEvent};
use simplicityhl::elements::hex::ToHex;
use simplicityhl::elements::{OutPoint, Txid};
use simplicityhl_core::derive_public_blinder_key;

use crate::cli::Cli;
use crate::cli::SyncCommand;
use crate::config::Config;
use crate::error::Error;
use crate::explorer::{
    esplora_utxo_to_outpoint, fetch_address_utxos, fetch_outspends, fetch_scripthash_utxos, fetch_tip_height,
    fetch_transaction,
};
use crate::sync::{sync_option_event, sync_swap_event};

#[derive(Default)]
struct SyncStats {
    utxos_checked: usize,
    utxos_marked_spent: usize,
    new_utxos_discovered: usize,
    new_utxos_imported: usize,
    nostr_options_synced: usize,
    nostr_swaps_synced: usize,
    errors: Vec<String>,
}

impl SyncStats {
    fn print_summary(&self) {
        println!();
        println!("=== Sync Summary ===");
        println!("UTXOs checked:        {}", self.utxos_checked);
        println!("UTXOs marked spent:   {}", self.utxos_marked_spent);
        println!("New UTXOs discovered: {}", self.new_utxos_discovered);
        println!("New UTXOs imported:   {}", self.new_utxos_imported);
        println!("NOSTR options synced: {}", self.nostr_options_synced);
        println!("NOSTR swaps synced:   {}", self.nostr_swaps_synced);

        if !self.errors.is_empty() {
            println!();
            println!("Warnings/Errors ({}):", self.errors.len());
            for (i, error) in self.errors.iter().enumerate().take(10) {
                println!("  {}. {}", i + 1, error);
            }
            if self.errors.len() > 10 {
                println!("  ... and {} more", self.errors.len() - 10);
            }
        }
    }
}

impl Cli {
    pub(crate) async fn run_sync(&self, config: Config, command: &SyncCommand) -> Result<(), Error> {
        match command {
            SyncCommand::Full => self.run_sync_full(config).await,
            SyncCommand::Spent => self.run_sync_spent(config).await,
            SyncCommand::Utxos => self.run_sync_utxos(config).await,
            SyncCommand::Nostr => self.run_sync_nostr(config).await,
            SyncCommand::History => self.run_sync_history(config).await,
        }
    }

    /// Full sync: mark spent UTXOs + discover new UTXOs + sync NOSTR events
    async fn run_sync_full(&self, config: Config) -> Result<(), Error> {
        println!("Starting full sync...");
        println!();

        let mut stats = SyncStats::default();

        // Step 1: Discover new UTXOs
        println!();
        println!("[1/3] Discovering new UTXOs via Esplora...");
        self.sync_discover_utxos(&config, &mut stats).await?;

        // Step 2: Sync NOSTR events
        println!();
        println!("[2/3] Syncing from NOSTR relay...");
        self.sync_nostr_events(&config, &mut stats).await?;

        // Step 3: Mark spent UTXOs
        println!("[3/3] Checking for spent UTXOs via Esplora...");
        self.sync_spent_utxos(&config, &mut stats).await?;

        stats.print_summary();

        Ok(())
    }

    /// Only check and mark spent UTXOs as spent via Esplora
    async fn run_sync_spent(&self, config: Config) -> Result<(), Error> {
        println!("Checking for spent UTXOs via Esplora...");
        println!();

        let mut stats = SyncStats::default();
        self.sync_spent_utxos(&config, &mut stats).await?;

        stats.print_summary();
        Ok(())
    }

    /// Only discover new UTXOs for wallet address and tracked contracts via Esplora
    async fn run_sync_utxos(&self, config: Config) -> Result<(), Error> {
        println!("Discovering new UTXOs via Esplora...");
        println!();

        let mut stats = SyncStats::default();
        self.sync_discover_utxos(&config, &mut stats).await?;

        stats.print_summary();
        Ok(())
    }

    /// Only sync options and swaps from NOSTR relay
    async fn run_sync_nostr(&self, config: Config) -> Result<(), Error> {
        println!("Syncing from NOSTR relay...");
        println!();

        let mut stats = SyncStats::default();
        self.sync_nostr_events(&config, &mut stats).await?;

        stats.print_summary();
        Ok(())
    }

    /// Only sync action history for existing contracts from NOSTR (no UTXOs)
    #[allow(clippy::too_many_lines)]
    async fn run_sync_history(&self, config: Config) -> Result<(), Error> {
        println!("Syncing action history for existing contracts...");
        println!();

        let wallet = self.get_wallet(&config).await?;
        let client = self.get_read_only_client(&config).await?;

        let mut actions_synced = 0;
        let mut contracts_checked = 0;
        let mut errors: Vec<String> = Vec::new();

        let option_contracts =
            <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), OPTION_SOURCE).await?;
        let swap_contracts =
            <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), SWAP_WITH_CHANGE_SOURCE).await?;

        println!(
            "  Found {} option contracts and {} swap contracts",
            option_contracts.len(),
            swap_contracts.len()
        );

        // Process option contracts
        for (args_bytes, tpg_str, metadata_bytes) in &option_contracts {
            let Some(meta_bytes) = metadata_bytes else {
                continue;
            };

            let Ok(metadata) = crate::metadata::ContractMetadata::from_bytes(meta_bytes) else {
                continue;
            };

            let Some(nostr_event_id_str) = &metadata.nostr_event_id else {
                continue;
            };

            let Ok(event_id) = nostr::EventId::from_hex(nostr_event_id_str) else {
                errors.push(format!("Invalid event ID: {nostr_event_id_str}"));
                continue;
            };

            let Ok((args, _)) = bincode::serde::decode_from_slice::<simplicityhl::Arguments, _>(
                args_bytes,
                bincode::config::standard(),
            ) else {
                continue;
            };

            let Ok(options_args) = contracts::options::OptionsArguments::from_arguments(&args) else {
                continue;
            };

            let Ok(taproot_pubkey_gen) = contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen::build_from_str(
                tpg_str,
                &options_args,
                config.address_params(),
                &contracts::options::get_options_address,
            ) else {
                errors.push(format!(
                    "Invalid taproot pubkey gen: {}",
                    &tpg_str[..tpg_str.len().min(20)]
                ));
                continue;
            };

            contracts_checked += 1;

            if let Ok(actions) = client.fetch_actions_for_event(event_id).await {
                for action in actions.into_iter().flatten() {
                    #[allow(clippy::cast_possible_wrap)]
                    let timestamp = action.created_at.as_secs() as i64;
                    let entry = crate::metadata::HistoryEntry::with_txid_and_nostr(
                        action.action.as_str(),
                        &action.outpoint.txid.to_string(),
                        &action.event_id.to_hex(),
                        timestamp,
                    );

                    if let Ok(added) =
                        crate::sync::add_history_entry_if_new(wallet.store(), &taproot_pubkey_gen, entry).await
                        && added
                    {
                        actions_synced += 1;
                    }
                }
            }
        }

        // Process swap contracts
        for (args_bytes, tpg_str, metadata_bytes) in &swap_contracts {
            let Some(meta_bytes) = metadata_bytes else {
                continue;
            };

            let Ok(metadata) = crate::metadata::ContractMetadata::from_bytes(meta_bytes) else {
                continue;
            };

            let Some(nostr_event_id_str) = &metadata.nostr_event_id else {
                continue;
            };

            let Ok(event_id) = nostr::EventId::from_hex(nostr_event_id_str) else {
                errors.push(format!("Invalid event ID: {nostr_event_id_str}"));
                continue;
            };

            let Ok((args, _)) = bincode::serde::decode_from_slice::<simplicityhl::Arguments, _>(
                args_bytes,
                bincode::config::standard(),
            ) else {
                continue;
            };

            let Ok(swap_args) = contracts::swap_with_change::SwapWithChangeArguments::from_arguments(&args) else {
                continue;
            };

            let Ok(taproot_pubkey_gen) = contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen::build_from_str(
                tpg_str,
                &swap_args,
                config.address_params(),
                &contracts::swap_with_change::get_swap_with_change_address,
            ) else {
                errors.push(format!(
                    "Invalid taproot pubkey gen: {}",
                    &tpg_str[..tpg_str.len().min(20)]
                ));
                continue;
            };

            contracts_checked += 1;

            if let Ok(actions) = client.fetch_actions_for_event(event_id).await {
                for action in actions.into_iter().flatten() {
                    #[allow(clippy::cast_possible_wrap)]
                    let timestamp = action.created_at.as_secs() as i64;
                    let entry = crate::metadata::HistoryEntry::with_txid_and_nostr(
                        action.action.as_str(),
                        &action.outpoint.txid.to_string(),
                        &action.event_id.to_hex(),
                        timestamp,
                    );

                    if let Ok(added) =
                        crate::sync::add_history_entry_if_new(wallet.store(), &taproot_pubkey_gen, entry).await
                        && added
                    {
                        actions_synced += 1;
                    }
                }
            }
        }

        client.disconnect().await;

        println!();
        println!("=== History Sync Summary ===");
        println!("Contracts checked:    {contracts_checked}");
        println!("Actions synced:       {actions_synced}");

        if !errors.is_empty() {
            println!();
            println!("Warnings/Errors ({}):", errors.len());
            for (i, error) in errors.iter().enumerate().take(10) {
                println!("  {}. {}", i + 1, error);
            }
            if errors.len() > 10 {
                println!("  ... and {} more", errors.len() - 10);
            }
        }

        Ok(())
    }

    /// Check all unspent UTXOs in the store and mark any that have been spent on-chain.
    async fn sync_spent_utxos(&self, config: &Config, stats: &mut SyncStats) -> Result<(), Error> {
        let wallet = self.get_wallet(config).await?;

        let unspent_outpoints = wallet.store().list_unspent_outpoints().await?;
        stats.utxos_checked = unspent_outpoints.len();

        if unspent_outpoints.is_empty() {
            println!("  No unspent UTXOs in store to check.");
            return Ok(());
        }

        println!("  Found {} unspent UTXOs to check...", unspent_outpoints.len());

        let mut by_txid: HashMap<Txid, Vec<u32>> = HashMap::new();
        for outpoint in &unspent_outpoints {
            by_txid.entry(outpoint.txid).or_default().push(outpoint.vout);
        }

        println!("  Checking {} transactions...", by_txid.len());

        let mut spent_count = 0;
        for (txid, vouts) in by_txid {
            match fetch_outspends(txid) {
                Ok(outspends) => {
                    for vout in vouts {
                        if let Some(status) = outspends.get(vout as usize)
                            && status.spent
                        {
                            let outpoint = OutPoint::new(txid, vout);
                            match wallet.store().mark_as_spent(outpoint).await {
                                Ok(true) => {
                                    spent_count += 1;
                                    tracing::debug!("Marked {} as spent", outpoint);
                                }
                                Ok(false) => {
                                    // Already marked or not found
                                }
                                Err(e) => {
                                    stats.errors.push(format!("Failed to mark {outpoint} as spent: {e}"));
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    stats
                        .errors
                        .push(format!("Failed to fetch outspends for {}: {}", txid.to_hex(), e));
                }
            }

            tracing::debug!("Checked transaction {txid}");
        }

        stats.utxos_marked_spent = spent_count;
        println!("  Marked {spent_count} UTXOs as spent.");

        Ok(())
    }

    /// Discover new UTXOs for the wallet address and all tracked contract script pubkeys.
    async fn sync_discover_utxos(&self, config: &Config, stats: &mut SyncStats) -> Result<(), Error> {
        let wallet = self.get_wallet(config).await?;

        let existing_outpoints: HashSet<OutPoint> =
            wallet.store().list_unspent_outpoints().await?.into_iter().collect();

        let mut imported_txids: HashSet<Txid> = HashSet::new();

        match fetch_tip_height() {
            Ok(height) => println!("  Current block height: {height}"),
            Err(e) => stats.errors.push(format!("Failed to fetch tip height: {e}")),
        }

        println!("  Checking wallet address...");
        let wallet_address = wallet.signer().p2pk_address(config.address_params())?;

        match fetch_address_utxos(&wallet_address) {
            Ok(utxos) => {
                stats.new_utxos_discovered += utxos.len();
                println!("    Found {} UTXOs for wallet address", utxos.len());

                for utxo in utxos {
                    match esplora_utxo_to_outpoint(&utxo) {
                        Ok(outpoint) => {
                            if !existing_outpoints.contains(&outpoint) && !imported_txids.contains(&outpoint.txid) {
                                match self
                                    .import_transaction_from_esplora(wallet.store(), outpoint.txid)
                                    .await
                                {
                                    Ok(true) => {
                                        stats.new_utxos_imported += 1;
                                        imported_txids.insert(outpoint.txid);
                                        tracing::debug!("Imported transaction: {}", outpoint.txid);
                                    }
                                    Ok(false) => {
                                        imported_txids.insert(outpoint.txid);
                                    }
                                    Err(e) => {
                                        stats.errors.push(format!("Failed to import tx {}: {e}", outpoint.txid));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            stats.errors.push(format!("Invalid UTXO from Esplora: {e}"));
                        }
                    }

                    tracing::debug!("Checked transaction {}", utxo.txid);
                }
            }
            Err(e) => {
                stats.errors.push(format!("Failed to fetch wallet UTXOs: {e}"));
            }
        }

        println!("  Checking tracked contract addresses...");
        let script_pubkeys = wallet.store().list_tracked_script_pubkeys().await?;
        println!("    Found {} tracked contracts", script_pubkeys.len());

        for script in &script_pubkeys {
            match fetch_scripthash_utxos(script) {
                Ok(utxos) => {
                    stats.new_utxos_discovered += utxos.len();

                    for utxo in utxos {
                        match esplora_utxo_to_outpoint(&utxo) {
                            Ok(outpoint) => {
                                if !existing_outpoints.contains(&outpoint) && !imported_txids.contains(&outpoint.txid) {
                                    match self
                                        .import_transaction_from_esplora(wallet.store(), outpoint.txid)
                                        .await
                                    {
                                        Ok(true) => {
                                            stats.new_utxos_imported += 1;
                                            imported_txids.insert(outpoint.txid);
                                            tracing::debug!("Imported transaction: {}", outpoint.txid);
                                        }
                                        Ok(false) => {
                                            imported_txids.insert(outpoint.txid);
                                        }
                                        Err(e) => {
                                            stats.errors.push(format!("Failed to import tx {}: {e}", outpoint.txid));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                stats.errors.push(format!("Invalid UTXO from Esplora: {e}"));
                            }
                        }

                        tracing::debug!("Checked transaction {}", utxo.txid);
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to fetch UTXOs for scripthash: {}", e);
                }
            }
        }

        println!("  Imported {} new transactions.", imported_txids.len());

        Ok(())
    }

    async fn import_transaction_from_esplora(&self, store: &coin_store::Store, txid: Txid) -> Result<bool, Error> {
        let tx = fetch_transaction(txid)?;

        let blinder_keypair = derive_public_blinder_key();
        let blinder_keys: HashMap<usize, _> = tx
            .output
            .iter()
            .enumerate()
            .filter(|(_, out)| !out.is_fee())
            .filter(|(_, out)| out.asset.is_confidential())
            .map(|(i, _)| (i, blinder_keypair))
            .collect();

        match store.insert_transaction(&tx, blinder_keys).await {
            Ok(()) => Ok(true),
            Err(
                coin_store::StoreError::UtxoAlreadyExists(_)
                | coin_store::StoreError::MissingBlinderKey(_)
                | coin_store::StoreError::Unblind(_),
            ) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Sync options and swaps from NOSTR relay.
    async fn sync_nostr_events(&self, config: &Config, stats: &mut SyncStats) -> Result<(), Error> {
        let wallet = self.get_wallet(config).await?;
        let client = self.get_read_only_client(config).await?;

        println!("  Fetching options from NOSTR...");
        let options_results = client.fetch_options(config.address_params()).await?;
        let valid_options: Vec<OptionCreatedEvent> = options_results.into_iter().filter_map(Result::ok).collect();

        println!("    Found {} valid options", valid_options.len());

        let mut options_already_synced = 0;
        for event in &valid_options {
            let arguments = event.options_args.build_option_arguments();
            match sync_option_event(wallet.store(), event, OPTION_SOURCE, arguments).await {
                Ok(()) => {
                    stats.nostr_options_synced += 1;
                }
                Err(e) => {
                    if e.to_string().contains("UNIQUE constraint") {
                        options_already_synced += 1;
                    } else {
                        stats
                            .errors
                            .push(format!("Failed to sync option {}: {}", event.event_id, e));
                    }
                }
            }
        }
        if options_already_synced > 0 {
            println!("    ({options_already_synced} options already synced)");
        }

        println!("  Fetching swaps from NOSTR...");
        let swaps_results = client.fetch_swaps(config.address_params()).await?;
        let valid_swaps: Vec<SwapCreatedEvent> = swaps_results.into_iter().filter_map(Result::ok).collect();

        println!("    Found {} valid swaps", valid_swaps.len());

        let mut actions_synced = 0;
        let mut swaps_already_synced = 0;
        for swap in &valid_swaps {
            // First sync the swap contract itself
            let arguments = swap.swap_args.build_arguments();
            match sync_swap_event(wallet.store(), swap, SWAP_WITH_CHANGE_SOURCE, arguments, None).await {
                Ok(()) => {
                    stats.nostr_swaps_synced += 1;
                }
                Err(e) => {
                    // Ignore duplicate errors (already synced)
                    if e.to_string().contains("UNIQUE constraint") {
                        swaps_already_synced += 1;
                    } else {
                        stats
                            .errors
                            .push(format!("Failed to sync swap {}: {}", swap.event_id, e));
                    }
                }
            }

            if let Ok(actions) = client.fetch_actions_for_event(swap.event_id).await {
                for action in actions.into_iter().flatten() {
                    #[allow(clippy::cast_possible_wrap)]
                    let timestamp = action.created_at.as_secs() as i64;
                    let entry = crate::metadata::HistoryEntry::with_txid_and_nostr(
                        action.action.as_str(),
                        &action.outpoint.txid.to_string(),
                        &action.event_id.to_hex(),
                        timestamp,
                    );

                    if let Ok(added) =
                        crate::sync::add_history_entry_if_new(wallet.store(), &swap.taproot_pubkey_gen, entry).await
                        && added
                    {
                        actions_synced += 1;
                    }

                    if let Err(e) = crate::sync::sync_utxo_with_public_blinder(wallet.store(), action.outpoint).await {
                        tracing::debug!("Could not sync action UTXO {}: {} (soft failure)", action.outpoint, e);
                    }
                }
            }
        }

        if swaps_already_synced > 0 {
            println!("    ({swaps_already_synced} swaps already synced)");
        }

        client.disconnect().await;

        println!(
            "  Synced {} new options, {} new swaps, {} action events.",
            stats.nostr_options_synced, stats.nostr_swaps_synced, actions_synced
        );

        Ok(())
    }
}
