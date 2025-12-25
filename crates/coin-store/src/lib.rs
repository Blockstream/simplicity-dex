#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

pub mod entry;
pub mod error;
pub mod executor;
pub mod filter;
pub mod store;

pub use error::StoreError;
pub use simplicityhl::elements::AssetId;
pub use store::Store;

pub use entry::{UtxoEntry, UtxoQueryResult};
pub use executor::UtxoStore;
pub use filter::UtxoFilter;
