use crate::common::FileError;
use config::ConfigError;
use nostr_relay_connector::error::RelayClientError;
use nostr_relay_processor::error::RelayProcessorError;

pub type Result<T> = core::result::Result<T, CliError>;

#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error("Occurred error with io, err: '{0}'")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    RelayClient(#[from] RelayClientError),
    #[error(transparent)]
    RelayProcessor(#[from] RelayProcessorError),
    #[error("Occurred error with usage of Dcd manager, err: {0}")]
    DcdManager(String),
    #[error("Configuration error, err: '{0}'")]
    Config(#[from] ConfigError),
    #[error("Failed to obtain utxo, '{0}'")]
    Utxo(String),
    #[error("'{0}', not set in environment or .env")]
    EnvNotSet(String),
    #[error("Failed to broadcast transaction, err: '{0}'")]
    Broadcast(String),
    #[error("Failed to obtain P2PK address, err: '{0}'")]
    P2pkAddress(String),
}
