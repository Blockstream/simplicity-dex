use crate::cli::{Cli, DEFAULT_CONFIG_PATH};
use crate::error::CliError;
use config::{Config, File, FileFormat, ValueKind};
use nostr::{Keys, RelayUrl};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;
use tracing::instrument;

/// `MAKER_EXPIRATION_TIME` = 31 days
const MAKER_EXPIRATION_TIME: u64 = 2_678_400;

#[derive(Debug, Clone, Serialize)]
pub struct HexSeed(pub String);

#[derive(Debug, Clone)]
pub struct AggregatedConfig {
    pub nostr_keypair: Option<Keys>,
    pub relays: Vec<RelayUrl>,
    pub seed_hex: Option<HexSeed>,
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

impl Serialize for KeysWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.secret_key().to_secret_hex())
    }
}

impl HexSeed {
    /// Create a new `HexSeed` from a hex-encoded string.
    ///
    /// # Errors
    ///
    /// Returns:
    /// - `CliError::FromHex` if the input string is not valid hexadecimal.
    /// - `CliError::InvalidSeedLength` if the decoded bytes are not exactly 32 bytes long.
    pub fn new(val: impl AsRef<str>) -> Result<Self, CliError> {
        let val_str = val.as_ref();
        let bytes = hex::decode(val_str).map_err(|err| crate::error::CliError::FromHex(err, val_str.to_string()))?;
        if bytes.len() != 32 {
            return Err(CliError::InvalidSeedLength {
                got: bytes.len(),
                expected: 32,
            });
        }
        Ok(HexSeed(val_str.to_string()))
    }
}

impl FromStr for HexSeed {
    type Err = CliError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        tracing::debug!("HexSeed from str");
        HexSeed::new(s)
    }
}

impl<'de> Deserialize<'de> for HexSeed {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        tracing::debug!("Seed deserialize");
        let s = String::deserialize(deserializer)?;
        HexSeed::new(&s).map_err(serde::de::Error::custom)
    }
}

impl From<HexSeed> for ValueKind {
    fn from(val: HexSeed) -> Self {
        ValueKind::String(val.0)
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
        let Cli {
            nostr_key,
            relays_list,
            nostr_config_path,
            seed_hex,
            maker_expiration_time,
            ..
        } = cli;

        #[derive(Deserialize, Serialize, Debug)]
        struct AggregatedConfigInner {
            pub nostr_keypair: Option<KeysWrapper>,
            pub relays: Option<Vec<RelayUrl>>,
            pub seed_hex: Option<HexSeed>,
            pub maker_expiration_time: u64,
        }

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
            Err(e) => Err(CliError::ConfigExtended(format!(
                "Got error in gathering AggregatedConfigInner, error: {e:?}"
            ))),
        }?;

        let Some(relays) = config.relays else {
            return Err(CliError::ConfigExtended(
                "No relays found in configuration..".to_string(),
            ));
        };

