use crate::handlers;
use crate::relay_client::{ClientConfig, RelayClient};
use crate::types::{CustomKind, MakerOrderEvent, MakerOrderSummary, OrderReplyEvent, ReplyOption};
use contracts::DCDArguments;
use nostr::prelude::IntoNostrSigner;
use nostr::{EventId, PublicKey, TryIntoUrl};
use nostr_sdk::prelude::Events;
use simplicityhl::elements::{AssetId, Txid};

use nostr::{Filter, Timestamp};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct RelayProcessor {
    relay_client: RelayClient,
}

#[derive(Debug, Default, Clone)]
pub struct OrderPlaceEventTags {
    pub dcd_arguments: DCDArguments,
    pub dcd_taproot_pubkey_gen: String,
    pub filler_asset_id: AssetId,
    pub grantor_collateral_asset_id: AssetId,
    pub grantor_settlement_asset_id: AssetId,
    pub settlement_asset_id: AssetId,
    pub collateral_asset_id: AssetId,
}

#[derive(Debug, Default, Clone)]
pub struct ListOrdersEventFilter {
    pub authors: Option<Vec<PublicKey>>,
    pub since: Option<Timestamp>,
    pub until: Option<Timestamp>,
    pub limit: Option<usize>,
}

impl ListOrdersEventFilter {
    #[must_use]
    pub fn to_filter(&self) -> Filter {
        let authors_set = if let Some(list) = &self.authors {
            let mut set = BTreeSet::new();
            for pk in list {
                set.insert(*pk);
            }
            if set.is_empty() { None } else { Some(set) }
        } else {
            None
        };

        Filter {
            ids: None,
            authors: authors_set,
            kinds: Some(BTreeSet::from([crate::types::MakerOrderKind::get_kind()])),
            search: None,
            since: self.since,
            until: self.until,
            limit: self.limit,
            generic_tags: BTreeMap::default(),
        }
    }
}

impl RelayProcessor {
    /// Create a [`RelayProcessor`] from relay URLs, optional keys, and client configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if connecting to any of the provided relays or
    /// configuring the underlying [`RelayClient`] fails.
    pub async fn try_from_config(
        relay_urls: impl IntoIterator<Item = impl TryIntoUrl>,
        keys: Option<impl IntoNostrSigner>,
        client_config: ClientConfig,
    ) -> crate::error::Result<Self> {
        Ok(RelayProcessor {
            relay_client: RelayClient::connect(relay_urls, keys, client_config).await?,
        })
    }

    /// Place a new maker order event on the relay network.
    ///
    /// # Errors
    ///
    /// Returns an error if constructing or publishing the order event fails,
    /// or if the relay client encounters an error while sending the event.
    pub async fn place_order(&self, tags: OrderPlaceEventTags, tx_id: Txid) -> crate::error::Result<EventId> {
        let event_id = handlers::place_order::handle(&self.relay_client, tags, tx_id).await?;
        Ok(event_id)
    }

    /// List maker orders matching the provided filter.
    ///
    /// # Errors
    ///
    /// Returns an error if querying relays for matching maker order events
    /// fails or if parsing retrieved events into [`MakerOrderSummary`] fails.
    pub async fn list_orders(&self, filter: ListOrdersEventFilter) -> crate::error::Result<Vec<MakerOrderSummary>> {
        let events = handlers::list_orders::handle(&self.relay_client, filter).await?;
        Ok(events)
    }

    /// Send a reply to an order event with the given reply option.
    ///
    /// # Errors
    ///
    /// Returns an error if building, signing, or publishing the reply event
    /// fails, or if the relay client fails to send the event.
    pub async fn reply_order(&self, event_source: EventId, reply_option: ReplyOption) -> crate::error::Result<EventId> {
        let event_id = handlers::reply_order::handle(&self.relay_client, event_source, reply_option).await?;
        Ok(event_id)
    }

    /// Fetch reply events for a given order event.
    ///
    /// # Errors
    ///
    /// Returns an error if querying relays for reply events fails or if
    /// parsing the retrieved events into [`OrderReplyEvent`] fails.
    pub async fn get_order_replies(&self, event_id: EventId) -> crate::error::Result<Vec<OrderReplyEvent>> {
        let events = handlers::order_replies::handle(&self.relay_client, event_id).await?;
        Ok(events)
    }

    /// Fetch a single maker order event by its event ID.
    ///
    /// # Errors
    ///
    /// Returns:
    /// - `NostrRelayError::NoEventsFound` if no event with the given ID is found.
    /// - `NostrRelayError::NotOnlyOneEventFound` if more than one matching event is found.
    /// - Any error produced by querying relays for the event.
    pub async fn get_order_by_id(&self, event_id: EventId) -> crate::error::Result<MakerOrderEvent> {
        let mut events = handlers::get_events::order::handle(&self.relay_client, event_id).await?;
        if events.is_empty() {
            return Err(crate::error::NostrRelayError::NoEventsFound(format!(
                "event_id: {event_id}"
            )));
        } else if events.len() > 1 {
            return Err(crate::error::NostrRelayError::NotOnlyOneEventFound(format!(
                "event_id: {event_id}"
            )));
        }
        Ok(events.remove(0))
    }

    /// Fetch raw events with the given event ID.
    ///
    /// # Errors
    ///
    /// Returns an error if querying relays for the event fails or if the
    /// underlying relay client encounters an error while fetching events.
    pub async fn get_event_by_id(&self, event_id: EventId) -> crate::error::Result<Events> {
        let events = handlers::get_events::ids::handle(&self.relay_client, event_id).await?;
        Ok(events)
    }
}
