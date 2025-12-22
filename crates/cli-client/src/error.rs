use crate::common::store::SledError;
use config::ConfigError;
use dex_nostr_relay::error::NostrRelayError;
use elements::bitcoin::secp256k1;
use tokio::task::JoinError;

pub type Result<T> = core::result::Result<T, CliError>;

#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error("Occurred error with io, err: '{0}'")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    NostrRelay(#[from] NostrRelayError),
    #[error("Occurred error with usage of Dcd manager, err: '{0}'")]
    DcdManager(String),
    #[error("Configuration error, err: '{0}'")]
    Config(#[from] ConfigError),
    #[error("Configuration error, err: '{0}'")]
    ConfigExtended(String),
    #[error("Failed to obtain utxo, '{0}'")]
    Utxo(String),
    #[error("'{0}', not set in environment or .env")]
    EnvNotSet(String),
    #[error("Failed to broadcast transaction, err: '{0}'")]
    Broadcast(String),
    #[error("Failed to obtain P2PK address, err: '{0}'")]
    P2pkAddress(String),
    #[error(transparent)]
    SledError(#[from] SledError),
    #[error("Asset name already exists, name: '{name}'")]
    AssetNameExists { name: String },
    #[error("Asset name is absent, name: '{name}'")]
    AssetNameAbsent { name: String },
    #[error("Failed to covert value from hex, err: '{0}', value: '{1}'")]
    FromHex(hex::FromHexError, String),
    #[error("Failed to convert dcd inner params into dcd params, err msg: '{0}'")]
    InnerDcdConversion(String),
    #[error("Expected at least {expected} elements, got {got}")]
    InvalidElementsSize { got: usize, expected: usize },
    #[error("Secp256k1 error: '{0}'")]
    EcCurve(#[from] secp256k1::Error),
    #[error("Failed to create DcdRatioArgs, msg: '{0}'")]
    DcdRatioArgs(String),
    #[error("Failed to obtain/save value from cache, msg: '{0}'")]
    Cache(String),
    #[error("Nostr keypair is required for the action, but it's absent")]
    NoNostrKeypairListed,
    #[error("Failed to join task, err: '{0}'")]
    TokioJoinError(#[from] JoinError),
    #[error("Occurred error with msg: '{0}'")]
    Custom(String),
}
