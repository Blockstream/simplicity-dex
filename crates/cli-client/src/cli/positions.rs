use crate::cli::Cli;
use crate::cli::interactive::{
    EnrichedTokenEntry, GRANTOR_TOKEN_TAG, OPTION_TOKEN_TAG, TokenDisplay, format_asset_value_with_tag,
    format_asset_with_tag, format_relative_time, format_settlement_asset, format_time_ago,
    get_grantor_tokens_from_wallet, get_option_tokens_from_wallet, truncate_with_ellipsis,
};
use crate::cli::tables::{display_collateral_table, display_token_table, display_user_token_table};
use crate::config::Config;
use crate::error::Error;
use crate::metadata::ContractMetadata;

use crate::price_fetcher::{CoingeckoPriceFetcher, PriceFetcherError, fetch_btc_usd_price};
use coin_store::{Store, UtxoEntry, UtxoFilter, UtxoQueryResult, UtxoStore};
use contracts::option_offer::{OPTION_OFFER_SOURCE, OptionOfferArguments, get_option_offer_address};
use contracts::options::{OPTION_SOURCE, OptionsArguments, get_options_address};
use contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen;
use simplicityhl::elements::Address;

/// Result type for contract info queries: (metadata, arguments, `taproot_pubkey_gen`)
type ContractInfoResult = Result<Option<(Vec<u8>, Vec<u8>, String)>, coin_store::StoreError>;

