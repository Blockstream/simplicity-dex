use crate::error::CliError;
use simplicityhl::elements::secp256k1_zkp as secp256k1;

/// Derives a secp256k1 secret key from a 32-byte seed and index.
///
/// # Errors
///
/// Returns `CliError` if the resulting 32-byte buffer is not a valid
/// secp256k1 secret key and `secp256k1::SecretKey::from_slice` fails.
pub fn derive_secret_key_from_index(index: u32, seed_hex: impl AsRef<[u8]>) -> Result<secp256k1::SecretKey, CliError> {
    let mut seed = [0u8; 32];
    seed.copy_from_slice(seed_hex.as_ref());

    for (i, b) in index.to_be_bytes().iter().enumerate() {
        seed[24 + i] ^= *b;
    }
    secp256k1::SecretKey::from_slice(&seed).map_err(CliError::from)
}

/// Derives a secp256k1 keypair from a 32-byte seed and index.
///
/// # Errors
///
/// Returns `CliError` if the underlying secret key derivation fails
/// (for example, if the derived 32-byte value is not a valid
/// secp256k1 secret key).
#[inline]
pub fn derive_keypair_from_index(index: u32, seed_hex: impl AsRef<[u8]>) -> Result<secp256k1::Keypair, CliError> {
    Ok(elements::bitcoin::secp256k1::Keypair::from_secret_key(
        elements::bitcoin::secp256k1::SECP256K1,
        &derive_secret_key_from_index(index, seed_hex)?,
    ))
}
