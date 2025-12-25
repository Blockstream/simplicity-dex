use simplicityhl::elements::secp256k1_zkp::UpstreamError;
use simplicityhl::elements::{OutPoint, UnblindError};
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    #[error("Database already exists: {0}")]
    DbAlreadyExists(PathBuf),

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

    #[error("Missing serialized TxOutWitness for output: {0}")]
    MissingSerializedTxOutWitness(OutPoint),

    #[error("Encoding error, err: {0}")]
    Encoding(#[from] simplicityhl::elements::encode::Error),

    #[error("Bincode encoding error, err: {0}")]
    BincodeEncoding(#[from] bincode::error::EncodeError),

    #[error("Bincode decoding error, err: {0}")]
    BincodeDecoding(#[from] bincode::error::DecodeError),

    #[error("Invalid secret key, err: {0}")]
    InvalidSecretKey(#[from] UpstreamError),

    #[error("Unblind error, err: {0}")]
    Unblind(#[from] UnblindError),

    #[error("SQLx error, err: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Migration error, err: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("Value overflow during calculation")]
    ValueOverflow,

    #[error("Simplicity compilation error: {0}")]
    SimplicityCompilation(String),
}