impl Cli {
    #[allow(clippy::too_many_lines)]
    pub(crate) async fn run_positions(&self, config: Config) -> Result<(), Error> {
        let wallet = self.get_wallet(&config).await?;

        println!("Your Positions:");
        println!("===============");
        println!();

        let fetcher = CoingeckoPriceFetcher;
        let btc_result = tokio::task::spawn_blocking(move || fetch_btc_usd_price(&fetcher))
            .await
            .unwrap_or_else(|e| Err(PriceFetcherError::Internal(e.to_string())));

        let btc_price = match btc_result {
            Ok(price) => format!("${price:.2}"),
            Err(PriceFetcherError::RateLimit) => "Rate limit exceeded".to_string(),
            Err(e) => {
                eprintln!("Fetcher error: {e}");
                "Price fetcher service unavailable".to_string()
            }
        };

        println!("Current btc price: {btc_price}");
        println!("-----------------------------");
        println!();

        let user_script_pubkey = wallet.signer().p2pk_address(config.network())?.script_pubkey();

        let options_filter = UtxoFilter::new().source(OPTION_SOURCE);
        let options_results = <_ as UtxoStore>::query_utxos(wallet.store(), &[options_filter]).await?;
        let option_entries = extract_entries(options_results);

        let collateral_displays = build_collateral_displays(&wallet, &option_entries, config.network()).await;

        println!("Option Contract Locked Assets:");
        println!("------------------------------");
        display_collateral_table(&collateral_displays);
        println!();

        let option_tokens = get_option_tokens_from_wallet(&wallet, OPTION_SOURCE, &user_script_pubkey).await?;
        let grantor_tokens = get_grantor_tokens_from_wallet(&wallet, OPTION_SOURCE, &user_script_pubkey).await?;

        let user_token_displays = build_user_token_displays(&option_tokens, &grantor_tokens, config.network());

        println!("Your Option/Grantor Tokens:");
        println!("---------------------------");
        display_user_token_table(&user_token_displays);
        println!();

        let option_offer_filter = UtxoFilter::new().source(OPTION_OFFER_SOURCE);
        let option_offer_results = <_ as UtxoStore>::query_utxos(wallet.store(), &[option_offer_filter]).await?;
        let option_offer_entries = extract_entries(option_offer_results);

        let option_offer_displays = build_option_offer_displays_with_args(&wallet, &option_offer_entries).await;

        println!("Pending Option Offers:");
        println!("----------------------");
        display_token_table(&option_offer_displays);

        println!();
        println!("Contract History:");
        println!("-----------------");

        let option_contracts =
            <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), OPTION_SOURCE).await?;
        let option_offer_contracts =
            <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), OPTION_OFFER_SOURCE).await?;

        let mut contracts_with_history: Vec<(&str, Address, ContractMetadata, i64)> = Vec::new();

        for (args_bytes, tpg_str, metadata_bytes) in &option_contracts {
            if let Some(bytes) = metadata_bytes
                && let Ok(metadata) = ContractMetadata::from_bytes(bytes)
                && !metadata.history.is_empty()
            {
                let Ok((args, _)) = bincode::serde::decode_from_slice::<simplicityhl::Arguments, _>(
                    args_bytes,
                    bincode::config::standard(),
                ) else {
                    continue;
                };
                let Ok(opt_args) = OptionsArguments::from_arguments(&args) else {
                    continue;
                };
                let Ok(tpg) =
                    TaprootPubkeyGen::build_from_str(tpg_str, &opt_args, config.network(), &get_options_address)
                else {
                    continue;
                };
                let most_recent = metadata.history.iter().map(|h| h.timestamp).max().unwrap_or(0);
                contracts_with_history.push(("Option", tpg.address, metadata, most_recent));
            }
        }

        for (args_bytes, tpg_str, metadata_bytes) in &option_offer_contracts {
            if let Some(bytes) = metadata_bytes
                && let Ok(metadata) = ContractMetadata::from_bytes(bytes)
                && !metadata.history.is_empty()
            {
                let Ok((args, _)) = bincode::serde::decode_from_slice::<simplicityhl::Arguments, _>(
                    args_bytes,
                    bincode::config::standard(),
                ) else {
                    continue;
                };
                let Ok(option_offer_args) = OptionOfferArguments::from_arguments(&args) else {
                    continue;
                };
                let Ok(tpg) = TaprootPubkeyGen::build_from_str(
                    tpg_str,
                    &option_offer_args,
                    config.network(),
                    &get_option_offer_address,
                ) else {
                    continue;
                };
                let most_recent = metadata.history.iter().map(|h| h.timestamp).max().unwrap_or(0);
                contracts_with_history.push(("OptionOffer", tpg.address, metadata, most_recent));
            }
        }

        contracts_with_history.sort_by(|a, b| b.3.cmp(&a.3));

        for (contract_type, address, metadata, _) in &contracts_with_history {
            let short_addr = format_contract_address(address);
            println!("\n  {contract_type} Contract {short_addr}:");
            for entry in &metadata.history {
                let time_str = format_time_ago(entry.timestamp);
                let txid_str = entry.txid.as_deref().map_or("N/A", |t| &t[..t.len().min(12)]);
                println!("    - {} @ {} (tx: {}...)", entry.action, time_str, txid_str);
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

/// Display struct for contract collateral
#[derive(Debug, Clone)]
pub struct CollateralDisplay {
    pub index: usize,
    pub collateral: String,
    pub settlement: String,
    pub expires: String,
    pub contract: String,
}

/// Display struct for user-owned option/grantor tokens
#[derive(Debug, Clone)]
pub struct UserTokenDisplay {
    pub index: usize,
    pub token_type: String,
    pub amount: String,
    pub strike: String,
    pub expires: String,
    pub contract: String,
}

/// Build locked asset displays, filtering to only show collateral or settlement assets (not reissuance tokens)
async fn build_collateral_displays(
    wallet: &crate::wallet::Wallet,
    entries: &[UtxoEntry],
    network: simplicityhl_core::SimplicityNetwork,
) -> Vec<CollateralDisplay> {
    let mut displays = Vec::new();
    let mut display_idx = 0;

    for entry in entries {
        let script_pubkey = entry.txout().script_pubkey.clone();
        let contract_info = <_ as UtxoStore>::get_contract_by_script_pubkey(wallet.store(), &script_pubkey).await;

        // Try to get option arguments to check if this is collateral
        let Some(info) = extract_collateral_info(wallet.store(), contract_info, entry, network).await else {
            continue;
        };

        display_idx += 1;
        displays.push(CollateralDisplay {
            index: display_idx,
            collateral: info.0,
            settlement: info.1,
            expires: info.2,
            contract: info.3,
        });
    }

    displays
}

/// Extract contract asset info, returning None if this UTXO is not a collateral or settlement asset (e.g., reissuance token)
async fn extract_collateral_info(
    store: &Store,
    contract_info: ContractInfoResult,
    entry: &UtxoEntry,
    network: simplicityhl_core::SimplicityNetwork,
) -> Option<(String, String, String, String)> {
    let (_metadata, args_bytes, tpg_str) = contract_info.ok().flatten()?;

    let (args, _) =
        bincode::serde::decode_from_slice::<simplicityhl::Arguments, _>(&args_bytes, bincode::config::standard())
            .ok()?;

    let opt_args = OptionsArguments::from_arguments(&args).ok()?;

    let entry_asset = entry.asset()?;
    let is_collateral = entry_asset == opt_args.get_collateral_asset_id();
    let is_settlement = entry_asset == opt_args.get_settlement_asset_id();
    if !is_collateral && !is_settlement {
        return None;
    }

    let tpg = TaprootPubkeyGen::build_from_str(&tpg_str, &opt_args, network, &get_options_address).ok()?;

    let locked_str = format_asset_value_with_tag(store, entry.value(), entry.asset()).await;
    let settlement_str = format_asset_with_tag(store, &opt_args.get_settlement_asset_id()).await;
    let expiry_str = format_relative_time(i64::from(opt_args.expiry_time()));
    let contract_str = format_contract_address(&tpg.address);

    Some((locked_str, settlement_str, expiry_str, contract_str))
}

/// Build user token displays from option and grantor tokens
fn build_user_token_displays(
    option_tokens: &[EnrichedTokenEntry],
    grantor_tokens: &[EnrichedTokenEntry],
    network: simplicityhl_core::SimplicityNetwork,
) -> Vec<UserTokenDisplay> {
    let mut displays = Vec::new();
    let mut idx = 0;

    // Add option tokens
    for entry in option_tokens {
        idx += 1;
        let settlement_asset = entry.option_arguments.get_settlement_asset_id();
        let settlement_per_contract = entry.option_arguments.settlement_per_contract();
        let expiry_time = entry.option_arguments.expiry_time();

        let contract_addr = TaprootPubkeyGen::build_from_str(
            &entry.taproot_pubkey_gen_str,
            &entry.option_arguments,
            network,
            &get_options_address,
        )
        .map_or_else(|_| "???".to_string(), |tpg| format_contract_address(&tpg.address));

        displays.push(UserTokenDisplay {
            index: idx,
            token_type: OPTION_TOKEN_TAG.to_string(),
            amount: entry.entry.value().unwrap_or(0).to_string(),
            strike: format!(
                "{} {}",
                settlement_per_contract,
                format_settlement_asset(&settlement_asset)
            ),
            expires: format_relative_time(i64::from(expiry_time)),
            contract: contract_addr,
        });
    }

    // Add grantor tokens
    for entry in grantor_tokens {
        idx += 1;
        let settlement_asset = entry.option_arguments.get_settlement_asset_id();
        let settlement_per_contract = entry.option_arguments.settlement_per_contract();
        let expiry_time = entry.option_arguments.expiry_time();

        let contract_addr = TaprootPubkeyGen::build_from_str(
            &entry.taproot_pubkey_gen_str,
            &entry.option_arguments,
            network,
            &get_options_address,
        )
        .map_or_else(|_| "???".to_string(), |tpg| format_contract_address(&tpg.address));

        displays.push(UserTokenDisplay {
            index: idx,
            token_type: GRANTOR_TOKEN_TAG.to_string(),
            amount: entry.entry.value().unwrap_or(0).to_string(),
            strike: format!(
                "{} {}",
                settlement_per_contract,
                format_settlement_asset(&settlement_asset)
            ),
            expires: format_relative_time(i64::from(expiry_time)),
            contract: contract_addr,
        });
    }

    displays
}

async fn build_option_offer_displays_with_args(
    wallet: &crate::wallet::Wallet,
    entries: &[UtxoEntry],
) -> Vec<TokenDisplay> {
    let mut displays = Vec::new();
    let mut display_idx = 0;

    for entry in entries {
        let script_pubkey = entry.txout().script_pubkey.clone();
        let contract_info = <_ as UtxoStore>::get_contract_by_script_pubkey(wallet.store(), &script_pubkey).await;

        let Some((settlement, expires, is_collateral, price)) =
            extract_option_offer_display_info_with_tags(wallet.store(), contract_info, entry).await
        else {
            continue;
        };

        if !is_collateral {
            continue; // Skip settlement outputs
        }

        let collateral = format_asset_value_with_tag(wallet.store(), entry.value(), entry.asset()).await;

        display_idx += 1;
        displays.push(TokenDisplay {
            index: display_idx,
            collateral,
            settlement,
            expires,
            status: format!("Price: {price}"),
        });
    }

    displays
}

/// Returns (`settlement_display`, `expiry_display`, `is_collateral_asset`, price)
async fn extract_option_offer_display_info_with_tags(
    store: &Store,
    contract_info: ContractInfoResult,
    entry: &UtxoEntry,
) -> Option<(String, String, bool, u64)> {
    let (_metadata, args_bytes, _tpg) = contract_info.ok().flatten()?;

    let (args, _) =
        bincode::serde::decode_from_slice::<simplicityhl::Arguments, _>(&args_bytes, bincode::config::standard())
            .ok()?;

    let option_offer_args = OptionOfferArguments::from_arguments(&args).ok()?;

    let settlement_str = format_asset_with_tag(store, &option_offer_args.get_settlement_asset_id()).await;
    let expiry_str = format_relative_time(i64::from(option_offer_args.expiry_time()));
    let price = option_offer_args.collateral_per_contract();

    let is_collateral = entry
        .asset()
        .is_some_and(|a| a == option_offer_args.get_collateral_asset_id());

    Some((settlement_str, expiry_str, is_collateral, price))
}

/// Format a contract address for display by truncating the bech32 address.
fn format_contract_address(address: &Address) -> String {
    truncate_with_ellipsis(&address.to_string(), 12)
}
