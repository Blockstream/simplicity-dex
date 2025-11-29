use crate::error::NostrRelayError;

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use nostr::prelude::*;
use nostr_sdk::pool::Output;
use nostr_sdk::prelude::Events;
use nostr_sdk::{Client, Relay, SubscribeAutoCloseOptions};

use tracing::instrument;

#[derive(Debug, Clone)]
pub struct RelayClient {
    client: Client,
    timeout: Duration,
}

#[derive(Debug)]
pub struct ClientConfig {
    pub timeout: Duration,
}

impl RelayClient {
    /// Connect to one or more Nostr relays and return a configured `RelayClient`.
    ///
    /// # Errors
    ///
    /// Returns an error if a relay URL cannot be converted or if adding a relay
    /// to the underlying `nostr_sdk::Client` fails.
    #[instrument(skip_all, level = "debug", err)]
    pub async fn connect(
        relay_urls: impl IntoIterator<Item = impl TryIntoUrl>,
        keys: Option<impl IntoNostrSigner>,
        client_config: ClientConfig,
    ) -> crate::error::Result<Self> {
        tracing::debug!(client_config = ?client_config, "Connecting to Nostr Relay Client(s)");

        let client = match keys {
            None => Client::default(),
            Some(keys) => {
                let client = Client::new(keys);
                client.automatic_authentication(true);
                client
            }
        };

        for url in relay_urls {
            let url = url
                .try_into_url()
                .map_err(|err| NostrRelayError::FailedToConvertRelayUrl {
                    err_msg: format!("{err:?}"),
                })?;

            client.add_relay(url).await?;
        }

        client.connect().await;

        Ok(Self {
            client,
            timeout: client_config.timeout,
        })
    }

    /// Request events from connected relays using the provided filter.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching events from the underlying client fails.
    #[instrument(skip_all, level = "debug", ret)]
    pub async fn req_and_wait(&self, filter: Filter) -> crate::error::Result<Events> {
        tracing::debug!(filter = ?filter, "Requesting events with filter");

        Ok(self.client.fetch_combined_events(filter, self.timeout).await?)
    }

    /// Return the configured signer for this relay client.
    ///
    /// # Errors
    ///
    /// Returns `NostrRelayError::MissingSigner` if no signer is configured,
    /// or an error if obtaining the signer from the underlying client fails.
    #[instrument(skip_all, level = "debug", ret)]
    pub async fn get_signer(&self) -> crate::error::Result<Arc<dyn NostrSigner>> {
        if !self.client.has_signer().await {
            return Err(NostrRelayError::MissingSigner);
        }

        Ok(self.client.signer().await?)
    }

    #[instrument(skip_all, level = "debug", ret)]
    pub async fn get_relays(&self) -> HashMap<RelayUrl, Relay> {
        self.client.relays().await
    }

    /// Publish a signed event to connected relays and return its `EventId`.
    ///
    /// # Errors
    ///
    /// Returns `NostrRelayError::MissingSigner` if no signer is configured,
    /// or an error if sending the event to the underlying client fails.
    #[instrument(skip_all, level = "debug", ret)]
    pub async fn publish_event(&self, event: &Event) -> crate::error::Result<EventId> {
        if !self.client.has_signer().await {
            return Err(NostrRelayError::MissingSigner);
        }

        let event_id = self.client.send_event(event).await?;
        let event_id = Self::handle_relay_output(event_id)?;

        Ok(event_id)
    }

    /// Subscribe to events matching the given filter.
    ///
    /// # Errors
    ///
    /// Returns an error if subscribing via the underlying client fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn subscribe(
        &self,
        filter: Filter,
        opts: Option<SubscribeAutoCloseOptions>,
    ) -> crate::error::Result<SubscriptionId> {
        Ok(self.client.subscribe(filter, opts).await?.val)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn unsubscribe(&self, subscription_id: &SubscriptionId) {
        self.client.unsubscribe(subscription_id).await;
    }

    /// Disconnect from all configured relays.
    ///
    /// # Errors
    ///
    /// Currently does not report errors and always returns `Ok(())`.
    #[instrument(skip_all, level = "debug", ret)]
    pub async fn disconnect(&self) -> crate::error::Result<()> {
        self.client.disconnect().await;
        Ok(())
    }

    /// Handle the output from a relay operation.
    ///
    /// # Errors
    ///
    /// Currently does not report errors and always returns `Ok(output.val)`.
    /// This may change if error handling for relay outputs is extended.
    /// TODO: handle error
    #[instrument(level = "debug")]
    fn handle_relay_output<T: Debug>(output: Output<T>) -> crate::error::Result<T> {
        tracing::debug!(output = ?output, "Handling Relay output.");

        Ok(output.val)
    }
}
