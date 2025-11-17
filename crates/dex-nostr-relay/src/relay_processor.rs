use crate::handlers;
use crate::relay_client::{ClientConfig, RelayClient};

use nostr::prelude::IntoNostrSigner;
use nostr::{EventId, PublicKey, TryIntoUrl};

use nostr_sdk::prelude::Events;

pub struct RelayProcessor {
    relay_client: RelayClient,
}

#[derive(Debug, Default, Clone)]
pub struct OrderPlaceEventTags {
    pub asset_to_sell: String,
    pub asset_to_buy: String,
    pub price: u64,
    pub expiry: u64,
    pub compiler_name: String,
    pub compiler_build_hash: String,
}

#[derive(Debug, Default, Clone)]
pub struct OrderReplyEventTags {
    pub tx_id: String,
}

impl RelayProcessor {
    pub async fn try_from_config(
        relay_urls: impl IntoIterator<Item = impl TryIntoUrl>,
        keys: Option<impl IntoNostrSigner>,
        client_config: ClientConfig,
    ) -> crate::error::Result<Self> {
        Ok(RelayProcessor {
            relay_client: RelayClient::connect(relay_urls, keys, client_config).await?,
        })
    }

    pub async fn place_order(&self, tags: OrderPlaceEventTags) -> crate::error::Result<EventId> {
        handlers::place_order::handle(&self.relay_client, tags).await
    }

    pub async fn list_orders(&self) -> crate::error::Result<Events> {
        handlers::list_orders::handle(&self.relay_client).await
    }

    pub async fn reply_order(
        &self,
        maker_event_id: EventId,
        maker_pubkey: PublicKey,
        tags: OrderReplyEventTags,
    ) -> crate::error::Result<EventId> {
        handlers::reply_order::handle(&self.relay_client, maker_event_id, maker_pubkey, tags).await
    }

    pub async fn get_order_replies(&self, event_id: EventId) -> crate::error::Result<Events> {
        handlers::order_replies::handle(&self.relay_client, event_id).await
    }

    pub async fn get_events_by_id(&self, event_id: EventId) -> crate::error::Result<Events> {
        handlers::get_events::ids::handle(&self.relay_client, event_id).await
    }
}
