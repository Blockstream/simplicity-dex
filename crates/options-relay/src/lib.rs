#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

pub mod client;
pub mod config;
pub mod error;
pub mod events;

pub use client::{PublishingClient, ReadOnlyClient};
pub use config::RelayConfig;
pub use error::{ParseError, RelayError};
pub use events::{
    ActionCompletedEvent, ActionType, OptionCreatedEvent, SwapCreatedEvent,
    ACTION_COMPLETED, OPTION_CREATED, SWAP_CREATED,
};
