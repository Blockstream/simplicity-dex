use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RelayConfig {
    primary_relay: String,
    backup_relays: Vec<String>,
    timeout: Duration,
    retry_count: u32,
}

impl RelayConfig {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
    pub const DEFAULT_RETRY_COUNT: u32 = 3;

    #[must_use]
    pub fn new(primary_relay: impl Into<String>) -> Self {
        Self {
            primary_relay: primary_relay.into(),
            backup_relays: Vec::new(),
            timeout: Self::DEFAULT_TIMEOUT,
            retry_count: Self::DEFAULT_RETRY_COUNT,
        }
    }

    #[must_use]
    pub fn add_backup_relay(mut self, relay_url: impl Into<String>) -> Self {
        self.backup_relays.push(relay_url.into());
        self
    }

    #[must_use]
    pub fn add_backup_relays(mut self, relay_urls: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.backup_relays.extend(relay_urls.into_iter().map(Into::into));
        self
    }

    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub const fn with_retry_count(mut self, count: u32) -> Self {
        self.retry_count = count;
        self
    }

    #[must_use]
    pub fn primary_relay(&self) -> &str {
        &self.primary_relay
    }

    #[must_use]
    pub fn all_relays(&self) -> Vec<&str> {
        std::iter::once(self.primary_relay.as_str())
            .chain(self.backup_relays.iter().map(String::as_str))
            .collect()
    }

    #[must_use]
    pub const fn timeout(&self) -> Duration {
        self.timeout
    }

    #[must_use]
    pub const fn retry_count(&self) -> u32 {
        self.retry_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config = RelayConfig::new("wss://relay.example.com");

        assert_eq!(config.primary_relay(), "wss://relay.example.com");
        assert_eq!(config.all_relays().len(), 1);
        assert_eq!(config.timeout(), RelayConfig::DEFAULT_TIMEOUT);
        assert_eq!(config.retry_count(), RelayConfig::DEFAULT_RETRY_COUNT);
    }

    #[test]
    fn test_config_with_backup_relays() {
        let config = RelayConfig::new("wss://primary.example.com")
            .add_backup_relay("wss://backup1.example.com")
            .add_backup_relay("wss://backup2.example.com");

        let all = config.all_relays();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], "wss://primary.example.com");
        assert_eq!(all[1], "wss://backup1.example.com");
        assert_eq!(all[2], "wss://backup2.example.com");
    }

    #[test]
    fn test_config_with_custom_settings() {
        let config = RelayConfig::new("wss://relay.example.com")
            .with_timeout(Duration::from_secs(60))
            .with_retry_count(5);

        assert_eq!(config.timeout(), Duration::from_secs(60));
        assert_eq!(config.retry_count(), 5);
    }
}
