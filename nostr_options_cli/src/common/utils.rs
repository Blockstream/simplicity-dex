use elements::bitcoin::XOnlyPublicKey;
use elements::hex::ToHex;
use elements::secp256k1_zkp::PublicKey;
use hex::FromHex;
use nostr::{Keys, RelayUrl};
use simplicity::bitcoin::secp256k1;
use simplicity::bitcoin::secp256k1::SecretKey;
use simplicityhl::elements::AssetId;
use simplicityhl_core::broadcast_tx;
use std::collections::HashSet;
use std::fmt::Debug;
use std::io::BufRead;
use std::str::FromStr;
use std::{io::Write, path::PathBuf};
use tracing::instrument;

const DEFAULT_RELAYS_FILEPATH: &str = ".simplicity-dex/relays.txt";
const DEFAULT_KEY_PATH: &str = ".simplicity-dex/keypair.txt";
pub const DEFAULT_CLIENT_TIMEOUT_SECS: u64 = 10;

pub fn write_into_stdout<T: AsRef<str> + std::fmt::Debug>(text: T) -> std::io::Result<usize> {
    let mut output = text.as_ref().to_string();
    output.push('\n');
    std::io::stdout().write(output.as_bytes())
}

pub fn default_key_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DEFAULT_KEY_PATH)
}

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

pub fn check_file_existence(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);

    if path.is_file() {
        Ok(path)
    } else {
        Err(FileError::IncorrectPathToFile(path.clone()).to_string())
    }
}

pub fn get_valid_urls_from_file(filepath: &PathBuf) -> Result<Vec<RelayUrl>, FileError> {
    let file = std::fs::File::open(filepath).map_err(|x| FileError::ProblemWithFile(x, filepath.clone()))?;
    let reader = std::io::BufReader::new(file);
    let mut set = HashSet::new();
    for x in reader.lines() {
        let line = x.map_err(|x| FileError::ProblemWithFile(x, filepath.clone()))?;
        match RelayUrl::parse(&line) {
            Ok(url) => {
                set.insert(url);
            }
            Err(e) => {
                return Err(FileError::UrlParseError(e, line));
            }
        }
    }
    Ok(set.into_iter().collect::<Vec<RelayUrl>>())
}

pub fn get_valid_key_from_file(filepath: &PathBuf) -> Result<Keys, FileError> {
    let file = std::fs::File::open(filepath).map_err(|x| FileError::ProblemWithFile(x, filepath.clone()))?;
    let reader = std::io::BufReader::new(file);
    let key = reader
        .lines()
        .next()
        .ok_or_else(|| FileError::EmptyFile(filepath.clone()))?
        .map_err(|x| FileError::ProblemWithFile(x, filepath.clone()))?;
    let key = Keys::from_str(&key).map_err(|e| FileError::KeyParseError(e, key))?;
    Ok(key)
}

pub fn broadcast_tx_inner(tx: &simplicityhl::elements::Transaction) -> crate::error::Result<String> {
    broadcast_tx(tx).map_err(|err| crate::error::CliError::Broadcast(err.to_string()))
}

pub fn decode_hex(str: impl AsRef<[u8]>) -> crate::error::Result<Vec<u8>> {
    let str_to_convert = str.as_ref();
    Ok(hex::decode(str_to_convert).map_err(|err| crate::error::CliError::FromHex(err, str_to_convert.to_hex()))?)
}

#[instrument(err)]
pub fn vec_to_arr<const N: usize, T: Debug>(vec: Vec<T>) -> crate::error::Result<[T; N]> {
    if vec.len() < N {
        return Err(crate::error::CliError::InvalidElementsSize {
            got: vec.len(),
            expected: N,
        });
    }
    let arr: [T; N] = vec
        .into_iter()
        .take(N)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|e| crate::error::CliError::Custom(format!("Failed to remove elements from '{:?}'", e)))?;

    Ok(arr)
}

pub const PUBLIC_SECRET_KEY: [u8; 32] = [2; 32];

#[inline]
pub fn derive_public_oracle_keypair() -> crate::error::Result<secp256k1::Keypair> {
    let blinder_key =
        secp256k1::Keypair::from_secret_key(secp256k1::SECP256K1, &SecretKey::from_slice(&PUBLIC_SECRET_KEY)?);
    Ok(blinder_key)
}

#[inline]
pub fn derive_oracle_pubkey() -> crate::error::Result<PublicKey> {
    Ok(derive_public_oracle_keypair()?.public_key())
}

pub fn entropy_to_asset_id(el: impl AsRef<[u8]>) -> crate::error::Result<AssetId> {
    use simplicity::hashes::sha256;
    let el = el.as_ref();
    let mut asset_entropy_bytes =
        <[u8; 32]>::from_hex(el).map_err(|err| crate::error::CliError::FromHex(err, el.to_hex()))?;
    asset_entropy_bytes.reverse();
    let midstate = sha256::Midstate::from_byte_array(asset_entropy_bytes);
    Ok(AssetId::from_entropy(midstate))
}

#[test]
fn test() {
    let x = derive_oracle_pubkey().unwrap();
    let hex = x.x_only_public_key().0.to_hex();

    let oracle_bytes = XOnlyPublicKey::from_str(&hex).unwrap().serialize();
}
