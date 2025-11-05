use std::env::VarError;
use thiserror::Error;
use tracing::instrument;

#[derive(Debug, Error)]
pub enum EnvParserError {
    #[error("Failed to parse env variable {missing_var_name}, err: {err}, check if it exists and is valid")]
    ConfigEnvParseError { missing_var_name: String, err: VarError },
}

pub trait EnvParser {
    const ENV_NAME: &'static str;
    fn obtain_env_value() -> Result<String, EnvParserError> {
        obtain_env_value(Self::ENV_NAME)
    }
}

#[instrument(level = "trace", skip(name), fields(name = name.as_ref()), ret)]
pub fn obtain_env_value(name: impl AsRef<str>) -> Result<String, EnvParserError> {
    std::env::var(name.as_ref()).map_err(|err| EnvParserError::ConfigEnvParseError {
        missing_var_name: name.as_ref().to_string(),
        err,
    })
}
