use crate::error::Error;

use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use coin_store::{UtxoEntry, UtxoFilter, UtxoQueryResult, UtxoStore};

use contracts::options::OptionsArguments;

use simplicityhl::elements::Script;
use simplicityhl::elements::hex::ToHex;
use simplicityhl_core::LIQUID_TESTNET_BITCOIN_ASSET;

pub const OPTION_TOKEN_TAG: &str = "option_token";
pub const GRANTOR_TOKEN_TAG: &str = "grantor_token";
pub const SWAP_COLLATERAL_TAG: &str = "swap_collateral";

#[derive(Debug, Clone)]
pub struct TokenDisplay {
    pub index: usize,
    pub collateral: String,
    pub settlement: String,
    pub expires: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct SwapDisplay {
    pub index: usize,
    pub offering: String,
    pub wants: String,
    pub expires: String,
    pub seller: String,
}

/// Format a past timestamp as "X ago" for history entries.
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub fn format_time_ago(timestamp: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let diff_secs = now - timestamp;

    if diff_secs <= 0 {
        return "just now".to_string();
    }

    let hours = diff_secs / 3600;
    let days = hours / 24;
    let remaining_hours = hours % 24;

    if days > 0 {
        if remaining_hours > 0 {
            format!("{days} days {remaining_hours} hours ago")
        } else {
            format!("{days} days ago")
        }
    } else if hours > 0 {
        format!("{hours} hours ago")
    } else {
        let minutes = diff_secs / 60;
        if minutes > 0 {
            format!("{minutes} minutes ago")
        } else {
            format!("{diff_secs} seconds ago")
        }
    }
}

/// Format a future expiry timestamp as "in X days" or "[EXPIRED]".
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub fn format_relative_time(expiry_timestamp: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let diff_secs = expiry_timestamp - now;

    if diff_secs <= 0 {
        return "[EXPIRED]".to_string();
    }

    let hours = diff_secs / 3600;
    let days = hours / 24;
    let remaining_hours = hours % 24;

    let time_str = if days > 0 {
        if remaining_hours > 0 {
            format!("in {days} days {remaining_hours} hours")
        } else {
            format!("in {days} days")
        }
    } else if hours > 0 {
        format!("in {hours} hours")
    } else {
        let minutes = diff_secs / 60;
        if minutes > 0 {
            format!("in {minutes} minutes")
        } else {
            format!("in {diff_secs} seconds")
        }
    };

    // Add urgency prefix
    if hours < 1 {
        format!("[URGENT] {time_str}")
    } else if hours < 24 {
        format!("[SOON] {time_str}")
    } else {
        time_str
    }
}

pub fn display_token_table(tokens: &[TokenDisplay]) {
    if tokens.is_empty() {
        println!("  (No tokens found)");
        return;
    }

    println!(
        "  {:<3} | {:<18} | {:<14} | {:<18} | Contract",
        "#", "Collateral/Token", "Strike/Token", "Expires"
    );
    println!("{}", "-".repeat(80));

    for token in tokens {
        println!(
            "  {:<3} | {:<18} | {:<14} | {:<18} | {}",
            token.index, token.collateral, token.settlement, token.expires, token.status
        );
    }
}

pub fn display_swap_table(swaps: &[SwapDisplay]) {
    if swaps.is_empty() {
        println!("  (No swaps found)");
        return;
    }

    println!(
        "  {:<3} | {:<20} | {:<14} | {:<15} | Seller",
        "#", "Price", "Wants", "Expires"
    );
    println!("{}", "-".repeat(80));

    for swap in swaps {
        println!(
            "  {:<3} | {:<20} | {:<14} | {:<15} | {}",
            swap.index, swap.offering, swap.wants, swap.expires, swap.seller
        );
    }
}

pub fn prompt_selection(prompt: &str, max: usize) -> io::Result<Option<usize>> {
    print!("{prompt} (1-{max}, or 'q' to quit): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.eq_ignore_ascii_case("q") {
        return Ok(None);
    }

    match input.parse::<usize>() {
        Ok(n) if n >= 1 && n <= max => Ok(Some(n - 1)), // Convert to 0-based
        _ => {
            println!("Invalid selection. Please enter a number between 1 and {max}.");
            prompt_selection(prompt, max) // Retry
        }
    }
}

pub fn prompt_amount(prompt: &str) -> io::Result<u64> {
    print!("{prompt}: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    input.parse::<u64>().map_or_else(
        |_| {
            println!("Invalid amount. Please enter a positive number.");
            prompt_amount(prompt) // Retry
        },
        Ok,
    )
}

#[must_use]
pub fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        s[..max_len].to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[allow(clippy::cast_possible_wrap)]
pub fn parse_expiry(expiry: &str) -> Result<i64, Error> {
    if let Ok(ts) = expiry.parse::<i64>() {
        return Ok(ts);
    }

    // Try parsing as relative duration (+30d, +2h, +1w, etc.)
    if let Some(duration_str) = expiry.strip_prefix('+') {
        let now = current_timestamp();
        let std_duration: std::time::Duration = duration_str
            .parse::<humantime::Duration>()
            .map_err(|err| Error::HumantimeParse {
                str: duration_str.to_string(),
                err,
            })?
            .into();

        return Ok(now + std_duration.as_secs() as i64);
    }

    Err(Error::Config(format!(
        "Invalid expiry format '{expiry}'. Use Unix timestamp or relative duration (+30d, +2h, +1w)"
    )))
}

#[allow(clippy::cast_possible_wrap)]
pub fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn extract_entries_from_result(result: &UtxoQueryResult) -> Vec<&UtxoEntry> {
    match result {
        UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _) => entries.iter().collect(),
        UtxoQueryResult::Empty => Vec::new(),
    }
}

