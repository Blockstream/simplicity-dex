#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

pub mod client;
pub mod config;
pub mod error;
pub mod events;

pub use client::{PublishingClient, ReadOnlyClient};
pub use config::NostrRelayConfig;
pub use error::{ParseError, RelayError};
pub use events::{
    ACTION_COMPLETED, ActionCompletedEvent, ActionType, OPTION_CREATED, OptionCreatedEvent, SWAP_CREATED,
    SwapCreatedEvent,
};
