use std::collections::{HashMap, HashSet};

use crate::cli::Cli;
use crate::cli::interactive::{
    EnrichedTokenEntry, GRANTOR_TOKEN_TAG, OPTION_TOKEN_TAG, TokenDisplay, current_timestamp,
    format_asset_value_with_tag, format_asset_with_tag, format_relative_time, format_settlement_asset, format_time_ago,
    get_grantor_tokens_from_wallet, get_option_tokens_from_wallet, truncate_with_ellipsis,
};
use crate::cli::tables::{
    display_active_options_table, display_collateral_table, display_token_table, display_user_token_table,
};
use crate::config::Config;
use crate::error::Error;
use crate::metadata::ContractMetadata;
use crate::price_fetcher::{CoingeckoPriceFetcher, PriceFetcherError, fetch_btc_usd_price};

use coin_store::{Store, UtxoEntry, UtxoFilter, UtxoQueryResult, UtxoStore};
use contracts::option_offer::{OPTION_OFFER_SOURCE, OptionOfferArguments, get_option_offer_address};
use contracts::options::{OPTION_SOURCE, OptionsArguments, get_options_address};
use contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen;
use simplicityhl::elements::Address;
use simplicityhl::elements::AssetId;

/// Result type for contract info queries: (metadata, arguments, `taproot_pubkey_gen`)
type ContractInfoResult = Result<Option<(Vec<u8>, Vec<u8>, String)>, coin_store::StoreError>;

/// Aggregated state for a single options contract.
struct ContractState {
    args: OptionsArguments,
    address: Address,
    user_options: u64,
    user_grantors: u64,
    locked_in_offers: u64,
    total_collateral: u64,
}

impl ContractState {
    /// A contract is valid if it has not expired and has collateral locked.
    fn is_valid(&self, now: i64) -> bool {
        self.expiry_time() > now && self.total_collateral > 0
    }

    /// A contract is active if someone other than the user holds at least one token.
    fn is_active(&self) -> bool {
        self.try_is_active().unwrap_or(false)
    }

    fn try_is_active(&self) -> Option<bool> {
        let collateral_per_contract = self.args.collateral_per_contract();
        if collateral_per_contract == 0 {
            return Some(false);
        }
        let total_issued = self.total_collateral / collateral_per_contract;
        let user_share = self
            .user_options
            .checked_add(self.user_grantors)?
            .checked_add(self.locked_in_offers)?;
        let held_by_others = total_issued.checked_mul(2)?.checked_sub(user_share)?;
        Some(held_by_others > 0)
    }

    fn expiry_time(&self) -> i64 {
        i64::from(self.args.expiry_time())
    }
}

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

        match btc_result {
            Ok(price) => println!("BTC/USD: ${price:.2}"),
            Err(PriceFetcherError::RateLimit) => eprintln!("BTC price unavailable: rate limit exceeded"),
            Err(e) => eprintln!("BTC price unavailable: {e}"),
        }
        println!();

        let user_script_pubkey = wallet.signer().p2pk_address(config.network())?.script_pubkey();

        let option_tokens = get_option_tokens_from_wallet(&wallet, OPTION_SOURCE, &user_script_pubkey).await?;
        let grantor_tokens = get_grantor_tokens_from_wallet(&wallet, OPTION_SOURCE, &user_script_pubkey).await?;

        let active_options_displays =
            build_active_options_displays(&wallet, &option_tokens, &grantor_tokens, config.network()).await;

        println!("Your Active Options Positions:");
        println!("-----------------------------");
        display_active_options_table(&active_options_displays);
        println!();

        let options_filter = UtxoFilter::new().source(OPTION_SOURCE);
        let options_results = <_ as UtxoStore>::query_utxos(wallet.store(), &[options_filter]).await?;
        let option_entries = extract_entries(options_results);

        let collateral_displays = build_collateral_displays(&wallet, &option_entries, config.network()).await;

        println!("Option Contract Locked Assets:");
        println!("------------------------------");
        display_collateral_table(&collateral_displays);
        println!();

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

