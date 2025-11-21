use crate::types::{MakerOrderEvent, OrderReplyEvent};
use nostr_sdk::prelude::Events;

pub fn filter_maker_order_events(events_to_filter: Events) -> Vec<MakerOrderEvent> {
    events_to_filter
        .into_iter()
        .filter_map(MakerOrderEvent::parse_event)
        .collect()
}

pub fn filter_order_reply_events(events_to_filter: Events) -> Vec<OrderReplyEvent> {
    events_to_filter
        .into_iter()
        .filter_map(OrderReplyEvent::parse_event)
        .collect()
}
