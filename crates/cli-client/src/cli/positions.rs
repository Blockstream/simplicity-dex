use crate::cli::Cli;
use crate::cli::interactive::{TokenDisplay, display_token_table, format_relative_time, format_time_ago};
use crate::config::Config;
use crate::error::Error;
use crate::metadata::ContractMetadata;

use coin_store::{UtxoEntry, UtxoFilter, UtxoQueryResult, UtxoStore};
use contracts::options::{OPTION_SOURCE, OptionsArguments};
use contracts::swap_with_change::{SWAP_WITH_CHANGE_SOURCE, SwapWithChangeArguments};
use simplicityhl::elements::hex::ToHex;

/// Result type for contract info queries: (metadata, arguments, `taproot_pubkey_gen`)
type ContractInfoResult = Result<Option<(Vec<u8>, Vec<u8>, String)>, coin_store::StoreError>;

impl Cli {
    pub(crate) async fn run_positions(&self, config: Config) -> Result<(), Error> {
        let wallet = self.get_wallet(&config).await?;

        println!("Your Positions:");
        println!("===============");
        println!();

        let options_filter = UtxoFilter::new().source(OPTION_SOURCE);
        let options_results = <_ as UtxoStore>::query_utxos(wallet.store(), &[options_filter]).await?;
        let option_entries = extract_entries(options_results);

        let option_displays = build_option_displays_with_args(&wallet, &option_entries).await;

        println!("Option/Grantor Tokens:");
        println!("----------------------");
        display_token_table(&option_displays);
        println!();

        let swap_filter = UtxoFilter::new().source(SWAP_WITH_CHANGE_SOURCE);
        let swap_results = <_ as UtxoStore>::query_utxos(wallet.store(), &[swap_filter]).await?;
        let swap_entries = extract_entries(swap_results);

        let swap_displays = build_swap_displays_with_args(&wallet, &swap_entries).await;

        println!("Pending Swaps:");
        println!("--------------");
        display_token_table(&swap_displays);

        println!();
        println!("Contract History:");
        println!("-----------------");

        let contracts = <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), OPTION_SOURCE).await?;
        for (_args_bytes, tpg_str, metadata_bytes) in &contracts {
            if let Some(bytes) = metadata_bytes
                && let Ok(metadata) = ContractMetadata::from_bytes(bytes)
                && !metadata.history.is_empty()
            {
                let short_tpg = truncate_id(tpg_str);
                println!("\n  Option Contract {short_tpg}:");
                for entry in &metadata.history {
                    let time_str = format_time_ago(entry.timestamp);
                    let txid_str = entry.txid.as_deref().map_or("N/A", |t| &t[..t.len().min(12)]);
                    println!("    - {} @ {} (tx: {}...)", entry.action, time_str, txid_str);
                }
            }
        }

        let swap_contracts =
            <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), SWAP_WITH_CHANGE_SOURCE).await?;
        for (_args_bytes, tpg_str, metadata_bytes) in &swap_contracts {
            if let Some(bytes) = metadata_bytes
                && let Ok(metadata) = ContractMetadata::from_bytes(bytes)
                && !metadata.history.is_empty()
            {
                let short_tpg = truncate_id(tpg_str);
                println!("\n  Swap Contract {short_tpg}:");
                for entry in &metadata.history {
                    let time_str = format_time_ago(entry.timestamp);
                    let txid_str = entry.txid.as_deref().map_or("N/A", |t| &t[..t.len().min(12)]);
                    println!("    - {} @ {} (tx: {}...)", entry.action, time_str, txid_str);
                }
            }
        }

        Ok(())
    }
}

fn extract_entries(results: Vec<UtxoQueryResult>) -> Vec<UtxoEntry> {
    results
        .into_iter()
        .flat_map(|r| match r {
            UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _) => entries,
            UtxoQueryResult::Empty => vec![],
        })
        .collect()
}

