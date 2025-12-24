use crate::config::RelayConfig;
use crate::error::{ParseError, RelayError};
use crate::events::{ActionCompletedEvent, OptionCreatedEvent, SwapCreatedEvent};

use std::sync::Arc;

use nostr::prelude::*;
use nostr_sdk::prelude::Events;
use simplicityhl::elements::AddressParams;
use tracing::instrument;

use super::ReadOnlyClient;

#[derive(Debug, Clone)]
pub struct PublishingClient {
    reader: ReadOnlyClient,
}

impl PublishingClient {
    #[instrument(skip_all, level = "debug", err)]
    pub async fn connect(config: RelayConfig, signer: impl IntoNostrSigner) -> Result<Self, RelayError> {
        let mut reader = ReadOnlyClient::connect(config).await?;

        reader.set_signer(signer).await;

        Ok(Self { reader })
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn signer(&self) -> Result<Arc<dyn NostrSigner>, RelayError> {
        Ok(self.reader.inner_client().signer().await?)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn public_key(&self) -> Result<PublicKey, RelayError> {
        Ok(self.reader.inner_client().signer().await?.get_public_key().await?)
    }

    #[instrument(skip(self, event), level = "debug")]
    pub async fn publish_event(&self, event: &Event) -> Result<EventId, RelayError> {
        tracing::debug!(event_id = %event.id, "Publishing event to all relays");

        let output = self.reader.inner_client().send_event(event).await?;

        tracing::debug!(
            event_id = %output.val,
            success_count = output.success.len(),
            failed_count = output.failed.len(),
            "Event published"
        );

        Ok(output.val)
    }

    #[instrument(skip(self, builder), level = "debug")]
    pub async fn publish(&self, builder: EventBuilder) -> Result<EventId, RelayError> {
        tracing::debug!("Building and publishing event");

        let output = self.reader.inner_client().send_event_builder(builder).await?;

        tracing::debug!(
            event_id = %output.val,
            success_count = output.success.len(),
            failed_count = output.failed.len(),
            "Event published"
        );

        Ok(output.val)
    }

    pub async fn publish_option_created(&self, event: &OptionCreatedEvent) -> Result<EventId, RelayError> {
        let pubkey = self.public_key().await?;
        let builder = event.to_event_builder(pubkey)?;
        self.publish(builder).await
    }

    pub async fn publish_swap_created(&self, event: &SwapCreatedEvent) -> Result<EventId, RelayError> {
        let pubkey = self.public_key().await?;
        let builder = event.to_event_builder(pubkey)?;
        self.publish(builder).await
    }

    pub async fn publish_action_completed(&self, event: &ActionCompletedEvent) -> Result<EventId, RelayError> {
        let pubkey = self.public_key().await?;
        let builder = event.to_event_builder(pubkey);
        self.publish(builder).await
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn fetch_events(&self, filter: Filter) -> Result<Events, RelayError> {
        self.reader.fetch_events(filter).await
    }

    pub async fn fetch_options(
        &self,
        params: &'static AddressParams,
    ) -> Result<Vec<Result<OptionCreatedEvent, ParseError>>, RelayError> {
        self.reader.fetch_options(params).await
    }

    pub async fn fetch_swaps(
        &self,
        params: &'static AddressParams,
    ) -> Result<Vec<Result<SwapCreatedEvent, ParseError>>, RelayError> {
        self.reader.fetch_swaps(params).await
    }

    pub async fn fetch_actions_for_event(
        &self,
        original_event_id: EventId,
    ) -> Result<Vec<Result<ActionCompletedEvent, ParseError>>, RelayError> {
        self.reader.fetch_actions_for_event(original_event_id).await
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn subscribe(&self, filter: Filter) -> Result<SubscriptionId, RelayError> {
        self.reader.subscribe(filter).await
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn unsubscribe(&self, subscription_id: &SubscriptionId) {
        self.reader.unsubscribe(subscription_id).await;
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn disconnect(&self) {
        self.reader.disconnect().await;
    }

    #[must_use]
    pub const fn config(&self) -> &RelayConfig {
        self.reader.config()
    }

    #[must_use]
    pub const fn as_reader(&self) -> &ReadOnlyClient {
        &self.reader
    }
}
