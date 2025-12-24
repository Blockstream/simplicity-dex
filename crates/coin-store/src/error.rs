use std::path::PathBuf;

use simplicityhl::elements::secp256k1_zkp::UpstreamError;
use simplicityhl::elements::{OutPoint, UnblindError};

#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    #[error("Database already exists: {0}")]
    AlreadyExists(PathBuf),

    #[error("Database not found: {0}")]
    NotFound(PathBuf),

    #[error("Database not initialized: {0}")]
    NotInitialized(PathBuf),

    #[error("UTXO already exists: {0}")]
    UtxoAlreadyExists(OutPoint),

    #[error("UTXO not found: {0}")]
    UtxoNotFound(OutPoint),

    #[error("Missing blinder key for confidential output: {0}")]
    MissingBlinderKey(OutPoint),

    #[error("Encoding error")]
    Encoding(#[from] simplicityhl::elements::encode::Error),

    #[error("Invalid secret key")]
    InvalidSecretKey(#[from] UpstreamError),

    #[error("Unblind error")]
    Unblind(#[from] UnblindError),

    #[error("SQLx error")]
    Sqlx(#[from] sqlx::Error),

    #[error("Migration error")]
    Migration(#[from] sqlx::migrate::MigrateError),
}
