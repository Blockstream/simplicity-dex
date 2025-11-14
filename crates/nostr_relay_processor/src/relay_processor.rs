use crate::handlers;
use crate::types::{MakerOrderEvent, MakerOrderSummary};
use nostr::prelude::IntoNostrSigner;
use nostr::{EventId, PublicKey, TryIntoUrl};
use nostr_relay_connector::relay_client::{ClientConfig, RelayClient};
use nostr_sdk::prelude::Events;
use simplicity_contracts::DCDArguments;
use simplicityhl::elements::{AssetId, Txid};

pub struct RelayProcessor {
    relay_client: RelayClient,
}

#[derive(Debug, Clone, Default)]
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

    pub async fn place_order(&self, tags: OrderPlaceEventTags, tx_id: Txid) -> crate::error::Result<EventId> {
        let event_id = handlers::place_order::handle(&self.relay_client, tags, tx_id).await?;
        Ok(event_id)
    }

    pub async fn list_orders(&self) -> crate::error::Result<Vec<MakerOrderSummary>> {
        let events = handlers::list_orders::handle(&self.relay_client).await?;
        Ok(events)
    }

    pub async fn reply_order(
        &self,
        maker_event_id: EventId,
        maker_pubkey: PublicKey,
        tags: OrderReplyEventTags,
    ) -> crate::error::Result<EventId> {
        let event_id = handlers::reply_order::handle(&self.relay_client, maker_event_id, maker_pubkey, tags).await?;
        Ok(event_id)
    }

    pub async fn get_order_replies(&self, event_id: EventId) -> crate::error::Result<Events> {
        let events = handlers::order_replies::handle(&self.relay_client, event_id).await?;
        Ok(events)
    }

    pub async fn get_order_by_id(&self, event_id: EventId) -> crate::error::Result<Vec<MakerOrderEvent>> {
        let events = handlers::get_events::order::handle(&self.relay_client, event_id).await?;
        Ok(events)
    }

    pub async fn get_event_by_id(&self, event_id: EventId) -> crate::error::Result<Events> {
        let events = handlers::get_events::ids::handle(&self.relay_client, event_id).await?;
        Ok(events)
    }
}
