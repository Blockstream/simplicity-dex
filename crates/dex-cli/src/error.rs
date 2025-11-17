use crate::utils::FileError;

use dex_nostr_relay::error::NostrRelayError;

pub type Result<T> = core::result::Result<T, CliError>;

#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error("Occurred error with io, err: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    NostrRelay(#[from] NostrRelayError),
}
