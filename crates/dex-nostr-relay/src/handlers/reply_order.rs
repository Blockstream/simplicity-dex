use crate::relay_client::RelayClient;
use crate::types::ReplyOption;

use nostr::{EventBuilder, EventId, Timestamp};

pub async fn handle(
    client: &RelayClient,
    source_event_id: EventId,
    reply_option: ReplyOption,
) -> crate::error::Result<EventId> {
    let client_signer = client.get_signer().await?;
    let client_pubkey = client_signer.get_public_key().await?;
    let timestamp_now = Timestamp::now();

    // Build tags based on reply option variant
    let tags = reply_option.form_tags(source_event_id, client_pubkey);

    let reply_event_builder = EventBuilder::new(reply_option.get_kind(), reply_option.get_content())
        .tags(tags)
        .custom_created_at(timestamp_now);

    let reply_event = reply_event_builder.build(client_pubkey);
    let reply_event = client_signer.sign_event(reply_event).await?;

    let event_id = client.publish_event(&reply_event).await?;

    Ok(event_id)
}
