use crate::handlers::common::filter_maker_order_events;
use crate::relay_client::RelayClient;
use crate::relay_processor::ListOrdersEventFilter;
use crate::types::{MakerOrderEvent, MakerOrderSummary};
use nostr::Timestamp;
use nostr_sdk::prelude::Events;

pub async fn handle(
    client: &RelayClient,
    filter: ListOrdersEventFilter,
) -> crate::error::Result<Vec<MakerOrderSummary>> {
    let events = client.req_and_wait(filter.to_filter()).await?;
    let events = filter_expired_events(events);
    let events = filter_maker_order_events(&events);
    let events = events.iter().map(MakerOrderEvent::summary).collect();
    Ok(events)
}

#[inline]
fn filter_expired_events(events_to_filter: Events) -> Events {
    let time_now = Timestamp::now();
    events_to_filter
        .into_iter()
        .filter(|x| match x.tags.expiration() {
            None => true,
            Some(t) => t.as_u64() > time_now.as_u64(),
        })
        .collect()
}
