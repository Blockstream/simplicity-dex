use nostr::SignerError;
use nostr::filter::SingleLetterTagError;

#[derive(thiserror::Error, Debug)]
pub enum NostrRelayError {
    #[error("Signer error: {0}")]
    Signer(#[from] SignerError),
    #[error("Single letter error: {0}")]
    SingleLetterTag(#[from] SingleLetterTagError),
    #[error("Failed to convert custom url to RelayURL, err: {err_msg}")]
    FailedToConvertRelayUrl { err_msg: String },
    #[error("An error occurred in Nostr Client, err: {0}")]
    NostrClientFailure(#[from] nostr_sdk::client::Error),
    #[error("Relay Client requires for operation signature, add key to the Client")]
    MissingSigner,
    #[error("No events found by filter: '{0}'")]
    NoEventsFound(String),
    #[error("Found many events, but required to be only one with filter: '{0}'")]
    NotOnlyOneEventFound(String),
    #[error("Failed to encode '{struct_to_encode}', err: `{err}`")]
    BincodeEncoding {
        err: bincode::error::EncodeError,
        struct_to_encode: String,
    },
}

pub type Result<T> = std::result::Result<T, NostrRelayError>;
