use crate::types::{MakerOrderEvent, OrderReplyEvent};
use chrono::{DateTime, TimeZone, Utc};
use nostr::Timestamp;
use nostr_sdk::prelude::Events;

pub fn filter_maker_order_events(events_to_filter: &Events) -> Vec<MakerOrderEvent> {
    events_to_filter
        .iter()
        .filter_map(MakerOrderEvent::parse_event)
        .collect()
}

pub fn filter_order_reply_events(events_to_filter: &Events) -> Vec<OrderReplyEvent> {
    events_to_filter
        .iter()
        .filter_map(OrderReplyEvent::parse_event)
        .collect()
}

pub fn timestamp_to_chrono_utc(time: Timestamp) -> Option<DateTime<Utc>> {
    chrono::Utc
        .timestamp_opt(i64::try_from(time.as_u64()).ok()?, 0)
        .single()
}
