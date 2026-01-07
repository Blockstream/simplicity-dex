use std::collections::HashMap;
use std::str::FromStr;

use serde::Deserialize;
use simplicityhl::elements::encode;
use simplicityhl::elements::hashes::{Hash, sha256};
use simplicityhl::elements::hex::ToHex;
use simplicityhl::elements::{Address, OutPoint, Script, Transaction, Txid};

#[allow(unused_imports)]
pub use cli_helper::explorer::{ExplorerError, broadcast_tx, fetch_utxo};

const ESPLORA_URL: &str = "https://blockstream.info/liquidtestnet/api";

/// Fee estimates response from Esplora.
/// Key: confirmation target (in blocks as string), Value: fee rate (sat/vB).
pub type FeeEstimates = HashMap<String, f64>;

/// Error type for Esplora sync operations.
#[derive(thiserror::Error, Debug)]
pub enum EsploraError {
    #[error("HTTP request failed: {0}")]
    Request(String),

    #[error("Failed to deserialize response: {0}")]
    Deserialize(String),

    #[error("Invalid txid format: {0}")]
    InvalidTxid(String),
}

pub type FetchTransactionError = EsploraError;

/// Spending status of a transaction output.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OutspendStatus {
    pub spent: bool,
    #[serde(default)]
    pub txid: Option<String>,
    #[serde(default)]
    pub vin: Option<u32>,
}

/// UTXO status from Esplora.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct UtxoStatus {
    pub confirmed: bool,
    #[serde(default)]
    pub block_height: Option<u64>,
    #[serde(default)]
    pub block_hash: Option<String>,
}

/// UTXO entry from Esplora address/scripthash endpoint.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct EsploraUtxo {
    pub txid: String,
    pub vout: u32,
    #[serde(default)]
    pub value: Option<u64>,
    #[serde(default)]
    pub valuecommitment: Option<String>,
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default)]
    pub assetcommitment: Option<String>,
    pub status: UtxoStatus,
}

/// Fetch full transaction from Esplora by txid.
///
/// Uses the `GET /tx/:txid/raw` endpoint which returns the raw transaction
/// as binary data.
///
/// # Errors
///
/// Returns an error if the HTTP request fails or if the response cannot
/// be deserialized into a valid transaction.
pub fn fetch_transaction(txid: Txid) -> Result<Transaction, EsploraError> {
    let url = format!("{ESPLORA_URL}/tx/{}/raw", txid.to_hex());
    let response = minreq::get(&url)
        .send()
        .map_err(|e| EsploraError::Request(e.to_string()))?;

    if response.status_code != 200 {
        return Err(EsploraError::Request(format!(
            "HTTP {}: {}",
            response.status_code, response.reason_phrase
        )));
    }

    let bytes = response.as_bytes();
    let tx: Transaction = encode::deserialize(bytes).map_err(|e| EsploraError::Deserialize(e.to_string()))?;

    Ok(tx)
}

/// Check if a specific output has been spent.
///
/// Uses the `GET /tx/:txid/outspend/:vout` endpoint.
#[allow(dead_code)]
pub fn fetch_outspend(txid: Txid, vout: u32) -> Result<OutspendStatus, EsploraError> {
    let url = format!("{ESPLORA_URL}/tx/{}/outspend/{vout}", txid.to_hex());
    let response = minreq::get(&url)
        .send()
        .map_err(|e| EsploraError::Request(e.to_string()))?;

    if response.status_code != 200 {
        return Err(EsploraError::Request(format!(
            "HTTP {}: {}",
            response.status_code, response.reason_phrase
        )));
    }

    let status: OutspendStatus = response.json().map_err(|e| EsploraError::Deserialize(e.to_string()))?;

    Ok(status)
}

/// Check spending status of all outputs in a transaction.
///
/// Uses the `GET /tx/:txid/outspends` endpoint. More efficient than
/// calling `fetch_outspend` for each output individually.
pub fn fetch_outspends(txid: Txid) -> Result<Vec<OutspendStatus>, EsploraError> {
    let url = format!("{ESPLORA_URL}/tx/{}/outspends", txid.to_hex());
    let response = minreq::get(&url)
        .send()
        .map_err(|e| EsploraError::Request(e.to_string()))?;

    if response.status_code != 200 {
        return Err(EsploraError::Request(format!(
            "HTTP {}: {}",
            response.status_code, response.reason_phrase
        )));
    }

    let statuses: Vec<OutspendStatus> = response.json().map_err(|e| EsploraError::Deserialize(e.to_string()))?;

    Ok(statuses)
}

/// Fetch UTXOs for an address.
///
/// Uses the `GET /address/:address/utxo` endpoint.
pub fn fetch_address_utxos(address: &Address) -> Result<Vec<EsploraUtxo>, EsploraError> {
    let url = format!("{ESPLORA_URL}/address/{address}/utxo");
    let response = minreq::get(&url)
        .send()
        .map_err(|e| EsploraError::Request(e.to_string()))?;

    if response.status_code != 200 {
        return Err(EsploraError::Request(format!(
            "HTTP {}: {}",
            response.status_code, response.reason_phrase
        )));
    }

    let utxos: Vec<EsploraUtxo> = response.json().map_err(|e| EsploraError::Deserialize(e.to_string()))?;

    Ok(utxos)
}

