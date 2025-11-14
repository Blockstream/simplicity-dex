use crate::handlers::common::filter_events;
use crate::types::{CustomKind, MakerOrderEvent, MakerOrderKind, MakerOrderSummary};
use nostr::{Filter, Timestamp};
use nostr_relay_connector::relay_client::RelayClient;
use nostr_sdk::prelude::Events;
use std::collections::{BTreeMap, BTreeSet};

pub async fn handle(client: &RelayClient) -> crate::error::Result<Vec<MakerOrderSummary>> {
    let events = client
        .req_and_wait(Filter {
            ids: None,
            authors: None,
            kinds: Some(BTreeSet::from([MakerOrderKind::get_kind()])),
            search: None,
            since: None,
            until: None,
            limit: None,
            generic_tags: BTreeMap::default(),
        })
        .await?;
    let events = filter_expired_events(events);
    let events = filter_events(events);
    let events = events.iter().map(MakerOrderEvent::summary).collect();
    Ok(events)
}

#[inline]
fn filter_expired_events(events_to_filter: Events) -> Events {
    let time_now = Timestamp::now();
    events_to_filter
        .into_iter()
        .filter(|x| match x.tags.expiration() {
            None => false,
            Some(t) => t.as_u64() > time_now.as_u64(),
        })
        .collect()
}
