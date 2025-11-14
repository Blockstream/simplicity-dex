use crate::relay_processor::OrderPlaceEventTags;
use crate::types::{
    BLOCKSTREAM_MAKER_CONTENT, CustomKind, MAKER_COLLATERAL_ASSET_ID_TAG, MAKER_DCD_ARG_TAG, MAKER_DCD_TAPROOT_TAG,
    MAKER_EXPIRATION_TIME, MAKER_FILLER_ASSET_ID_TAG, MAKER_FUND_TX_ID_TAG, MAKER_GRANTOR_COLLATERAL_ASSET_ID_TAG,
    MAKER_GRANTOR_SETTLEMENT_ASSET_ID_TAG, MAKER_SETTLEMENT_ASSET_ID_TAG, MakerOrderKind,
};
use nostr::util::hex;
use nostr::{EventBuilder, EventId, Tag, TagKind, Timestamp};
use nostr_relay_connector::relay_client::RelayClient;
use simplicity::elements::Txid;
use std::borrow::Cow;

pub async fn handle(client: &RelayClient, tags: OrderPlaceEventTags, tx_id: Txid) -> crate::error::Result<EventId> {
    let client_signer = client.get_signer().await?;
    let client_pubkey = client_signer.get_public_key().await?;

    let timestamp_now = Timestamp::now();

    let dcd_arguments = {
        let x = bincode::encode_to_vec(tags.dcd_arguments, bincode::config::standard()).unwrap();
        hex::encode(x)
    };

    let maker_order = EventBuilder::new(MakerOrderKind::get_kind(), BLOCKSTREAM_MAKER_CONTENT)
        .tags([
            Tag::public_key(client_pubkey),
            Tag::expiration(Timestamp::from(timestamp_now.as_u64() + MAKER_EXPIRATION_TIME)),
            Tag::custom(TagKind::Custom(Cow::from(MAKER_DCD_ARG_TAG)), [dcd_arguments]),
            Tag::custom(
                TagKind::Custom(Cow::from(MAKER_DCD_TAPROOT_TAG)),
                [tags.dcd_taproot_pubkey_gen],
            ),
            Tag::custom(
                TagKind::Custom(Cow::from(MAKER_FILLER_ASSET_ID_TAG)),
                [tags.filler_asset_id.to_string()],
            ),
            Tag::custom(
                TagKind::Custom(Cow::from(MAKER_GRANTOR_COLLATERAL_ASSET_ID_TAG)),
                [tags.grantor_collateral_asset_id.to_string()],
            ),
            Tag::custom(
                TagKind::Custom(Cow::from(MAKER_GRANTOR_SETTLEMENT_ASSET_ID_TAG)),
                [tags.grantor_settlement_asset_id.to_string()],
            ),
            Tag::custom(
                TagKind::Custom(Cow::from(MAKER_SETTLEMENT_ASSET_ID_TAG)),
                [tags.settlement_asset_id.to_string()],
            ),
            Tag::custom(
                TagKind::Custom(Cow::from(MAKER_COLLATERAL_ASSET_ID_TAG)),
                [tags.collateral_asset_id.to_string()],
            ),
            Tag::custom(TagKind::Custom(Cow::from(MAKER_FUND_TX_ID_TAG)), [tx_id.to_string()]),
        ])
        .custom_created_at(timestamp_now);

    let text_note = maker_order.build(client_pubkey);
    let signed_event = client_signer.sign_event(text_note).await?;

    let maker_order_event_id = client.publish_event(&signed_event).await?;

    Ok(maker_order_event_id)
}
