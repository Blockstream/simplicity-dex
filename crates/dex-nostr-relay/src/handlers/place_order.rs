use crate::relay_client::RelayClient;
use crate::relay_processor::OrderPlaceEventTags;
use crate::types::{BLOCKSTREAM_MAKER_CONTENT, CustomKind, MakerOrderEvent, MakerOrderKind};
use nostr::{EventBuilder, EventId, Timestamp};
use simplicity::elements::Txid;

pub async fn handle(client: &RelayClient, tags: OrderPlaceEventTags, tx_id: Txid) -> crate::error::Result<EventId> {
    let client_signer = client.get_signer().await?;
    let client_pubkey = client_signer.get_public_key().await?;

    let timestamp_now = Timestamp::now();

    let tags = MakerOrderEvent::form_tags(tags, tx_id, client_pubkey)?;
    let maker_order = EventBuilder::new(MakerOrderKind::get_kind(), BLOCKSTREAM_MAKER_CONTENT)
        .tags(tags)
        .custom_created_at(timestamp_now);

    let text_note = maker_order.build(client_pubkey);
    let signed_event = client_signer.sign_event(text_note).await?;

    let maker_order_event_id = client.publish_event(&signed_event).await?;

    Ok(maker_order_event_id)
}
