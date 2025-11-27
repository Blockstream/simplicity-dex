use crate::common::config::AggregatedConfig;
use crate::error::CliError;
use simplicityhl::elements::secp256k1_zkp as secp256k1;

/// # Errors
///
/// Returns `CliError::EcCurve` if the derived 32 byte array is not a valid secp256k1 secret key
pub fn derive_secret_key_from_index(index: u32, config: &AggregatedConfig) -> Result<secp256k1::SecretKey, CliError> {
    let mut seed = config.seed_hex.0;

    for (i, b) in index.to_be_bytes().iter().enumerate() {
        seed[24 + i] ^= *b;
    }
    secp256k1::SecretKey::from_slice(&seed).map_err(CliError::from)
}
