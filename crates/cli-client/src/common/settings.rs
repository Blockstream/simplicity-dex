use config::{Case, Config};
use tracing::instrument;

pub struct Seed(pub SeedInner);
pub type SeedInner = [u8; 32];
pub struct SeedHex {
    pub seed_hex: String,
}

impl SeedHex {
    pub const ENV_NAME: &'static str = "SEED_HEX";
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub seed_hex: String,
}

impl Settings {
    /// Load CLI settings from environment variables.
    ///
    /// # Errors
    ///
    /// Returns:
    /// - `CliError::Config` if building the configuration from the environment fails.
    /// - `CliError::EnvNotSet` if the [`SeedHex::ENV_NAME`] environment variable is not set
    ///   or cannot be read as a UTF-8 string.
    #[instrument(level = "debug", ret)]
    pub fn load() -> crate::error::Result<Self> {
        let cfg = Config::builder()
            .add_source(
                config::Environment::default()
                    .separator("__")
                    .convert_case(Case::ScreamingSnake),
            )
            .build()
            .map_err(crate::error::CliError::Config)?;

        let seed_hex = cfg
            .get_string(SeedHex::ENV_NAME)
            .map_err(|_| crate::error::CliError::EnvNotSet(SeedHex::ENV_NAME.to_string()))?;

        Ok(Self { seed_hex })
    }
}