/// Display struct for users' active options positions
#[derive(Debug, Clone)]
pub struct ActiveOptionsDisplay {
    pub index: usize,
    pub option_tokens: u64,
    pub grantor_tokens: u64,
    pub expires: String,
    pub contract_id: String,
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

/// Query contract tokens locked as collateral in option-offer contracts.
async fn query_contract_tokens_in_offers(
    wallet: &crate::wallet::Wallet,
    contract_token_ids: &HashSet<AssetId>,
    network: simplicityhl_core::SimplicityNetwork,
) -> HashMap<AssetId, u64> {
    let mut result = HashMap::new();

    let Ok(option_offer_contracts) =
        <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), OPTION_OFFER_SOURCE).await
    else {
        return result;
    };

    let (all_filters, filter_to_asset): (Vec<_>, Vec<_>) = option_offer_contracts
        .into_iter()
        .filter_map(|(args_bytes, tpg_str, _)| {
            let arguments = bincode::serde::decode_from_slice::<simplicityhl::Arguments, _>(
                &args_bytes,
                bincode::config::standard(),
            )
            .ok()?
            .0;

            let args = OptionOfferArguments::from_arguments(&arguments).ok()?;
            let collateral_id = args.get_collateral_asset_id();

            if !contract_token_ids.contains(&collateral_id) {
                return None;
            }

            let tpg = TaprootPubkeyGen::build_from_str(&tpg_str, &args, network, &get_option_offer_address).ok()?;

            Some((
                UtxoFilter::new().taproot_pubkey_gen(tpg).asset_id(collateral_id),
                collateral_id,
            ))
        })
        .unzip();

    if all_filters.is_empty() {
        return result;
    }

    let Ok(results) = <_ as UtxoStore>::query_utxos(wallet.store(), &all_filters).await else {
        return result;
    };

    for (query_result, asset_id) in results.into_iter().zip(filter_to_asset) {
        if let UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _) = query_result {
            let total: u64 = entries.iter().filter_map(UtxoEntry::value).sum();
            if total > 0 {
                result
                    .entry(asset_id)
                    .and_modify(|v| *v = v.saturating_add(total))
                    .or_insert(total);
            }
        }
    }

    result
}

/// Parsed contract data before collateral query.
struct ParsedContract {
    args: OptionsArguments,
    tpg: TaprootPubkeyGen,
}

/// Raw contract data from DB: (`args_bytes`, `tpg_string`, `metadata_bytes`).
type RawContractData = (Vec<u8>, String, Option<Vec<u8>>);

/// Returns (`parsed_contracts_by_id`, `token_to_contract_id`).
fn parse_option_contracts(
    option_contracts: &[RawContractData],
    network: simplicityhl_core::SimplicityNetwork,
) -> (HashMap<String, ParsedContract>, HashMap<AssetId, String>) {
    let mut parsed_contracts: HashMap<String, ParsedContract> = HashMap::new();
    let mut token_to_contract: HashMap<AssetId, String> = HashMap::new();

    for (args_bytes, tpg_str, _metadata_bytes) in option_contracts {
        let Ok((arguments, _)) =
            bincode::serde::decode_from_slice::<simplicityhl::Arguments, _>(args_bytes, bincode::config::standard())
        else {
            continue;
        };
        let Ok(args) = OptionsArguments::from_arguments(&arguments) else {
            continue;
        };
        let Ok(tpg) = TaprootPubkeyGen::build_from_str(tpg_str, &args, network, &get_options_address) else {
            continue;
        };

        let contract_id = tpg_str.clone();
        token_to_contract.insert(args.option_token(), contract_id.clone());
        let (grantor_token_id, _) = args.get_grantor_token_ids();
        token_to_contract.insert(grantor_token_id, contract_id.clone());
        parsed_contracts.insert(contract_id, ParsedContract { args, tpg });
    }

    (parsed_contracts, token_to_contract)
}

fn aggregate_token_balances(tokens: &[EnrichedTokenEntry]) -> HashMap<String, u64> {
    let mut balances: HashMap<String, u64> = HashMap::new();
    for entry in tokens {
        let contract_id = entry.taproot_pubkey_gen_str.clone();
        let amount = entry.entry.value().unwrap_or(0);
        *balances.entry(contract_id).or_default() += amount;
    }
    balances
}

fn map_offers_to_contracts(
    tokens_in_offers: &HashMap<AssetId, u64>,
    token_to_contract: &HashMap<AssetId, String>,
) -> HashMap<String, u64> {
    let mut locked_in_offers_by_contract: HashMap<String, u64> = HashMap::new();
    for (token_asset_id, amount) in tokens_in_offers {
        if let Some(contract_id) = token_to_contract.get(token_asset_id) {
            *locked_in_offers_by_contract.entry(contract_id.clone()).or_default() += amount;
        }
    }
    locked_in_offers_by_contract
}