        if relays.is_empty() {
            return Err(CliError::ConfigExtended("Relays configuration is empty..".to_string()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const TEST_NOSTR_KEY: &str = "nsec1j4c6269y9w0q2er2xjw8sv2ehyrtfxq3jwgdlxj6qfn8z4gjsq5qfvfk99";
    const TEST_RELAY_1: &str = "wss://relay1.example.com";
    const TEST_RELAY_2: &str = "wss://relay2.example.com";
    const TEST_SEED_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const TEST_EXPIRATION_TIME: u64 = 86400;
    const NOSTR_CONFIG_CLI_CMD: &str = "--nostr-config-path";
    const CLI_TEST_NOSTR_KEY: &str = "nsec1ufnus6pju578ste3v90xd5m2decpuzpql2295m3sknqcjzyys9ls0qlc85";
    const CLI_TEST_SEED_HEX: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
    const RELAYS_LIST_CLI_CMD: &str = "--relays-list";
    const SHOW_CONFIG_CLI_CMD: &str = "show-config";
    const NOSTR_KEY_CLI_CMD: &str = "--nostr-key";
    const SEED_HEX_CLI_CMD: &str = "--seed-hex";
    const MAKER_EXPIRATION_TIME_CLI_CMD: &str = "--maker-expiration-time";
    const TEST_PROGRAM_NAME_CLI_CMD: &str = "test-program";

    #[derive(Deserialize, Serialize, Debug)]
    struct AggregatedConfigInner {
        pub nostr_keypair: Option<KeysWrapper>,
        pub relays: Option<Vec<RelayUrl>>,
        pub seed_hex: Option<HexSeed>,
        pub maker_expiration_time: Option<u64>,
    }

    fn create_temp_config_file(config_inner: &AggregatedConfigInner) -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("test_config.toml");
        let toml_content = toml::to_string(config_inner).expect("Failed to serialize config to TOML");
        fs::write(&config_path, toml_content).expect("Failed to write config file");
        (temp_dir, config_path)
    }

    /// Helper function to create a minimal CLI instance for testing
    fn create_test_cli(config_path: &Path) -> Cli {
        let args = vec![
            TEST_PROGRAM_NAME_CLI_CMD,
            NOSTR_CONFIG_CLI_CMD,
            config_path.to_str().unwrap(),
            SHOW_CONFIG_CLI_CMD,
        ];
        Cli::parse_from(args)
    }

    #[test]
    fn test_config_from_file_only() -> anyhow::Result<()> {
        let config_inner = AggregatedConfigInner {
            nostr_keypair: Some(KeysWrapper(Keys::from_str(TEST_NOSTR_KEY)?)),
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: Some(HexSeed::new(TEST_SEED_HEX)?),
            maker_expiration_time: Some(TEST_EXPIRATION_TIME),
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert!(config.nostr_keypair.is_some());
        assert_eq!(config.relays.len(), 1);
        assert_eq!(config.relays[0].to_string(), TEST_RELAY_1);
        assert!(config.seed_hex.is_some());
        assert_eq!(config.seed_hex.unwrap().0, TEST_SEED_HEX);
        assert_eq!(config.maker_expiration_time, TEST_EXPIRATION_TIME);
        Ok(())
    }

    #[test]
    fn test_config_cli_overrides_file_nostr_key() -> anyhow::Result<()> {
        let file_key = TEST_NOSTR_KEY;
        let cli_key = CLI_TEST_NOSTR_KEY;

        let config_inner = AggregatedConfigInner {
            nostr_keypair: Some(KeysWrapper(Keys::from_str(file_key)?)),
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: None,
            maker_expiration_time: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);

        let args = vec![
            TEST_PROGRAM_NAME_CLI_CMD,
            NOSTR_CONFIG_CLI_CMD,
            config_path.to_str().unwrap(),
            NOSTR_KEY_CLI_CMD,
            cli_key,
            SHOW_CONFIG_CLI_CMD,
        ];
        let cli = Cli::parse_from(args);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert!(config.nostr_keypair.is_some());
        let cli_keys = Keys::from_str(cli_key)?;
        assert_eq!(
            config.nostr_keypair.unwrap().secret_key().to_secret_hex(),
            cli_keys.secret_key().to_secret_hex()
        );
        Ok(())
    }

    #[test]
    fn test_config_cli_overrides_file_relays() -> anyhow::Result<()> {
        let config_inner = AggregatedConfigInner {
            nostr_keypair: None,
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: None,
            maker_expiration_time: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);

        let args = vec![
            TEST_PROGRAM_NAME_CLI_CMD,
            NOSTR_CONFIG_CLI_CMD,
            config_path.to_str().unwrap(),
            RELAYS_LIST_CLI_CMD,
            TEST_RELAY_2,
            SHOW_CONFIG_CLI_CMD,
        ];
        let cli = Cli::parse_from(args);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert_eq!(config.relays.len(), 1);
        assert_eq!(config.relays[0].to_string(), TEST_RELAY_2);
        Ok(())
    }

    #[test]
    fn test_config_cli_overrides_file_seed_hex() -> anyhow::Result<()> {
        let file_seed = TEST_SEED_HEX;
        let cli_seed = CLI_TEST_SEED_HEX;

        let config_inner = AggregatedConfigInner {
            nostr_keypair: None,
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: Some(HexSeed::new(file_seed)?),
            maker_expiration_time: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);

        let args = vec![
            TEST_PROGRAM_NAME_CLI_CMD,
            NOSTR_CONFIG_CLI_CMD,
            config_path.to_str().unwrap(),
            SEED_HEX_CLI_CMD,
            cli_seed,
            SHOW_CONFIG_CLI_CMD,
        ];
        let cli = Cli::parse_from(args);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert!(config.seed_hex.is_some());
        assert_eq!(config.seed_hex.unwrap().0, cli_seed);
        Ok(())
    }

    #[test]
    fn test_config_cli_overrides_file_maker_expiration_time() -> anyhow::Result<()> {
        let file_expiration = TEST_EXPIRATION_TIME;
        let cli_expiration = 172_800_u64;

        let config_inner = AggregatedConfigInner {
            nostr_keypair: None,
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: None,
            maker_expiration_time: Some(file_expiration),
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);

        let args = vec![
            TEST_PROGRAM_NAME_CLI_CMD.to_string(),
            NOSTR_CONFIG_CLI_CMD.to_string(),
            config_path.to_str().unwrap().to_string(),
            MAKER_EXPIRATION_TIME_CLI_CMD.to_string(),
            cli_expiration.to_string(),
            SHOW_CONFIG_CLI_CMD.to_string(),
        ];
        let cli = Cli::parse_from(args);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert_eq!(config.maker_expiration_time, cli_expiration);
        Ok(())
    }

    #[test]
    fn test_config_multiple_cli_overrides() -> anyhow::Result<()> {
        let config_inner = AggregatedConfigInner {
            nostr_keypair: Some(KeysWrapper(Keys::from_str(TEST_NOSTR_KEY)?)),
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: Some(HexSeed::new(TEST_SEED_HEX)?),
            maker_expiration_time: Some(TEST_EXPIRATION_TIME),
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);

        let cli_key = CLI_TEST_NOSTR_KEY;
        let cli_relay = TEST_RELAY_2;
        let cli_seed = CLI_TEST_SEED_HEX;
        let cli_expiration = 172_800_u64;

        let args = vec![
            TEST_PROGRAM_NAME_CLI_CMD.to_string(),
            NOSTR_CONFIG_CLI_CMD.to_string(),
            config_path.to_str().unwrap().to_string(),
            NOSTR_KEY_CLI_CMD.to_string(),
            cli_key.to_string(),
            RELAYS_LIST_CLI_CMD.to_string(),
            cli_relay.to_string(),
            SEED_HEX_CLI_CMD.to_string(),
            cli_seed.to_string(),
            MAKER_EXPIRATION_TIME_CLI_CMD.to_string(),
            cli_expiration.to_string(),
            SHOW_CONFIG_CLI_CMD.to_string(),
        ];
        let cli = Cli::parse_from(args);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        // Verify all override file values
        assert!(config.nostr_keypair.is_some());
        let cli_keys = Keys::from_str(cli_key)?;
        assert_eq!(
            config.nostr_keypair.unwrap().secret_key().to_secret_hex(),
            cli_keys.secret_key().to_secret_hex()
        );

        assert_eq!(config.relays.len(), 1);
        assert_eq!(config.relays[0].to_string(), cli_relay);

        assert!(config.seed_hex.is_some());
        assert_eq!(config.seed_hex.unwrap().0, cli_seed);

        assert_eq!(config.maker_expiration_time, cli_expiration);
        Ok(())
    }

    #[test]
    fn test_config_default_maker_expiration_time() -> anyhow::Result<()> {
        let config_inner = AggregatedConfigInner {
            nostr_keypair: None,
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: None,
            maker_expiration_time: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert_eq!(config.maker_expiration_time, MAKER_EXPIRATION_TIME);
        Ok(())
    }

    #[test]
    fn test_config_missing_relays_error() {
        let config_inner = AggregatedConfigInner {
            nostr_keypair: None,
            relays: None,
            seed_hex: None,
            maker_expiration_time: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let result = AggregatedConfig::new(&cli);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CliError::ConfigExtended(_)));
    }

    #[test]
    fn test_config_empty_relays_error() {
        let config_inner = AggregatedConfigInner {
            nostr_keypair: None,
            relays: Some(vec![]),
            seed_hex: None,
            maker_expiration_time: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let result = AggregatedConfig::new(&cli);

        assert!(result.is_err());
        match result.unwrap_err() {
            CliError::ConfigExtended(msg) => {
                assert!(msg.contains("empty"));
            }
            _ => panic!("Expected ConfigExtended error"),
        }
    }

    #[test]
    fn test_config_multiple_relays() -> anyhow::Result<()> {
        let relay3 = "wss://relay3.example.com";
        let config_inner = AggregatedConfigInner {
            nostr_keypair: None,
            relays: Some(vec![
                RelayUrl::parse(TEST_RELAY_1)?,
                RelayUrl::parse(TEST_RELAY_2)?,
                RelayUrl::parse(relay3)?,
            ]),
            seed_hex: None,
            maker_expiration_time: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert_eq!(config.relays.len(), 3);
        assert_eq!(config.relays[0].to_string(), TEST_RELAY_1);
        assert_eq!(config.relays[1].to_string(), TEST_RELAY_2);
        assert_eq!(config.relays[2].to_string(), relay3);
        Ok(())
    }

    #[test]
    fn test_config_cli_multiple_relays() -> anyhow::Result<()> {
        let config_inner = AggregatedConfigInner {
            nostr_keypair: None,
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: None,
            maker_expiration_time: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);

        let args = vec![
            TEST_PROGRAM_NAME_CLI_CMD.to_string(),
            NOSTR_CONFIG_CLI_CMD.to_string(),
            config_path.to_str().unwrap().to_string(),
            RELAYS_LIST_CLI_CMD.to_string(),
            format!("{},{}", TEST_RELAY_2, "wss://relay3.example.com"),
            SHOW_CONFIG_CLI_CMD.to_string(),
        ];
        let cli = Cli::parse_from(args);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert_eq!(config.relays.len(), 2);
        assert_eq!(config.relays[0].to_string(), TEST_RELAY_2);
        assert_eq!(config.relays[1].to_string(), "wss://relay3.example.com");
        Ok(())
    }

    #[test]
    fn test_config_optional_fields_none() -> anyhow::Result<()> {
        let config_inner = AggregatedConfigInner {
            nostr_keypair: None,
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: None,
            maker_expiration_time: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert!(config.nostr_keypair.is_none());
        assert!(config.seed_hex.is_none());
        Ok(())
    }

    #[test]
    fn test_check_nostr_keypair_existence_present() -> anyhow::Result<()> {
        let config_inner = AggregatedConfigInner {
            nostr_keypair: Some(KeysWrapper(Keys::from_str(TEST_NOSTR_KEY)?)),
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: None,
            maker_expiration_time: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");
        let result = config.check_nostr_keypair_existence();

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_check_nostr_keypair_existence_absent() -> anyhow::Result<()> {
        let config_inner = AggregatedConfigInner {
            nostr_keypair: None,
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: None,
            maker_expiration_time: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");
        let result = config.check_nostr_keypair_existence();

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CliError::NoNostrKeypairListed));
        Ok(())
    }

    #[test]
    fn test_check_seed_hex_existence_present() -> anyhow::Result<()> {
        let config_inner = AggregatedConfigInner {
            nostr_keypair: None,
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: Some(HexSeed::new(TEST_SEED_HEX)?),
            maker_expiration_time: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");
        let result = config.check_seed_hex_existence();

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_check_seed_hex_existence_absent() -> anyhow::Result<()> {
        let config_inner = AggregatedConfigInner {
            nostr_keypair: None,
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: None,
            maker_expiration_time: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");
        let result = config.check_seed_hex_existence();

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CliError::NoSeedHex));
        Ok(())
    }

    #[test]
    fn test_hexseed_new_valid() -> anyhow::Result<()> {
        let result = HexSeed::new(TEST_SEED_HEX);
        assert!(result.is_ok());
        assert_eq!(result?.0, TEST_SEED_HEX);
        Ok(())
    }

    #[test]
    fn test_hexseed_new_invalid_hex() {
        let invalid_hex = "not_valid_hex_string_zzz!";
        let result = HexSeed::new(invalid_hex);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CliError::FromHex(..)));
    }

    #[test]
    fn test_hexseed_new_invalid_length_short() {
        let short_hex = "0123456789abcdef"; // Only 16 hex chars = 8 bytes
        let result = HexSeed::new(short_hex);
        assert!(result.is_err());
        match result.unwrap_err() {
            CliError::InvalidSeedLength { got, expected } => {
                assert_eq!(expected, 32);
                assert_eq!(got, 8);
            }
            _ => panic!("Expected InvalidSeedLength error"),
        }
    }

    #[test]
    fn test_hexseed_new_invalid_length_long() {
        let long_hex = format!("{TEST_SEED_HEX}00");
        let result = HexSeed::new(long_hex);
        assert!(result.is_err());
        match result.unwrap_err() {
            CliError::InvalidSeedLength { got, expected } => {
                assert_eq!(expected, 32);
                assert_eq!(got, 33);
            }
            _ => panic!("Expected InvalidSeedLength error"),
        }
    }

    /// Verifies the priority: CLI args > Config file > Defaults
    #[test]
    fn test_config_priority_order() -> anyhow::Result<()> {
        let config_inner = AggregatedConfigInner {
            nostr_keypair: None,
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: None,
            maker_expiration_time: Some(TEST_EXPIRATION_TIME),
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);

        // Only config file (no CLI overrides)
        let cli1 = create_test_cli(&config_path.clone());
        let config1 = AggregatedConfig::new(&cli1).expect("Failed to create config");
        assert_eq!(config1.maker_expiration_time, TEST_EXPIRATION_TIME);

        // CLI overrides config file
        let cli_expiration = 999_999_u64;
        let args = vec![
            TEST_PROGRAM_NAME_CLI_CMD.to_string(),
            NOSTR_CONFIG_CLI_CMD.to_string(),
            config_path.to_str().unwrap().to_string(),
            MAKER_EXPIRATION_TIME_CLI_CMD.to_string(),
            cli_expiration.to_string(),
            SHOW_CONFIG_CLI_CMD.to_string(),
        ];
        let cli2 = Cli::parse_from(args);
        let config2 = AggregatedConfig::new(&cli2).expect("Failed to create config");
        assert_eq!(config2.maker_expiration_time, cli_expiration);

        // Test 3: Default value when neither CLI nor config specify
        let minimal_config = AggregatedConfigInner {
            nostr_keypair: None,
            relays: Some(vec![RelayUrl::parse(TEST_RELAY_1)?]),
            seed_hex: None,
            maker_expiration_time: None,
        };
        let (_temp_dir3, config_path3) = create_temp_config_file(&minimal_config);
        let cli3 = create_test_cli(&config_path3);
        let config3 = AggregatedConfig::new(&cli3).expect("Failed to create config");
        assert_eq!(config3.maker_expiration_time, MAKER_EXPIRATION_TIME);
        Ok(())
    }

    #[test]
    fn test_config_nonexistent_file_with_non_default_path() {
        let nonexistent_path = PathBuf::from("/tmp/nonexistent_config_file_12345.toml");

        let args = vec![
            TEST_PROGRAM_NAME_CLI_CMD,
            NOSTR_CONFIG_CLI_CMD,
            nonexistent_path.to_str().unwrap(),
            SHOW_CONFIG_CLI_CMD,
        ];
        let cli = Cli::parse_from(args);

        let result = AggregatedConfig::new(&cli);

        assert!(result.is_err());
    }

    #[test]
    fn test_config_nonexistent_default_file_with_cli_relays() -> anyhow::Result<()> {
        let nonexistent_path = PathBuf::from(DEFAULT_CONFIG_PATH);

        let args = vec![
            TEST_PROGRAM_NAME_CLI_CMD,
            NOSTR_CONFIG_CLI_CMD,
            nonexistent_path.to_str().unwrap(),
            RELAYS_LIST_CLI_CMD,
            TEST_RELAY_1,
            SHOW_CONFIG_CLI_CMD,
        ];
        let cli = Cli::parse_from(args);

        let result = AggregatedConfig::new(&cli);

        assert!(result.is_ok());
        let config = result?;
        assert_eq!(config.relays.len(), 1);
        assert_eq!(config.relays[0].to_string(), TEST_RELAY_1);
        Ok(())
    }
}
