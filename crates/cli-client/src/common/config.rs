use crate::cli::{Cli, DEFAULT_CONFIG_PATH};
use crate::error::CliError::ConfigExtended;

use std::str::FromStr;

use config::{Config, File, FileFormat, ValueKind};

use nostr::{Keys, RelayUrl};

use serde::{Deserialize, Deserializer};

use crate::error::CliError;
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct AggregatedConfig {
    pub nostr_keypair: Option<Keys>,
    pub relays: Vec<RelayUrl>,
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

impl From<KeysWrapper> for ValueKind {
    fn from(val: KeysWrapper) -> Self {
        ValueKind::String(val.0.secret_key().to_secret_hex())
    }
}

impl AggregatedConfig {
    /// Build aggregated configuration from CLI arguments and optional config file.
    ///
    /// # Errors
    ///
    /// Returns:
    /// - `CliError::Config` if the underlying `config` builder or deserialization fails.
    /// - `CliError::ConfigExtended` if the aggregated configuration cannot be
    ///   constructed (e.g., missing or empty `relays` list).
    #[instrument(level = "debug", skip(cli))]
    pub fn new(cli: &Cli) -> crate::error::Result<Self> {
        #[derive(Deserialize, Debug)]
        pub struct AggregatedConfigInner {
            pub nostr_keypair: Option<KeysWrapper>,
            pub relays: Option<Vec<RelayUrl>>,
        }

        let Cli {
            nostr_key,
            relays_list,
            nostr_config_path,
            ..
        } = cli;

        let mut config_builder = Config::builder().add_source(
            File::from(nostr_config_path.clone())
                .format(FileFormat::Toml)
                .required(DEFAULT_CONFIG_PATH != nostr_config_path.to_string_lossy().as_ref()),
        );

        if let Some(nostr_key) = nostr_key {
            tracing::debug!("Adding keypair value from CLI");
            config_builder =
                config_builder.set_override_option("nostr_keypair", Some(KeysWrapper(nostr_key.clone())))?;
        }

        if let Some(relays) = relays_list {
            tracing::debug!("Adding relays values from CLI, relays: '{:?}'", relays);
            config_builder = config_builder.set_override_option(
                "relays",
                Some(
                    relays
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect::<Vec<String>>(),
                ),
            )?;
        }

        // TODO(Alex): add Liquid private key

        let config = match config_builder.build()?.try_deserialize::<AggregatedConfigInner>() {
            Ok(conf) => Ok(conf),
            Err(e) => Err(ConfigExtended(format!(
                "Got error in gathering AggregatedConfigInner, error: {e:?}"
            ))),
        }?;

        let Some(relays) = config.relays else {
            return Err(ConfigExtended("No relays found in configuration..".to_string()));
        };

        if relays.is_empty() {
            return Err(ConfigExtended("Relays configuration is empty..".to_string()));
        }

        let aggregated_config = AggregatedConfig {
            nostr_keypair: config.nostr_keypair.map(|x| x.0),
            relays,
        };

        tracing::debug!("Config gathered: '{:?}'", aggregated_config);

        Ok(aggregated_config)
    }

    /// Ensure that a Nostr keypair is present in the aggregated configuration.
    ///
    /// # Errors
    ///
    /// Returns `CliError::NoNostrKeypairListed` if `nostr_keypair` is `None`.
    pub fn check_nostr_keypair_existence(&self) -> crate::error::Result<()> {
        if self.nostr_keypair.is_none() {
            return Err(CliError::NoNostrKeypairListed);
        }
        Ok(())
    }
}