pub fn extract_entries_from_results(results: Vec<UtxoQueryResult>) -> Vec<UtxoEntry> {
    results
        .into_iter()
        .flat_map(|r| match r {
            UtxoQueryResult::Found(entries, _) | UtxoQueryResult::InsufficientValue(entries, _) => entries,
            UtxoQueryResult::Empty => vec![],
        })
        .collect()
}

/// Get grantor tokens from user's wallet using the `contract_tokens` table.
///
/// This function queries UTXOs with the "`grantor_token`" tag, which automatically
/// joins with the `contract_tokens` and `simplicity_contracts` tables to provide
/// full contract context.
pub async fn get_grantor_tokens_from_wallet(
    wallet: &crate::wallet::Wallet,
    _source: &str,
    user_script_pubkey: &Script,
) -> Result<Vec<EnrichedTokenEntry>, Error> {
    let filter = UtxoFilter::new()
        .token_tag(GRANTOR_TOKEN_TAG)
        .script_pubkey(user_script_pubkey.clone());

    let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[filter]).await?;
    let entries = extract_entries_from_results(results);

    let mut enriched: Vec<EnrichedTokenEntry> = entries
        .into_iter()
        .filter_map(|entry| {
            let arguments = entry.arguments()?;
            let option_arguments = OptionsArguments::from_arguments(arguments).ok()?;
            let taproot_pubkey_gen_str = entry.taproot_pubkey_gen()?.to_string();

            Some(EnrichedTokenEntry {
                entry,
                option_arguments,
                taproot_pubkey_gen_str,
            })
        })
        .collect();

    enriched.sort_by(|a, b| b.entry.value().unwrap_or(0).cmp(&a.entry.value().unwrap_or(0)));

    Ok(enriched)
}

#[derive(Debug)]
pub struct EnrichedTokenEntry {
    pub entry: UtxoEntry,
    pub option_arguments: OptionsArguments,
    pub taproot_pubkey_gen_str: String,
}

