use crate::relay_client::RelayClient;
use crate::types::{CustomKind, OrderReplyEvent, TakerReplyOrderKind};

use std::collections::{BTreeMap, BTreeSet};

use crate::handlers::common::{filter_order_reply_events, sort_order_replies_by_time};
use nostr::{EventId, Filter, SingleLetterTag};

pub async fn handle(client: &RelayClient, event_id: EventId) -> crate::error::Result<Vec<OrderReplyEvent>> {
    let events = client
        .req_and_wait(Filter {
            ids: None,
            authors: None,
            kinds: Some(BTreeSet::from([TakerReplyOrderKind::get_kind()])),
            search: None,
            since: None,
            until: None,
            limit: None,
            generic_tags: BTreeMap::from([(SingleLetterTag::from_char('e')?, BTreeSet::from([event_id.to_string()]))]),
        })
        .await?;
    let events = filter_order_reply_events(&events);
    let events = sort_order_replies_by_time(events);

    Ok(events)
}