/// Fetch UTXOs by scripthash.
///
/// Uses the `GET /scripthash/:hash/utxo` endpoint.
/// The scripthash is SHA256 of the scriptPubKey (reversed for display).
pub fn fetch_scripthash_utxos(script: &Script) -> Result<Vec<EsploraUtxo>, EsploraError> {
    let hash = sha256::Hash::hash(script.as_bytes());
    let hash_bytes = hash.to_byte_array();
    let scripthash = hex::encode(hash_bytes);

    let url = format!("{ESPLORA_URL}/scripthash/{scripthash}/utxo");
    let response = minreq::get(&url)
        .send()
        .map_err(|e| EsploraError::Request(e.to_string()))?;

    if response.status_code != 200 {
        return Err(EsploraError::Request(format!(
            "HTTP {}: {}",
            response.status_code, response.reason_phrase
        )));
    }

    let utxos: Vec<EsploraUtxo> = response.json().map_err(|e| EsploraError::Deserialize(e.to_string()))?;

    Ok(utxos)
}

/// Fetch current blockchain tip height.
///
/// Uses the `GET /blocks/tip/height` endpoint.
pub fn fetch_tip_height() -> Result<u64, EsploraError> {
    let url = format!("{ESPLORA_URL}/blocks/tip/height");
    let response = minreq::get(&url)
        .send()
        .map_err(|e| EsploraError::Request(e.to_string()))?;

    if response.status_code != 200 {
        return Err(EsploraError::Request(format!(
            "HTTP {}: {}",
            response.status_code, response.reason_phrase
        )));
    }

    let height_str = response
        .as_str()
        .map_err(|e| EsploraError::Deserialize(e.to_string()))?;
    let height: u64 = height_str
        .trim()
        .parse()
        .map_err(|e: std::num::ParseIntError| EsploraError::Deserialize(e.to_string()))?;

    Ok(height)
}

/// Parse a txid string into a Txid.
pub fn parse_txid(txid_str: &str) -> Result<Txid, EsploraError> {
    Txid::from_str(txid_str).map_err(|e| EsploraError::InvalidTxid(e.to_string()))
}

/// Convert an `EsploraUtxo` to an `OutPoint`.
pub fn esplora_utxo_to_outpoint(utxo: &EsploraUtxo) -> Result<OutPoint, EsploraError> {
    let txid = parse_txid(&utxo.txid)?;
    Ok(OutPoint::new(txid, utxo.vout))
}

/// Fetch fee estimates for various confirmation targets.
///
/// Uses the `GET /fee-estimates` endpoint.
/// Note: Liquid testnet typically returns empty results, so callers should
/// use a fallback rate (see `config.fee.fallback_rate`).
///
/// Returns a map where key is confirmation target (blocks) and value is fee rate (sat/vB).
///
/// Example response: `{ "1": 87.882, "2": 87.882, ..., "144": 1.027, "1008": 1.027 }`
pub fn fetch_fee_estimates() -> Result<FeeEstimates, EsploraError> {
    let url = format!("{ESPLORA_URL}/fee-estimates");
    let response = minreq::get(&url)
        .send()
        .map_err(|e| EsploraError::Request(e.to_string()))?;

    if response.status_code != 200 {
        return Err(EsploraError::Request(format!(
            "HTTP {}: {}",
            response.status_code, response.reason_phrase
        )));
    }

    let estimates: FeeEstimates = response.json().map_err(|e| EsploraError::Deserialize(e.to_string()))?;

    Ok(estimates)
}

/// Get fee rate for a specific confirmation target.
///
/// Fetches fee estimates from Esplora and returns the rate for the given target.
/// If the exact target is not available, falls back to higher targets.
///
/// # Arguments
///
/// * `target_blocks` - Desired confirmation target in blocks (1-25, 144, 504, 1008)
///
/// # Returns
///
/// Fee rate in sats/kvb (satoshis per 1000 virtual bytes).
/// Multiply Esplora's sat/vB value by 1000.
///
/// # Errors
///
/// Returns an error if the HTTP request fails or no suitable fee rate is found.
#[allow(clippy::cast_possible_truncation)]
pub fn get_fee_rate(target_blocks: u32) -> Result<f32, EsploraError> {
    let estimates = fetch_fee_estimates()?;

    let target_str = target_blocks.to_string();
    if let Some(&rate) = estimates.get(&target_str) {
        return Ok((rate * 1000.0) as f32); // Convert sat/vB to sats/kvb
    }

    // Fall back to higher targets (lower fee rates)
    // Available targets: 1-25, 144, 504, 1008
    let fallback_targets = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 144, 504, 1008,
    ];

    for &target in fallback_targets.iter().filter(|&&t| t >= target_blocks) {
        let key = target.to_string();
        if let Some(&rate) = estimates.get(&key) {
            return Ok((rate * 1000.0) as f32);
        }
    }

    // If no higher target found, try any available rate (use lowest target = highest rate)
    for &target in &fallback_targets {
        let key = target.to_string();
        if let Some(&rate) = estimates.get(&key) {
            return Ok((rate * 1000.0) as f32);
        }
    }

    Err(EsploraError::Request("No fee estimates available".to_string()))
}
