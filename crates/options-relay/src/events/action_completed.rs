use crate::error::ParseError;
use crate::events::kinds::{
    ACTION_COMPLETED, ACTION_OPTION_CANCELLED, ACTION_OPTION_CREATED, ACTION_OPTION_EXERCISED, ACTION_OPTION_EXPIRED,
    ACTION_OPTION_FUNDED, ACTION_SETTLEMENT_CLAIMED, ACTION_SWAP_CANCELLED, ACTION_SWAP_CREATED, ACTION_SWAP_EXERCISED,
    TAG_ACTION, TAG_OUTPOINT,
};

use std::str::FromStr;

use nostr::{Event, EventBuilder, EventId, PublicKey, Tag, TagKind, Timestamp};
use simplicityhl::elements::OutPoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    OptionCreated,
    OptionFunded,
    SwapCreated,
    SwapExercised,
    SwapCancelled,
    OptionExercised,
    OptionCancelled,
    SettlementClaimed,
    OptionExpired,
}

impl ActionType {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::OptionCreated => ACTION_OPTION_CREATED,
            Self::OptionFunded => ACTION_OPTION_FUNDED,
            Self::SwapCreated => ACTION_SWAP_CREATED,
            Self::SwapExercised => ACTION_SWAP_EXERCISED,
            Self::SwapCancelled => ACTION_SWAP_CANCELLED,
            Self::OptionExercised => ACTION_OPTION_EXERCISED,
            Self::OptionCancelled => ACTION_OPTION_CANCELLED,
            Self::SettlementClaimed => ACTION_SETTLEMENT_CLAIMED,
            Self::OptionExpired => ACTION_OPTION_EXPIRED,
        }
    }
}

impl FromStr for ActionType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ACTION_OPTION_CREATED => Ok(Self::OptionCreated),
            ACTION_OPTION_FUNDED => Ok(Self::OptionFunded),
            ACTION_SWAP_CREATED => Ok(Self::SwapCreated),
            ACTION_SWAP_EXERCISED => Ok(Self::SwapExercised),
            ACTION_SWAP_CANCELLED => Ok(Self::SwapCancelled),
            ACTION_OPTION_EXERCISED => Ok(Self::OptionExercised),
            ACTION_OPTION_CANCELLED => Ok(Self::OptionCancelled),
            ACTION_SETTLEMENT_CLAIMED => Ok(Self::SettlementClaimed),
            ACTION_OPTION_EXPIRED => Ok(Self::OptionExpired),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActionCompletedEvent {
    pub event_id: EventId,
    pub pubkey: PublicKey,
    pub created_at: Timestamp,
    pub original_event_id: EventId,
    pub action: ActionType,
    pub outpoint: OutPoint,
}

impl ActionCompletedEvent {
    #[must_use]
    pub fn new(original_event_id: EventId, action: ActionType, outpoint: OutPoint) -> Self {
        Self {
            event_id: EventId::all_zeros(),
            pubkey: PublicKey::from_slice(&[1; 32]).unwrap(),
            created_at: Timestamp::now(),
            original_event_id,
            action,
            outpoint,
        }
    }

    #[must_use]
    pub fn to_event_builder(&self, creator_pubkey: PublicKey) -> EventBuilder {
        EventBuilder::new(ACTION_COMPLETED, "")
            .tag(Tag::public_key(creator_pubkey))
            .tag(Tag::event(self.original_event_id))
            .tag(Tag::custom(TagKind::custom(TAG_ACTION), [self.action.as_str()]))
            .tag(Tag::custom(TagKind::custom(TAG_OUTPOINT), [self.outpoint.to_string()]))
    }

    pub fn from_event(event: &Event) -> Result<Self, ParseError> {
        event.verify()?;

        if event.kind != ACTION_COMPLETED {
            return Err(ParseError::InvalidKind);
        }

        let original_event_id = event
            .tags
            .iter()
            .find(|t| t.kind() == TagKind::e())
            .and_then(|t| t.content())
            .and_then(|s| EventId::from_hex(s).ok())
            .ok_or(ParseError::MissingTag("e"))?;

        let action_str = event
            .tags
            .iter()
            .find(|t| matches!(t.kind(), TagKind::Custom(s) if s.as_ref() == TAG_ACTION))
            .and_then(|t| t.content())
            .ok_or(ParseError::MissingTag(TAG_ACTION))?;

        let action: ActionType = action_str.parse().map_err(|()| ParseError::InvalidAction)?;

        let outpoint_str = event
            .tags
            .iter()
            .find(|t| matches!(t.kind(), TagKind::Custom(s) if s.as_ref() == TAG_OUTPOINT))
            .and_then(|t| t.content())
            .ok_or(ParseError::MissingTag(TAG_OUTPOINT))?;

        let outpoint: OutPoint = outpoint_str.parse()?;

        Ok(Self {
            event_id: event.id,
            pubkey: event.pubkey,
            created_at: event.created_at,
            original_event_id,
            action,
            outpoint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{Keys, hashes::Hash};
    use simplicityhl::elements::Txid;

    fn dummy_outpoint() -> OutPoint {
        OutPoint::new(Txid::all_zeros(), 0)
    }

    #[test]
    fn action_type_roundtrip() {
        let actions = [
            ActionType::OptionCreated,
            ActionType::OptionFunded,
            ActionType::SwapCreated,
            ActionType::SwapExercised,
            ActionType::SwapCancelled,
            ActionType::OptionExercised,
            ActionType::OptionCancelled,
            ActionType::SettlementClaimed,
            ActionType::OptionExpired,
        ];

        for action in actions {
            let s = action.as_str();
            let parsed: ActionType = s.parse().expect("should parse");
            assert_eq!(action, parsed);
        }
    }

    #[test]
    fn action_completed_event_builder_roundtrip() -> anyhow::Result<()> {
        let keys = Keys::generate();
        let original_event_id = EventId::all_zeros();

        let event = ActionCompletedEvent::new(original_event_id, ActionType::OptionExercised, dummy_outpoint());

        let builder = event.to_event_builder(keys.public_key());
        let built_event = builder.sign_with_keys(&keys)?;

        let parsed = ActionCompletedEvent::from_event(&built_event)?;

        assert_eq!(parsed.original_event_id, original_event_id);
        assert_eq!(parsed.action, ActionType::OptionExercised);
        assert_eq!(parsed.outpoint, dummy_outpoint());

        Ok(())
    }
}