/// Get option tokens from user's wallet using the `contract_tokens` table.
///
/// This function queries UTXOs with the "`option_token`" tag, which automatically
/// joins with the `contract_tokens` and `simplicity_contracts` tables to provide
/// full contract context.
pub async fn get_option_tokens_from_wallet(
    wallet: &crate::wallet::Wallet,
    _source: &str,
    user_script_pubkey: &Script,
) -> Result<Vec<EnrichedTokenEntry>, Error> {
    let filter = UtxoFilter::new()
        .token_tag(OPTION_TOKEN_TAG)
        .script_pubkey(user_script_pubkey.clone());

    let results = <_ as UtxoStore>::query_utxos(wallet.store(), &[filter]).await?;
    let entries = extract_entries_from_results(results);

    let mut enriched: Vec<EnrichedTokenEntry> = entries
        .into_iter()
        .filter_map(|entry| {
            let arguments = entry.arguments()?;
            let option_arguments = OptionsArguments::from_arguments(arguments).ok()?;
            let taproot_pubkey_gen_str = entry.taproot_pubkey_gen()?.to_string();

            Some(EnrichedTokenEntry {
                entry,
                option_arguments,
                taproot_pubkey_gen_str,
            })
        })
        .collect();

    enriched.sort_by(|a, b| b.entry.value().unwrap_or(0).cmp(&a.entry.value().unwrap_or(0)));

    Ok(enriched)
}

/// Select from enriched token entries that include contract arguments.
/// This shows settlement and expiry information from the contract.
pub fn select_enriched_token_interactive<'a>(
    entries: &'a [EnrichedTokenEntry],
    prompt: &str,
) -> Result<&'a EnrichedTokenEntry, Error> {
    let displays: Vec<TokenDisplay> = entries
        .iter()
        .enumerate()
        .map(|(idx, enriched)| {
            let settlement_asset = enriched.option_arguments.get_settlement_asset_id();
            let settlement_per_contract = enriched.option_arguments.settlement_per_contract();
            let expiry_time = enriched.option_arguments.expiry_time();

            let contract_addr = enriched
                .taproot_pubkey_gen_str
                .split(':')
                .next_back()
                .map_or_else(|| "???".to_string(), |s| truncate_with_ellipsis(s, 12));

            TokenDisplay {
                index: idx + 1,
                collateral: format!("{} tokens", enriched.entry.value().unwrap_or(0)),
                settlement: format!(
                    "{} {}",
                    settlement_per_contract,
                    format_settlement_asset(&settlement_asset)
                ),
                expires: format_relative_time(i64::from(expiry_time)),
                status: contract_addr,
            }
        })
        .collect();

    if displays.is_empty() {
        return Err(Error::Config("No valid tokens found".to_string()));
    }

    display_token_table(&displays);
    println!();

    let selection = prompt_selection(prompt, displays.len())
        .map_err(Error::Io)?
        .ok_or_else(|| Error::Config("Selection cancelled".to_string()))?;

    Ok(&entries[selection])
}

pub fn format_settlement_asset(asset_id: &simplicityhl::elements::AssetId) -> String {
    if *asset_id == *LIQUID_TESTNET_BITCOIN_ASSET {
        "LBTC".to_string()
    } else {
        let hex = asset_id.to_hex();
        format!("({})...", &hex[..hex.len().min(8)])
    }
}

/// Look up a human-readable tag for an asset from the `contract_tokens` table.
///
/// Returns `Some(tag)` if the asset is registered (e.g., "`option_token`", "`grantor_token`"),
/// or `None` if not found.
pub async fn lookup_asset_tag(store: &coin_store::Store, asset_id: &simplicityhl::elements::AssetId) -> Option<String> {
    <_ as UtxoStore>::get_contract_by_token(store, *asset_id)
        .await
        .ok()
        .flatten()
        .map(|(_, tag)| tag)
}

/// Format an asset ID with tag lookup, falling back to hex if no tag found.
///
/// Returns "LBTC" for native asset, the tag if registered, or truncated hex otherwise.
pub async fn format_asset_with_tag(store: &coin_store::Store, asset_id: &simplicityhl::elements::AssetId) -> String {
    if *asset_id == *LIQUID_TESTNET_BITCOIN_ASSET {
        return "LBTC".to_string();
    }

    if let Some(tag) = lookup_asset_tag(store, asset_id).await {
        return tag;
    }

    let hex = asset_id.to_hex();
    format!("({})...", &hex[..hex.len().min(8)])
}

