#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("Signer error: {0}")]
    Signer(#[from] signer::SignerError),

    #[error("Store error: {0}")]
    Store(#[from] coin_store::StoreError),

    #[error("Explorer error: {0}")]
    Explorer(#[from] cli_helper::explorer::ExplorerError),

    #[error("Contract error: {0}")]
    Contract(#[from] contracts::error::TransactionBuildError),

    #[error("Program error: {0}")]
    Program(#[from] simplicityhl_core::ProgramError),

    #[error("PSET error: {0}")]
    Pset(#[from] simplicityhl::elements::pset::Error),

    #[error("Hex error: {0}")]
    Hex(#[from] hex::FromHexError),
}
