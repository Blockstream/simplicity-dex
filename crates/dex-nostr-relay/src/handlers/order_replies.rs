use crate::relay_client::RelayClient;
use crate::types::{CustomKind, TakerOrderKind};

use std::collections::{BTreeMap, BTreeSet};

use nostr::{EventId, Filter, SingleLetterTag};
use nostr_sdk::prelude::Events;

pub async fn handle(client: &RelayClient, event_id: EventId) -> crate::error::Result<Events> {
    let events = client
        .req_and_wait(Filter {
            ids: None,
            authors: None,
            kinds: Some(BTreeSet::from([TakerOrderKind::get_kind()])),
            search: None,
            since: None,
            until: None,
            limit: None,
            generic_tags: BTreeMap::from([(SingleLetterTag::from_char('e')?, BTreeSet::from([event_id.to_string()]))]),
        })
        .await?;
    Ok(events)
}