/// Format an asset value with tag lookup, showing "value tag" or "value (hex)...".
pub async fn format_asset_value_with_tag(
    store: &coin_store::Store,
    value: Option<u64>,
    asset_id: Option<simplicityhl::elements::AssetId>,
) -> String {
    match (value, asset_id) {
        (Some(v), Some(a)) => {
            let asset_str = format_asset_with_tag(store, &a).await;
            format!("{v} {asset_str}")
        }
        (Some(v), None) => format!("{v} (unknown)"),
        _ => "Confidential".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCEPTABLE_THRESHOLD: i64 = 2;

    #[test]
    #[allow(clippy::cast_possible_wrap)]
    fn test_format_relative_time() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

        assert_eq!(format_relative_time(now - 100), "[EXPIRED]");

        let urgent = format_relative_time(now + 1800);
        assert!(urgent.starts_with("[URGENT]"));

        let soon = format_relative_time(now + 6 * 3600);
        assert!(soon.starts_with("[SOON]"));

        let normal = format_relative_time(now + 3 * 24 * 3600);
        assert!(normal.starts_with("in 3 days"));
    }

    #[test]
    fn test_truncate_with_ellipsis() {
        assert_eq!(truncate_with_ellipsis("hello", 10), "hello");
        assert_eq!(truncate_with_ellipsis("hello world", 8), "hello...");
        assert_eq!(truncate_with_ellipsis("abc", 3), "abc");
    }

    #[test]
    fn test_parse_expiry_unix_timestamp() {
        let ts = 1_704_067_200_i64;
        assert_eq!(parse_expiry("1704067200").unwrap(), ts);
    }

    #[test]
    fn test_parse_expiry_zero_timestamp() {
        assert_eq!(parse_expiry("0").unwrap(), 0);
    }

    #[test]
    fn test_parse_expiry_relative_days() {
        let now = current_timestamp();
        let result = parse_expiry("+30d").unwrap();
        let expected = now + 30 * 24 * 3600;
        assert!((result - expected).abs() < ACCEPTABLE_THRESHOLD);
    }

    #[test]
    fn test_parse_expiry_relative_hours() {
        let now = current_timestamp();
        let result = parse_expiry("+2h").unwrap();
        let expected = now + 2 * 3600;
        assert!((result - expected).abs() < ACCEPTABLE_THRESHOLD);
    }

    #[test]
    fn test_parse_expiry_relative_weeks() {
        let now = current_timestamp();
        let result = parse_expiry("+1w").unwrap();
        let expected = now + 7 * 24 * 3600;
        assert!((result - expected).abs() < ACCEPTABLE_THRESHOLD);
    }

    #[test]
    fn test_parse_expiry_relative_minutes() {
        let now = current_timestamp();
        let result = parse_expiry("+45min").unwrap();
        let expected = now + 45 * 60;
        assert!((result - expected).abs() < ACCEPTABLE_THRESHOLD);
    }

    #[test]
    fn test_parse_expiry_combined_duration() {
        let now = current_timestamp();
        let result = parse_expiry("+1d2h").unwrap();
        let expected = now + 24 * 3600 + 2 * 3600;
        assert!((result - expected).abs() < ACCEPTABLE_THRESHOLD);
    }

    #[test]
    fn test_parse_expiry_invalid_format() {
        let result = parse_expiry("invalid");
        assert!(result.is_err());
        match result {
            Err(Error::Config(msg)) => {
                assert!(msg.contains("Invalid expiry format"));
            }
            _ => panic!("Expected Config error"),
        }
    }

    #[test]
    fn test_parse_expiry_invalid_relative_duration() {
        let result = parse_expiry("+invalid_duration");
        assert!(result.is_err());
        match result {
            Err(Error::HumantimeParse { .. }) => {}
            _ => panic!("Expected HumantimeParse error"),
        }
    }

    #[test]
    fn test_parse_expiry_negative_relative_duration() {
        let result = parse_expiry("-30d");
        assert!(result.is_err());
    }
}
