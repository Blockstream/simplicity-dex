use crate::error::{ParseError, RelayError};
use crate::events::kinds::{SWAP_CREATED, TAG_SWAP_ARGS, TAG_SWAP_UTXO, TAG_TAPROOT_GEN};

use contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen;
use contracts::swap_with_change::{SwapWithChangeArguments, get_swap_with_change_address};
use nostr::{Event, EventBuilder, EventId, PublicKey, Tag, TagKind, Timestamp};
use simplicityhl::elements::{AddressParams, OutPoint};
use simplicityhl_core::Encodable;

#[derive(Debug, Clone)]
pub struct SwapCreatedEvent {
    pub event_id: EventId,
    pub pubkey: PublicKey,
    pub created_at: Timestamp,
    pub swap_args: SwapWithChangeArguments,
    pub utxo: OutPoint,
    pub taproot_pubkey_gen: TaprootPubkeyGen,
}

impl SwapCreatedEvent {
    #[must_use]
    pub fn new(swap_args: SwapWithChangeArguments, utxo: OutPoint, taproot_pubkey_gen: TaprootPubkeyGen) -> Self {
        Self {
            event_id: EventId::all_zeros(),
            pubkey: PublicKey::from_slice(&[1; 32]).unwrap(),
            created_at: Timestamp::now(),
            swap_args,
            utxo,
            taproot_pubkey_gen,
        }
    }

    pub fn to_event_builder(&self, creator_pubkey: PublicKey) -> Result<EventBuilder, RelayError> {
        let args_hex = self.swap_args.to_hex()?;

        Ok(EventBuilder::new(SWAP_CREATED, "")
            .tag(Tag::public_key(creator_pubkey))
            .tag(Tag::custom(TagKind::custom(TAG_SWAP_ARGS), [args_hex]))
            .tag(Tag::custom(TagKind::custom(TAG_SWAP_UTXO), [self.utxo.to_string()]))
            .tag(Tag::custom(
                TagKind::custom(TAG_TAPROOT_GEN),
                [self.taproot_pubkey_gen.to_string()],
            )))
    }

    pub fn from_event(event: &Event, params: &'static AddressParams) -> Result<Self, ParseError> {
        event.verify()?;

        if event.kind != SWAP_CREATED {
            return Err(ParseError::InvalidKind);
        }

        let args_hex = event
            .tags
            .iter()
            .find(|t| matches!(t.kind(), TagKind::Custom(s) if s.as_ref() == TAG_SWAP_ARGS))
            .and_then(|t| t.content())
            .ok_or(ParseError::MissingTag(TAG_SWAP_ARGS))?;

        let swap_args = SwapWithChangeArguments::from_hex(args_hex)?;

        let utxo_str = event
            .tags
            .iter()
            .find(|t| matches!(t.kind(), TagKind::Custom(s) if s.as_ref() == TAG_SWAP_UTXO))
            .and_then(|t| t.content())
            .ok_or(ParseError::MissingTag(TAG_SWAP_UTXO))?;

        let utxo: OutPoint = utxo_str.parse()?;

        let taproot_str = event
            .tags
            .iter()
            .find(|t| matches!(t.kind(), TagKind::SingleLetter(l) if l.character == nostr::Alphabet::T))
            .and_then(|t| t.content())
            .ok_or(ParseError::MissingTag(TAG_TAPROOT_GEN))?;

        let taproot_pubkey_gen =
            TaprootPubkeyGen::build_from_str(taproot_str, &swap_args, params, &get_swap_with_change_address)?;

        Ok(Self {
            event_id: event.id,
            pubkey: event.pubkey,
            created_at: event.created_at,
            swap_args,
            utxo,
            taproot_pubkey_gen,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use nostr::{Keys, hashes::Hash};

    use simplicityhl::elements::{AssetId, Txid};
    use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_TEST_ASSET_ID_STR};

    fn get_mocked_data() -> anyhow::Result<(SwapWithChangeArguments, TaprootPubkeyGen)> {
        let settlement_asset_id = AssetId::from_slice(&hex::decode(LIQUID_TESTNET_TEST_ASSET_ID_STR)?)?;

        let args = SwapWithChangeArguments::new(*LIQUID_TESTNET_BITCOIN_ASSET, settlement_asset_id, 1000, 50, [1; 32]);

        let taproot_pubkey_gen =
            TaprootPubkeyGen::from(&args, &AddressParams::LIQUID_TESTNET, &get_swap_with_change_address)?;

        Ok((args, taproot_pubkey_gen))
    }

    #[test]
    fn swap_created_event_roundtrip() -> anyhow::Result<()> {
        let keys = Keys::generate();
        let (args, taproot_pubkey_gen) = get_mocked_data()?;
        let utxo = OutPoint::new(Txid::all_zeros(), 0);

        let event = SwapCreatedEvent::new(args.clone(), utxo, taproot_pubkey_gen.clone());

        let builder = event.to_event_builder(keys.public_key())?;
        let built_event = builder.sign_with_keys(&keys)?;

        let parsed = SwapCreatedEvent::from_event(&built_event, &AddressParams::LIQUID_TESTNET)?;

        assert_eq!(parsed.swap_args, args);
        assert_eq!(parsed.utxo, utxo);
        assert_eq!(parsed.taproot_pubkey_gen.to_string(), taproot_pubkey_gen.to_string());

        Ok(())
    }
}
