use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::elements::{Transaction, TxOut};

/// Witness scale factor for weight-to-vsize conversion.
/// In segwit, weight = 4 * `base_size` + `witness_size`, so vsize = weight / 4.
pub const WITNESS_SCALE_FACTOR: usize = 4;

/// Placeholder fee for first-pass weight measurement (1 satoshi).
/// Used when building a transaction to measure its actual weight before
/// calculating the real fee.
pub const PLACEHOLDER_FEE: u64 = 1;

/// Default fallback fee rate in sats/kvb (0.10 sat/vB).
/// Higher than LWK default to meet Liquid minimum relay fee requirements.
#[allow(dead_code)]
pub const DEFAULT_FEE_RATE: f32 = 100.0;

/// Estimate fee by signing a placeholder transaction to get accurate weight.
///
/// This function handles the pattern of:
/// 1. If fee is provided, use it directly
/// 2. Otherwise, build a temporary transaction with placeholder fee,
///    sign it to get the actual weight including witness data,
///    then calculate the fee from the signed weight
///
/// This accounts for witness data (signatures) that significantly increase
/// transaction weight, providing accurate fee estimation.
///
/// # Arguments
///
/// * `fee` - Optional user-provided fee in satoshis
/// * `fee_rate` - Fee rate in satoshis per 1000 virtual bytes (sats/kvb)
/// * `builder` - Closure that builds a PST and returns it with the UTXOs needed for signing
/// * `signer` - Closure that signs the transaction given the tx and UTXOs
///
/// # Returns
///
/// The fee to use (either provided or estimated from signed weight).
///
/// # Errors
///
/// Returns an error if the builder, signer, or transaction extraction fails.
pub fn estimate_fee_signed<B, S, E>(fee: Option<&u64>, fee_rate: f32, builder: B, signer: S) -> Result<u64, E>
where
    B: FnOnce(u64) -> Result<(PartiallySignedTransaction, Vec<TxOut>), E>,
    S: FnOnce(Transaction, &[TxOut]) -> Result<Transaction, E>,
    E: From<simplicityhl::elements::pset::Error>,
{
    if let Some(f) = fee {
        return Ok(*f);
    }

    let (pst, utxos) = builder(PLACEHOLDER_FEE)?;
    let tx = pst.extract_tx()?;
    let signed_tx = signer(tx, &utxos)?;
    let signed_weight = signed_tx.weight();
    let estimated = calculate_fee(signed_weight, fee_rate);
    println!("Estimated fee: {estimated} sats (signed weight: {signed_weight}, rate: {fee_rate} sats/kvb)");
    Ok(estimated)
}

/// Calculate fee from weight and fee rate (sats/kvb).
///
/// Formula: `fee = ceil(vsize * fee_rate / 1000)`
/// where `vsize = ceil(weight / 4)`
///
/// # Arguments
///
/// * `weight` - Transaction weight in weight units (WU)
/// * `fee_rate` - Fee rate in satoshis per 1000 virtual bytes (sats/kvb)
///
/// # Returns
///
/// The calculated fee in satoshis.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn calculate_fee(weight: usize, fee_rate: f32) -> u64 {
    let vsize = weight.div_ceil(WITNESS_SCALE_FACTOR);
    (vsize as f32 * fee_rate / 1000.0).ceil() as u64
}
