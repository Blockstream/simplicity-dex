use crate::relay_client::RelayClient;
use crate::relay_processor::OrderReplyEventTags;
use crate::types::{BLOCKSTREAM_TAKER_CONTENT, CustomKind, TakerOrderKind};

use std::borrow::Cow;

use nostr::{EventBuilder, EventId, NostrSigner, PublicKey, Tag, TagKind, Timestamp};

pub async fn handle(
    client: &RelayClient,
    maker_event_id: EventId,
    maker_pubkey: PublicKey,
    tags: OrderReplyEventTags,
) -> crate::error::Result<EventId> {
    let client_signer = client.get_signer().await?;
    let client_pubkey = client_signer.get_public_key().await?;

    let timestamp_now = Timestamp::now();

    let taker_response = EventBuilder::new(TakerOrderKind::get_kind(), BLOCKSTREAM_TAKER_CONTENT)
        .tags([
            Tag::public_key(client_pubkey),
            Tag::event(maker_event_id),
            Tag::custom(TagKind::Custom(Cow::from("maker_pubkey")), [maker_pubkey]),
            Tag::custom(TagKind::Custom(Cow::from("tx_id")), [tags.tx_id]),
        ])
        .custom_created_at(timestamp_now);

    let reply_event = taker_response.build(client_pubkey);
    let reply_event = client_signer.sign_event(reply_event).await?;

    let event_id = client.publish_event(&reply_event).await?;

    Ok(event_id)
}
