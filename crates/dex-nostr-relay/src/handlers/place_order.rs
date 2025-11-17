use crate::relay_client::RelayClient;
use crate::relay_processor::OrderPlaceEventTags;
use crate::types::{BLOCKSTREAM_MAKER_CONTENT, CustomKind, MAKER_EXPIRATION_TIME, MakerOrderKind};

use std::borrow::Cow;

use nostr::{EventBuilder, EventId, Tag, TagKind, Timestamp};

pub async fn handle(client: &RelayClient, tags: OrderPlaceEventTags) -> crate::error::Result<EventId> {
    let client_signer = client.get_signer().await?;
    let client_pubkey = client_signer.get_public_key().await?;

    let timestamp_now = Timestamp::now();

    let maker_order = EventBuilder::new(MakerOrderKind::get_kind(), BLOCKSTREAM_MAKER_CONTENT)
        .tags([
            Tag::public_key(client_pubkey),
            Tag::expiration(Timestamp::from(timestamp_now.as_u64() + MAKER_EXPIRATION_TIME)),
            Tag::custom(
                TagKind::Custom(Cow::from("compiler")),
                [tags.compiler_name, tags.compiler_build_hash],
            ),
            Tag::custom(TagKind::Custom(Cow::from("asset_to_buy")), [tags.asset_to_buy]),
            Tag::custom(TagKind::Custom(Cow::from("asset_to_sell")), [tags.asset_to_sell]),
            Tag::custom(TagKind::Custom(Cow::from("price")), [tags.price.to_string()]),
        ])
        .custom_created_at(timestamp_now);

    let text_note = maker_order.build(client_pubkey);
    let signed_event = client_signer.sign_event(text_note).await?;

    let maker_order_event_id = client.publish_event(&signed_event).await?;

    Ok(maker_order_event_id)
}
