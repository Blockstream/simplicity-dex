#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

pub mod entry;
pub mod error;
pub mod filter;
pub mod store;

pub use entry::{QueryResult, UtxoEntry};
pub use error::StoreError;
pub use filter::Filter;
pub use store::Store;
