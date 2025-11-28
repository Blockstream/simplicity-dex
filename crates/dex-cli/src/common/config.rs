use crate::cli::{Cli, DEFAULT_CONFIG_PATH};
use crate::error::CliError::ConfigExtended;

use std::str::FromStr;

use config::{Config, File, FileFormat, ValueKind};

use nostr::{Keys, RelayUrl};

use serde::{Deserialize, Deserializer};

use crate::error::CliError;
use tracing::instrument;

/// `MAKER_EXPIRATION_TIME` = 31 days
const MAKER_EXPIRATION_TIME: u64 = 2_678_400;

#[derive(Debug, Clone)]
pub struct Seed(pub SeedInner);
pub type SeedInner = [u8; 32];

#[derive(Debug)]
pub struct AggregatedConfig {
    pub nostr_keypair: Option<Keys>,
    pub relays: Vec<RelayUrl>,
    pub seed_hex: Option<Seed>,
    pub maker_expiration_time: u64,
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

impl FromStr for Seed {
    type Err = CliError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s).map_err(|err| crate::error::CliError::FromHex(err, s.to_string()))?;
        if bytes.len() != 32 {
            return Err(CliError::InvalidSeedLength {
                got: bytes.len(),
                expected: 32,
            });
        }
        let mut inner = [0u8; 32];
        inner.copy_from_slice(&bytes);
        Ok(Seed(inner))
    }
}

impl<'de> Deserialize<'de> for Seed {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Seed::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl From<Seed> for ValueKind {
    fn from(val: Seed) -> Self {
        ValueKind::String(hex::encode(val.0))
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
            pub seed_hex: Option<Seed>,
            pub maker_expiration_time: u64,
        }

        let Cli {
            nostr_key,
            relays_list,
            nostr_config_path,
            seed_hex,
            maker_expiration_time,
            ..
        } = cli;

        let mut config_builder = Config::builder()
            .add_source(
                File::from(nostr_config_path.clone())
                    .format(FileFormat::Toml)
                    .required(DEFAULT_CONFIG_PATH != nostr_config_path.to_string_lossy().as_ref()),
            )
            .set_default("maker_expiration_time", MAKER_EXPIRATION_TIME)?;

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

        if let Some(seed_hex) = seed_hex {
            tracing::debug!("Adding SeedHex value from CLI");
            config_builder = config_builder.set_override_option("seed_hex", Some(seed_hex.clone()))?;
        }

        if let Some(maker_expiration_time) = maker_expiration_time {
            tracing::debug!(
                "Adding expiration time from config, expiration_time: '{:?}'",
                maker_expiration_time
            );
            config_builder =
                config_builder.set_override_option("maker_expiration_time", Some(*maker_expiration_time))?;
        }

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
            seed_hex: config.seed_hex,
            maker_expiration_time: config.maker_expiration_time,
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

    /// Ensure that a Seed hex is present in the aggregated configuration.
    ///
    /// # Errors
    ///
    /// Returns `CliError::NoSeedHex` if `seed_hex` is `None`.
    pub fn check_seed_hex_existence(&self) -> crate::error::Result<()> {
        if self.seed_hex.is_none() {
            return Err(CliError::NoSeedHex);
        }
        Ok(())
    }
}
