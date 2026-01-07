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
    ACTION_COMPLETED, ACTION_OPTION_CANCELLED, ACTION_OPTION_CREATED, ACTION_OPTION_EXERCISED, ACTION_OPTION_EXPIRED,
    ACTION_OPTION_FUNDED, ACTION_SETTLEMENT_CLAIMED, ACTION_SWAP_CANCELLED, ACTION_SWAP_CREATED, ACTION_SWAP_EXERCISED,
    ActionCompletedEvent, ActionType, OPTION_CREATED, OptionCreatedEvent, SWAP_CREATED, SwapCreatedEvent,
};
