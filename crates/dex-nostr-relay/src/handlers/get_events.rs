pub mod ids {
    use crate::relay_client::RelayClient;

    use std::collections::{BTreeMap, BTreeSet};

    use nostr::{EventId, Filter};
    use nostr_sdk::prelude::Events;

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
