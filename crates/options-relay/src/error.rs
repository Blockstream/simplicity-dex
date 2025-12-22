use contracts::error::TaprootPubkeyGenError;

use nostr::prelude::url;
use nostr::SignerError;

use simplicityhl::elements::bitcoin::blockdata::transaction::ParseOutPointError;
use simplicityhl_core::EncodingError;

#[derive(thiserror::Error, Debug)]
pub enum RelayError {
    #[error("Invalid relay URL")]
    InvalidRelayUrl(#[from] url::ParseError),

    #[error("No relays configured")]
    NoRelaysConfigured,

    #[error("Signer error")]
    Signer(#[from] SignerError),

    #[error("Nostr client error")]
    NostrClient(#[from] nostr_sdk::client::Error),

    #[error("No events found")]
    NoEventsFound,

    /// Triggered when encoding contract arguments (e.g., `OptionsArguments`, `SwapWithChangeArguments`)
    /// to hex/bincode format for NOSTR event tags fails.
    #[error("Encoding error")]
    Encoding(#[from] EncodingError),
}

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("Event verification failed")]
    EventVerification(#[from] nostr::event::Error),

    #[error("Invalid event kind")]
    InvalidKind,

    #[error("Missing required tag: {0}")]
    MissingTag(&'static str),

    #[error("Invalid action type")]
    InvalidAction,

    #[error("Invalid outpoint")]
    InvalidOutpoint(#[from] ParseOutPointError),

    #[error("Decoding error")]
    Decoding(#[from] EncodingError),

    #[error("Taproot verification failed")]
    TaprootVerification(#[from] TaprootPubkeyGenError),
}
