use crate::common::check_file_existence;
use crate::error::CliError::ConfigExtended;
use config::{Config, Environment, File, FileFormat, ValueKind};
use nostr::{Keys, RelayUrl};
use serde::{Deserialize, Deserializer};
use std::path::PathBuf;
use std::str::FromStr;
use tracing::instrument;

#[derive(Debug)]
pub struct AggregatedConfig {
    pub nostr_keypair: Option<Keys>,
    pub relays: Vec<RelayUrl>,
}

pub struct CliConfigArgs {
    pub nostr_key: Option<KeysWrapper>,
    pub relays_list: Option<Vec<RelayUrl>>,
    pub nostr_config_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct KeysWrapper(pub Keys);

impl<'de> Deserialize<'de> for KeysWrapper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let keys = Keys::from_str(&s).map_err(serde::de::Error::custom)?;
        Ok(KeysWrapper(keys))
    }
}

impl Into<ValueKind> for KeysWrapper {
    fn into(self) -> ValueKind {
        ValueKind::String(self.0.secret_key().to_secret_hex())
    }
}

impl AggregatedConfig {
    /// Parses also config values from env variables `CONF_NOSTR_KEYPAIR`, `CONF_RELAYS`
    #[instrument(level = "debug", skip(cli_args))]
    pub fn new(cli_args: CliConfigArgs) -> crate::error::Result<Self> {
        const NOSTR_KEYPAIR_CONFIG_FIELD_NAME: &str = "nostr_keypair";
        const RELAYS_CONFIG_FIELD_NAME: &str = "relays";
        const ENV_VARIABLE_SEPARATOR: &'static str = ",";
        const DEFAULT_CONFIG_PATH: &'static str = ".simplicity-dex.config.toml";

        #[derive(Deserialize, Debug)]
        pub struct AggregatedConfigInner {
            pub nostr_keypair: Option<KeysWrapper>,
            pub relays: Option<Vec<RelayUrl>>,
        }

        let CliConfigArgs {
            nostr_key,
            relays_list,
            nostr_config_path,
        } = cli_args;

        let _ = dotenvy::dotenv();
        let mut config_builder =
            Config::builder().add_source(
                Environment::default()
                    .list_separator(ENV_VARIABLE_SEPARATOR)
                    .with_list_parse_key(ENV_VARIABLE_SEPARATOR)
                    .try_parsing(true)
            );

        // Add default config path
        if let Ok(path) = check_file_existence(DEFAULT_CONFIG_PATH) {
            tracing::debug!("Default config file found at '{:?}'", path);
            config_builder = config_builder.add_source(File::from(path).format(FileFormat::Toml));
        } else {
            tracing::debug!("No config file found at '{}'", DEFAULT_CONFIG_PATH);
        }

        // Add custom config path
        if let Some(path) = nostr_config_path {
            tracing::debug!(
                "Custom config file found at '{:?}', canonical: '{:?}'",
                path,
                std::fs::canonicalize(&path)
            );
            config_builder = config_builder.add_source(File::from(path).format(FileFormat::Toml));
        } else {
            tracing::debug!("No custom config file were passed");
        }

        // Add possible cli value
        tracing::debug!("Adding custom keypair value, is_some: '{}'", nostr_key.is_some());
        config_builder = config_builder.set_override_option(NOSTR_KEYPAIR_CONFIG_FIELD_NAME, nostr_key)?;

        // Add possible relays value
        tracing::debug!("Adding custom relays values, relays: '{:?}'", relays_list);
        config_builder = config_builder.set_override_option(
            RELAYS_CONFIG_FIELD_NAME,
            relays_list.map(|x| x.iter().map(|r| r.to_string()).collect::<Vec<String>>()),
        )?;

        let config = config_builder.build()?;
        let config = match config.try_deserialize::<AggregatedConfigInner>() {
            Ok(conf) => Ok(conf),
            Err(e) => Err(crate::error::CliError::ConfigExtended(format!(
                "Got error in gathering AggregatedConfigInner, error: {e:?}"
            ))),
        }?;

        if config.relays.is_none() {
            return Err(ConfigExtended("No relays found in configuration..".to_string()));
        } else if let Some(x) = config.relays.as_ref() && x.is_empty(){
            return Err(ConfigExtended("Relays configuration is empty..".to_string()));
        }

        tracing::debug!("Config gathered: '{:?}'", config);
        Ok(AggregatedConfig {
            nostr_keypair: config.nostr_keypair.map(|x| x.0),
            relays: config.relays.unwrap(),
        })
    }
}