async fn build_option_displays_with_args(wallet: &crate::wallet::Wallet, entries: &[UtxoEntry]) -> Vec<TokenDisplay> {
    let mut displays = Vec::new();

    for (idx, entry) in entries.iter().enumerate() {
        let script_pubkey = entry.txout().script_pubkey.clone();
        let contract_info = <_ as UtxoStore>::get_contract_by_script_pubkey(wallet.store(), &script_pubkey).await;

        let (settlement, expires, status) = extract_option_display_info(contract_info, entry);

        displays.push(TokenDisplay {
            index: idx + 1,
            outpoint: entry.outpoint().to_string(),
            collateral: format_asset_value(entry.value(), entry.asset()),
            settlement,
            expires,
            status,
        });
    }

    displays
}

fn extract_option_display_info(contract_info: ContractInfoResult, entry: &UtxoEntry) -> (String, String, String) {
    let default = || ("N/A".to_string(), "N/A".to_string(), "Token".to_string());

    let Some((_metadata, args_bytes, _tpg)) = contract_info.ok().flatten() else {
        return default();
    };

    let Ok((args, _)) =
        bincode::serde::decode_from_slice::<simplicityhl::Arguments, _>(&args_bytes, bincode::config::standard())
    else {
        return default();
    };

    let Ok(opt_args) = OptionsArguments::from_arguments(&args) else {
        return default();
    };

    let settlement_str = format_asset_short(&opt_args.get_settlement_asset_id());
    let expiry_str = format_relative_time(i64::from(opt_args.expiry_time()));
    let status_str = if entry.contract().is_some() {
        "Collateral"
    } else {
        "Token"
    };

    (settlement_str, expiry_str, status_str.to_string())
}

async fn build_swap_displays_with_args(wallet: &crate::wallet::Wallet, entries: &[UtxoEntry]) -> Vec<TokenDisplay> {
    let mut displays = Vec::new();
    let mut display_idx = 0;

    for entry in entries {
        let script_pubkey = entry.txout().script_pubkey.clone();
        let contract_info = <_ as UtxoStore>::get_contract_by_script_pubkey(wallet.store(), &script_pubkey).await;

        let Some((settlement, expires, is_collateral, price)) =
            extract_swap_display_info_with_asset_check(contract_info, entry)
        else {
            continue;
        };

        if !is_collateral {
            continue; // Skip settlement outputs
        }

        display_idx += 1;
        displays.push(TokenDisplay {
            index: display_idx,
            outpoint: entry.outpoint().to_string(),
            collateral: format_asset_value(entry.value(), entry.asset()),
            settlement,
            expires,
            status: format!("Price: {price}"),
        });
    }

    displays
}

/// Returns (`settlement_display`, `expiry_display`, `is_collateral_asset`, price)
fn extract_swap_display_info_with_asset_check(
    contract_info: ContractInfoResult,
    entry: &UtxoEntry,
) -> Option<(String, String, bool, u64)> {
    let (_metadata, args_bytes, _tpg) = contract_info.ok().flatten()?;

    let (args, _) =
        bincode::serde::decode_from_slice::<simplicityhl::Arguments, _>(&args_bytes, bincode::config::standard())
            .ok()?;

    let swap_args = SwapWithChangeArguments::from_arguments(&args).ok()?;

    let settlement_str = format_asset_short(&swap_args.get_settlement_asset_id());
    let expiry_str = format_relative_time(i64::from(swap_args.expiry_time()));
    let price = swap_args.collateral_per_contract();

    let is_collateral = entry.asset().is_some_and(|a| a == swap_args.get_collateral_asset_id());

    Some((settlement_str, expiry_str, is_collateral, price))
}

fn format_asset_value(value: Option<u64>, asset: Option<simplicityhl::elements::AssetId>) -> String {
    match (value, asset) {
        (Some(v), Some(a)) => {
            let hex = a.to_hex();
            format!("{v} ({})...", &hex[..hex.len().min(8)])
        }
        (Some(v), None) => format!("{v} (unknown)"),
        _ => "Confidential".to_string(),
    }
}

fn format_asset_short(asset_id: &simplicityhl::elements::AssetId) -> String {
    let hex = asset_id.to_hex();
    format!("({})...", &hex[..hex.len().min(8)])
}

fn truncate_id(s: &str) -> String {
    if s.len() > 12 {
        format!("{}...", &s[..12])
    } else {
        s.to_string()
    }
}
