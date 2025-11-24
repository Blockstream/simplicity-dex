// TODO(Illia): refactor this.

use elements::hex::ToHex;
use elements::secp256k1_zkp::PublicKey;
use hex::FromHex;
use simplicity::bitcoin::secp256k1;
use simplicity::bitcoin::secp256k1::SecretKey;
use simplicityhl::elements::AssetId;
use simplicityhl_core::broadcast_tx;
use std::fmt::Debug;
use std::{io::Write, path::PathBuf};

const DEFAULT_RELAYS_FILEPATH: &str = ".simplicity-dex/relays.txt";
const DEFAULT_KEY_PATH: &str = ".simplicity-dex/keypair.txt";
pub const DEFAULT_CLIENT_TIMEOUT_SECS: u64 = 10;

pub(crate) fn write_into_stdout<T: AsRef<str> + std::fmt::Debug>(text: T) -> std::io::Result<usize> {
    let mut output = text.as_ref().to_string();
    output.push('\n');
    std::io::stdout().write(output.as_bytes())
}

#[must_use]
pub fn default_key_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DEFAULT_KEY_PATH)
}

#[must_use]
pub fn default_relays_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DEFAULT_RELAYS_FILEPATH)
}

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("Unable to parse url: {1}, error: {0}")]
    UrlParseError(nostr::types::url::Error, String),
    #[error("Got error on reading/writing to file: {1}, error: {0}")]
    ProblemWithFile(std::io::Error, PathBuf),
    #[error("Incorrect path to the file, please check validity of the path (err: path is not a file), got path: {0}")]
    IncorrectPathToFile(PathBuf),
    #[error("File is empty, got path: {0}")]
    EmptyFile(PathBuf),
    #[error("File is empty, got path: {0}")]
    KeyParseError(nostr::key::Error, String),
}

pub(crate) fn broadcast_tx_inner(tx: &simplicityhl::elements::Transaction) -> crate::error::Result<String> {
    broadcast_tx(tx).map_err(|err| crate::error::CliError::Broadcast(err.to_string()))
}

pub(crate) fn decode_hex(str: impl AsRef<[u8]>) -> crate::error::Result<Vec<u8>> {
    let str_to_convert = str.as_ref();
    hex::decode(str_to_convert).map_err(|err| crate::error::CliError::FromHex(err, str_to_convert.to_hex()))
}

pub const PUBLIC_SECRET_KEY: [u8; 32] = [2; 32];

#[inline]
pub(crate) fn derive_public_oracle_keypair() -> crate::error::Result<secp256k1::Keypair> {
    let blinder_key =
        secp256k1::Keypair::from_secret_key(secp256k1::SECP256K1, &SecretKey::from_slice(&PUBLIC_SECRET_KEY)?);
    Ok(blinder_key)
}

#[inline]
pub(crate) fn derive_oracle_pubkey() -> crate::error::Result<PublicKey> {
    Ok(derive_public_oracle_keypair()?.public_key())
}

pub(crate) fn entropy_to_asset_id(el: impl AsRef<[u8]>) -> crate::error::Result<AssetId> {
    use simplicity::hashes::sha256;
    let el = el.as_ref();
    let mut asset_entropy_bytes =
        <[u8; 32]>::from_hex(el).map_err(|err| crate::error::CliError::FromHex(err, el.to_hex()))?;
    asset_entropy_bytes.reverse();
    let midstate = sha256::Midstate::from_byte_array(asset_entropy_bytes);
    Ok(AssetId::from_entropy(midstate))
}
