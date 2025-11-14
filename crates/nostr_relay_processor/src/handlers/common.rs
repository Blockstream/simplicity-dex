use crate::types::MakerOrderEvent;
use nostr_sdk::prelude::Events;

pub fn filter_events(events_to_filter: Events) -> Vec<MakerOrderEvent> {
    events_to_filter
        .into_iter()
        .map(MakerOrderEvent::parse_event)
        .filter(|x| x.is_some())
        .map(|x| x.unwrap())
        .collect()
}
