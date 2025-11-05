pub type Result<T> = core::result::Result<T, CliError>;

#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error("Occcurred error with io, err: {0}")]
    Io(#[from] std::io::Error),
}
