use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::Error;
use crate::explorer;
use crate::fee::DEFAULT_FEE_RATE;
use options_relay::NostrRelayConfig;
use serde::{Deserialize, Serialize};
use simplicityhl::elements::AddressParams;

const DEFAULT_CONFIG_PATH: &str = "config.toml";
const DEFAULT_DATA_DIR: &str = ".data";
const DEFAULT_DATABASE_FILENAME: &str = "coins.db";
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_RELAY: &str = "wss://relay.damus.io";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub relay: RelayConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub fee: FeeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    #[serde(default = "default_network")]
    pub name: NetworkName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NetworkName {
    #[default]
    Testnet,
    Mainnet,
}

impl NetworkName {
    #[must_use]
    pub const fn address_params(self) -> &'static AddressParams {
        match self {
            Self::Testnet => &AddressParams::LIQUID_TESTNET,
            Self::Mainnet => &AddressParams::LIQUID,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    #[serde(default = "default_relays")]
    pub urls: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
}

/// Fee estimation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeConfig {
    /// Confirmation target in blocks.
    /// Set to 0 to always use the fallback rate (no network call).
    /// Common targets: 1 (next block), 6 (1 hour), 144 (1 day).
    #[serde(default)]
    pub confirmation_target: u32,
    /// Fallback fee rate in sats/kvb if estimation fails or target is 0.
    /// Default: 100.0 sats/kvb (0.10 sat/vB) to meet Liquid minimum relay fee.
    #[serde(default = "default_fallback_rate")]
    pub fallback_rate: f32,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        Self::load(path).unwrap_or_default()
    }

    #[must_use]
    pub fn database_path(&self) -> PathBuf {
        self.storage.data_dir.join(DEFAULT_DATABASE_FILENAME)
    }

    #[must_use]
    pub const fn address_params(&self) -> &'static AddressParams {
        self.network.name.address_params()
    }

    /// Get fee rate from config or Esplora.
    /// Returns fee rate in sats/kvb.
    pub fn get_fee_rate(&self) -> f32 {
        if self.fee.confirmation_target == 0 {
            self.fee.fallback_rate
        } else {
            explorer::get_fee_rate(self.fee.confirmation_target).unwrap_or(self.fee.fallback_rate)
        }
    }
}

impl RelayConfig {
    pub fn get_nostr_relay_config(&self) -> NostrRelayConfig {
        let mut urls = self.urls.iter();

        let primary = urls.next().map_or("wss://relay.damus.io", String::as_str);

        NostrRelayConfig::new(primary)
            .add_backup_relays(urls.map(String::as_str))
            .with_timeout(Duration::from_secs(self.timeout_secs))
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            name: default_network(),
        }
    }
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            urls: default_relays(),
            timeout_secs: default_timeout(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
        }
    }
}

impl Default for FeeConfig {
    fn default() -> Self {
        Self {
            confirmation_target: 0,
            fallback_rate: default_fallback_rate(),
        }
    }
}

const fn default_network() -> NetworkName {
    NetworkName::Testnet
}

fn default_relays() -> Vec<String> {
    vec![DEFAULT_RELAY.to_string()]
}

const fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

const fn default_fallback_rate() -> f32 {
    DEFAULT_FEE_RATE
}

fn default_data_dir() -> PathBuf {
    PathBuf::from(DEFAULT_DATA_DIR)
}

#[must_use]
pub fn default_config_path() -> PathBuf {
    PathBuf::from(DEFAULT_CONFIG_PATH)
}