async fn query_collateral_for_candidates(
    wallet: &crate::wallet::Wallet,
    candidate_ids: &HashSet<String>,
    parsed_contracts: &HashMap<String, ParsedContract>,
) -> HashMap<String, u64> {
    let (query_ids, query_filters): (Vec<_>, Vec<_>) = candidate_ids
        .iter()
        .filter_map(|contract_id| {
            let parsed = parsed_contracts.get(contract_id)?;
            let collateral_asset_id = parsed.args.get_collateral_asset_id();
            let filter = UtxoFilter::new()
                .taproot_pubkey_gen(parsed.tpg.clone())
                .asset_id(collateral_asset_id);
            Some((contract_id.clone(), filter))
        })
        .unzip();

    let mut total_collateral_by_contract: HashMap<String, u64> = HashMap::new();

    if !query_filters.is_empty()
        && let Ok(results) = <_ as UtxoStore>::query_utxos(wallet.store(), &query_filters).await
    {
        for (contract_id, result) in query_ids.into_iter().zip(results.into_iter()) {
            let entries = match result {
                UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _) => entries,
                UtxoQueryResult::Empty => Vec::new(),
            };
            let total: u64 = entries.iter().filter_map(UtxoEntry::value).sum();
            total_collateral_by_contract.insert(contract_id, total);
        }
    }

    total_collateral_by_contract
}

fn build_and_filter_contract_states(
    candidate_ids: HashSet<String>,
    mut parsed_contracts: HashMap<String, ParsedContract>,
    option_balances: &HashMap<String, u64>,
    grantor_balances: &HashMap<String, u64>,
    locked_in_offers_by_contract: &HashMap<String, u64>,
    total_collateral_by_contract: &HashMap<String, u64>,
    now: i64,
) -> Vec<ContractState> {
    candidate_ids
        .into_iter()
        .filter_map(|contract_id| {
            let parsed = parsed_contracts.remove(&contract_id)?;
            let state = ContractState {
                args: parsed.args,
                address: parsed.tpg.address,
                user_options: option_balances.get(&contract_id).copied().unwrap_or(0),
                user_grantors: grantor_balances.get(&contract_id).copied().unwrap_or(0),
                locked_in_offers: locked_in_offers_by_contract.get(&contract_id).copied().unwrap_or(0),
                total_collateral: total_collateral_by_contract.get(&contract_id).copied().unwrap_or(0),
            };
            Some(state)
        })
        .filter(|state| state.is_valid(now) && state.is_active())
        .collect()
}

async fn fetch_active_contract_states(
    wallet: &crate::wallet::Wallet,
    option_tokens: &[EnrichedTokenEntry],
    grantor_tokens: &[EnrichedTokenEntry],
    network: simplicityhl_core::SimplicityNetwork,
    now: i64,
) -> Vec<ContractState> {
    let Ok(option_contracts) =
        <_ as UtxoStore>::list_contracts_by_source_with_metadata(wallet.store(), OPTION_SOURCE).await
    else {
        return Vec::new();
    };

    let (parsed_contracts, token_to_contract) = parse_option_contracts(&option_contracts, network);
    let contract_token_ids: HashSet<AssetId> = token_to_contract.keys().copied().collect();

    let option_balances = aggregate_token_balances(option_tokens);
    let grantor_balances = aggregate_token_balances(grantor_tokens);
    let tokens_in_offers = query_contract_tokens_in_offers(wallet, &contract_token_ids, network).await;

    let locked_in_offers_by_contract = map_offers_to_contracts(&tokens_in_offers, &token_to_contract);

    let candidate_ids: HashSet<String> = option_balances
        .keys()
        .chain(grantor_balances.keys())
        .chain(locked_in_offers_by_contract.keys())
        .cloned()
        .collect();

    let total_collateral_by_contract = query_collateral_for_candidates(wallet, &candidate_ids, &parsed_contracts).await;

    build_and_filter_contract_states(
        candidate_ids,
        parsed_contracts,
        &option_balances,
        &grantor_balances,
        &locked_in_offers_by_contract,
        &total_collateral_by_contract,
        now,
    )
}

async fn build_active_options_displays(
    wallet: &crate::wallet::Wallet,
    option_tokens: &[EnrichedTokenEntry],
    grantor_tokens: &[EnrichedTokenEntry],
    network: simplicityhl_core::SimplicityNetwork,
) -> Vec<ActiveOptionsDisplay> {
    let now = current_timestamp();
    let mut contract_states = fetch_active_contract_states(wallet, option_tokens, grantor_tokens, network, now).await;

    contract_states.sort_by_key(ContractState::expiry_time);

    contract_states
        .into_iter()
        .enumerate()
        .map(|(idx, state)| {
            let contract_addr = truncate_with_ellipsis(&state.address.to_string(), 12);
            ActiveOptionsDisplay {
                index: idx + 1,
                option_tokens: state.user_options,
                grantor_tokens: state.user_grantors,
                expires: format_relative_time(state.expiry_time()),
                contract_id: contract_addr,
            }
        })
        .collect()
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
