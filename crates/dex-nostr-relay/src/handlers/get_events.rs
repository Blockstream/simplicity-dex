pub mod ids {
    use crate::relay_client::RelayClient;
    use nostr::{EventId, Filter};
    use nostr_sdk::prelude::Events;
    use std::collections::{BTreeMap, BTreeSet};

    pub async fn handle(client: &RelayClient, event_id: EventId) -> crate::error::Result<Events> {
        let events = client
            .req_and_wait(Filter {
                ids: Some(BTreeSet::from([event_id])),
                authors: None,
                kinds: None,
                search: None,
                since: None,
                until: None,
                limit: None,
                generic_tags: BTreeMap::default(),
            })
            .await?;
        Ok(events)
    }
}

pub mod order {
    use crate::handlers::common::{filter_maker_order_events, sort_maker_order_events_by_time};
    use crate::relay_client::RelayClient;
    use crate::types::MakerOrderEvent;
    use nostr::{EventId, Filter};
    use std::collections::{BTreeMap, BTreeSet};

    pub async fn handle(client: &RelayClient, event_id: EventId) -> crate::error::Result<Vec<MakerOrderEvent>> {
        let events = client
            .req_and_wait(Filter {
                ids: Some(BTreeSet::from([event_id])),
                authors: None,
                kinds: None,
                search: None,
                since: None,
                until: None,
                limit: None,
                generic_tags: BTreeMap::default(),
            })
            .await?;
        let events = filter_maker_order_events(&events);
        let events = sort_maker_order_events_by_time(events);
        Ok(events)
    }
}
