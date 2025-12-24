use crate::config::RelayConfig;
use crate::error::{ParseError, RelayError};
use crate::events::{ActionCompletedEvent, OptionCreatedEvent, SwapCreatedEvent, filters};

use nostr::prelude::*;
use nostr_sdk::Client;
use nostr_sdk::prelude::Events;
use simplicityhl::elements::AddressParams;
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct ReadOnlyClient {
    client: Client,
    config: RelayConfig,
}

impl ReadOnlyClient {
    #[instrument(skip_all, level = "debug", err)]
    pub async fn connect(config: RelayConfig) -> Result<Self, RelayError> {
        tracing::debug!(
            primary = %config.primary_relay(),
            backup_count = config.all_relays().len() - 1,
            "Connecting to NOSTR relays"
        );

        let client = Client::default();

        for url in config.all_relays() {
            let relay_url = Url::parse(url)?;

            client.add_relay(relay_url).await?;
        }

        client.connect().await;

        Ok(Self { client, config })
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn fetch_events(&self, filter: Filter) -> Result<Events, RelayError> {
        tracing::debug!(?filter, "Fetching events");

        Ok(self.client.fetch_combined_events(filter, self.config.timeout()).await?)
    }

    pub async fn fetch_options(
        &self,
        params: &'static AddressParams,
    ) -> Result<Vec<Result<OptionCreatedEvent, ParseError>>, RelayError> {
        let events = self.fetch_events(filters::option_created()).await?;
        Ok(events
            .iter()
            .map(|e| OptionCreatedEvent::from_event(e, params))
            .collect())
    }

    pub async fn fetch_swaps(
        &self,
        params: &'static AddressParams,
    ) -> Result<Vec<Result<SwapCreatedEvent, ParseError>>, RelayError> {
        let events = self.fetch_events(filters::swap_created()).await?;
        Ok(events.iter().map(|e| SwapCreatedEvent::from_event(e, params)).collect())
    }

    pub async fn fetch_actions_for_event(
        &self,
        original_event_id: EventId,
    ) -> Result<Vec<Result<ActionCompletedEvent, ParseError>>, RelayError> {
        let events = self
            .fetch_events(filters::action_completed_for_event(original_event_id))
            .await?;
        Ok(events.iter().map(ActionCompletedEvent::from_event).collect())
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn subscribe(&self, filter: Filter) -> Result<SubscriptionId, RelayError> {
        tracing::debug!(?filter, "Subscribing to events");

        Ok(self.client.subscribe(filter, None).await?.val)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn unsubscribe(&self, subscription_id: &SubscriptionId) {
        tracing::debug!(%subscription_id, "Unsubscribing");

        self.client.unsubscribe(subscription_id).await;
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn disconnect(&self) {
        tracing::debug!("Disconnecting from all relays");

        self.client.disconnect().await;
    }

    #[must_use]
    pub const fn config(&self) -> &RelayConfig {
        &self.config
    }

    pub(crate) const fn inner_client(&self) -> &Client {
        &self.client
    }

    pub(crate) async fn set_signer(&mut self, signer: impl IntoNostrSigner) {
        self.client.automatic_authentication(true);

        self.client.set_signer(signer).await;
    }
}
