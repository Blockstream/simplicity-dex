use nostr::Filter;

use crate::events::kinds::{ACTION_COMPLETED, OPTION_CREATED, SWAP_CREATED};

#[must_use]
pub fn option_created() -> Filter {
    Filter::new().kind(OPTION_CREATED)
}

#[must_use]
pub fn option_created_by_pubkey(pubkey: nostr::PublicKey) -> Filter {
    Filter::new().kind(OPTION_CREATED).author(pubkey)
}

#[must_use]
pub fn swap_created() -> Filter {
    Filter::new().kind(SWAP_CREATED)
}

#[must_use]
pub fn swap_created_by_pubkey(pubkey: nostr::PublicKey) -> Filter {
    Filter::new().kind(SWAP_CREATED).author(pubkey)
}

#[must_use]
pub fn action_completed() -> Filter {
    Filter::new().kind(ACTION_COMPLETED)
}

#[must_use]
pub fn action_completed_for_event(original_event_id: nostr::EventId) -> Filter {
    Filter::new().kind(ACTION_COMPLETED).event(original_event_id)
}

#[must_use]
pub fn all_option_events() -> Filter {
    Filter::new().kinds([OPTION_CREATED, SWAP_CREATED, ACTION_COMPLETED])
}
