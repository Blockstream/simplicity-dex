use crate::cli::{Cli, DEFAULT_CONFIG_PATH};
use crate::error::CliError::ConfigExtended;

use std::str::FromStr;

use config::{Config, File, FileFormat, ValueKind};

use nostr::{Keys, RelayUrl};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

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

impl Serialize for KeysWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.secret_key().to_secret_hex())
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use serde::Serialize;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const TEST_NOSTR_KEY: &str = "nsec1j4c6269y9w0q2er2xjw8sv2ehyrtfxq3jwgdlxj6qfn8z4gjsq5qfvfk99";
    const TEST_RELAY_1: &str = "wss://relay1.example.com";
    const TEST_RELAY_2: &str = "wss://relay2.example.com";
    const TEST_RELAY_3: &str = "wss://relay3.example.com";
    const CLI_TEST_NOSTR_KEY: &str = "nsec1ufnus6pju578ste3v90xd5m2decpuzpql2295m3sknqcjzyys9ls0qlc85";
    const NOSTR_CONFIG_CLI_CMD: &str = "--nostr-config-path";
    const RELAYS_LIST_CLI_CMD: &str = "--relays-list";
    const SHOW_CONFIG_CLI_CMD: &str = "show-config";
    const NOSTR_KEY_CLI_CMD: &str = "--nostr-key";
    const TEST_PROGRAM_NAME: &str = "test-program";
    const NONEXISTENT_CONFIG_PATH: &str = "/tmp/nonexistent_config_file_12345.toml";

    #[derive(Deserialize, Serialize, Debug)]
    struct TestConfigInner {
        pub nostr_keypair: Option<KeysWrapper>,
        pub relays: Option<Vec<RelayUrl>>,
    }

    fn create_temp_config_file(config_inner: &TestConfigInner) -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("test_config.toml");
        let toml_content = toml::to_string(config_inner).expect("Failed to serialize config to TOML");
        fs::write(&config_path, toml_content).expect("Failed to write config file");
        (temp_dir, config_path)
    }

    fn create_test_cli(config_path: &Path) -> Cli {
        let args = vec![
            TEST_PROGRAM_NAME,
            NOSTR_CONFIG_CLI_CMD,
            config_path.to_str().unwrap(),
            SHOW_CONFIG_CLI_CMD,
        ];
        Cli::parse_from(args)
    }

    fn build_cli(config_path: &Path, extra_args: &[&str]) -> Cli {
        let mut args = vec![TEST_PROGRAM_NAME, NOSTR_CONFIG_CLI_CMD, config_path.to_str().unwrap()];
        args.extend_from_slice(extra_args);
        args.push(SHOW_CONFIG_CLI_CMD);
        Cli::parse_from(args)
    }

    fn create_relay_only_config(relays: &[&str]) -> anyhow::Result<TestConfigInner> {
        Ok(TestConfigInner {
            nostr_keypair: None,
            relays: Some(
                relays
                    .iter()
                    .map(|r| RelayUrl::parse(r))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        })
    }

    /// Create a config with keypair and relays (common pattern)
    fn create_full_config(nostr_key: &str, relays: &[&str]) -> anyhow::Result<TestConfigInner> {
        Ok(TestConfigInner {
            nostr_keypair: Some(KeysWrapper(Keys::from_str(nostr_key)?)),
            relays: Some(
                relays
                    .iter()
                    .map(|r| RelayUrl::parse(r))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        })
    }

    #[test]
    fn test_config_from_file_only() -> anyhow::Result<()> {
        let config_inner = create_full_config(TEST_NOSTR_KEY, &[TEST_RELAY_1])?;

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert!(config.nostr_keypair.is_some());
        assert_eq!(config.relays.len(), 1);
        assert_eq!(config.relays[0].to_string(), TEST_RELAY_1);
        Ok(())
    }

    #[test]
    fn test_config_cli_overrides_file_nostr_key() -> anyhow::Result<()> {
        let config_inner = create_full_config(TEST_NOSTR_KEY, &[TEST_RELAY_1])?;

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = build_cli(&config_path, &[NOSTR_KEY_CLI_CMD, CLI_TEST_NOSTR_KEY]);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert!(config.nostr_keypair.is_some());
        let cli_keys = Keys::from_str(CLI_TEST_NOSTR_KEY)?;
        assert_eq!(
            config.nostr_keypair.unwrap().secret_key().to_secret_hex(),
            cli_keys.secret_key().to_secret_hex()
        );
        Ok(())
    }

    #[test]
    fn test_config_cli_overrides_file_relays() -> anyhow::Result<()> {
        let config_inner = create_relay_only_config(&[TEST_RELAY_1])?;

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = build_cli(&config_path, &[RELAYS_LIST_CLI_CMD, TEST_RELAY_2]);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert_eq!(config.relays.len(), 1);
        assert_eq!(config.relays[0].to_string(), TEST_RELAY_2);
        Ok(())
    }

    #[test]
    fn test_config_multiple_cli_overrides() -> anyhow::Result<()> {
        let config_inner = create_full_config(TEST_NOSTR_KEY, &[TEST_RELAY_1])?;

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = build_cli(
            &config_path,
            &[NOSTR_KEY_CLI_CMD, CLI_TEST_NOSTR_KEY, RELAYS_LIST_CLI_CMD, TEST_RELAY_2],
        );

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        // Verify CLI overrides file values
        assert!(config.nostr_keypair.is_some());
        let cli_keys = Keys::from_str(CLI_TEST_NOSTR_KEY)?;
        assert_eq!(
            config.nostr_keypair.unwrap().secret_key().to_secret_hex(),
            cli_keys.secret_key().to_secret_hex()
        );

        assert_eq!(config.relays.len(), 1);
        assert_eq!(config.relays[0].to_string(), TEST_RELAY_2);
        Ok(())
    }

    #[test]
    fn test_config_missing_relays_error() {
        let config_inner = TestConfigInner {
            nostr_keypair: None,
            relays: None,
        };

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let result = AggregatedConfig::new(&cli);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CliError::ConfigExtended(_)));
    }

    #[test]
    fn test_config_empty_relays_error() {
        let config_inner = TestConfigInner {
            nostr_keypair: None,
            relays: Some(vec![]),
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
        let config_inner = create_relay_only_config(&[TEST_RELAY_1, TEST_RELAY_2, TEST_RELAY_3])?;

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert_eq!(config.relays.len(), 3);
        assert_eq!(config.relays[0].to_string(), TEST_RELAY_1);
        assert_eq!(config.relays[1].to_string(), TEST_RELAY_2);
        assert_eq!(config.relays[2].to_string(), TEST_RELAY_3);
        Ok(())
    }

    #[test]
    fn test_config_cli_multiple_relays() -> anyhow::Result<()> {
        let config_inner = create_relay_only_config(&[TEST_RELAY_1])?;

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);

        let relays_str = format!("{TEST_RELAY_2},{TEST_RELAY_3}");
        let cli = build_cli(&config_path, &[RELAYS_LIST_CLI_CMD, &relays_str]);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");

        assert_eq!(config.relays.len(), 2);
        assert_eq!(config.relays[0].to_string(), TEST_RELAY_2);
        assert_eq!(config.relays[1].to_string(), TEST_RELAY_3);
        Ok(())
    }

    #[test]
    fn test_check_nostr_keypair_existence_present() -> anyhow::Result<()> {
        let config_inner = create_full_config(TEST_NOSTR_KEY, &[TEST_RELAY_1])?;

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");
        let result = config.check_nostr_keypair_existence();

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_check_nostr_keypair_existence_absent() -> anyhow::Result<()> {
        let config_inner = create_relay_only_config(&[TEST_RELAY_1])?;

        let (_temp_dir, config_path) = create_temp_config_file(&config_inner);
        let cli = create_test_cli(&config_path);

        let config = AggregatedConfig::new(&cli).expect("Failed to create config");
        let result = config.check_nostr_keypair_existence();

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CliError::NoNostrKeypairListed));
        Ok(())
    }

    #[test]
    fn test_config_nonexistent_file_with_non_default_path() {
        let nonexistent_path = PathBuf::from(NONEXISTENT_CONFIG_PATH);

        let args = vec![
            TEST_PROGRAM_NAME,
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
            TEST_PROGRAM_NAME,
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
