use crate::error::{ParseError, RelayError};
use crate::events::kinds::{
    OPTION_OFFER_CREATED, TAG_EXPIRY, TAG_OPTION_OFFER_ARGS, TAG_OPTION_OFFER_UTXO, TAG_TAPROOT_GEN,
};

use contracts::option_offer::{OptionOfferArguments, get_option_offer_address};
use contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen;
use nostr::{Event, EventBuilder, EventId, PublicKey, Tag, TagKind, Timestamp};
use simplicityhl::elements::OutPoint;
use simplicityhl_core::{Encodable, SimplicityNetwork};

#[derive(Debug, Clone)]
pub struct OptionOfferCreatedEvent {
    pub event_id: EventId,
    pub pubkey: PublicKey,
    pub created_at: Timestamp,
    pub option_offer_args: OptionOfferArguments,
    pub utxo: OutPoint,
    pub taproot_pubkey_gen: TaprootPubkeyGen,
}

impl OptionOfferCreatedEvent {
    #[must_use]
    pub fn new(option_offer_args: OptionOfferArguments, utxo: OutPoint, taproot_pubkey_gen: TaprootPubkeyGen) -> Self {
        Self {
            event_id: EventId::all_zeros(),
            pubkey: PublicKey::from_slice(&[1; 32]).unwrap(),
            created_at: Timestamp::now(),
            option_offer_args,
            utxo,
            taproot_pubkey_gen,
        }
    }

    pub fn to_event_builder(&self, creator_pubkey: PublicKey) -> Result<EventBuilder, RelayError> {
        let args_hex = self.option_offer_args.to_hex()?;

        Ok(EventBuilder::new(OPTION_OFFER_CREATED, "")
            .tag(Tag::public_key(creator_pubkey))
            .tag(Tag::custom(TagKind::custom(TAG_OPTION_OFFER_ARGS), [args_hex]))
            .tag(Tag::custom(
                TagKind::custom(TAG_OPTION_OFFER_UTXO),
                [self.utxo.to_string()],
            ))
            .tag(Tag::custom(
                TagKind::custom(TAG_TAPROOT_GEN),
                [self.taproot_pubkey_gen.to_string()],
            ))
            .tag(Tag::custom(
                TagKind::custom(TAG_EXPIRY),
                [self.option_offer_args.expiry_time().to_string()],
            )))
    }

    pub fn from_event(event: &Event, network: SimplicityNetwork) -> Result<Self, ParseError> {
        event.verify()?;

        if event.kind != OPTION_OFFER_CREATED {
            return Err(ParseError::InvalidKind);
        }

        let args_hex = event
            .tags
            .iter()
            .find(|t| matches!(t.kind(), TagKind::Custom(s) if s.as_ref() == TAG_OPTION_OFFER_ARGS))
            .and_then(|t| t.content())
            .ok_or(ParseError::MissingTag(TAG_OPTION_OFFER_ARGS))?;

        let option_offer_args = OptionOfferArguments::from_hex(args_hex)?;

        let utxo_str = event
            .tags
            .iter()
            .find(|t| matches!(t.kind(), TagKind::Custom(s) if s.as_ref() == TAG_OPTION_OFFER_UTXO))
            .and_then(|t| t.content())
            .ok_or(ParseError::MissingTag(TAG_OPTION_OFFER_UTXO))?;

        let utxo: OutPoint = utxo_str.parse()?;

        let taproot_str = event
            .tags
            .iter()
            .find(|t| matches!(t.kind(), TagKind::SingleLetter(l) if l.character == nostr::Alphabet::T))
            .and_then(|t| t.content())
            .ok_or(ParseError::MissingTag(TAG_TAPROOT_GEN))?;

        let taproot_pubkey_gen =
            TaprootPubkeyGen::build_from_str(taproot_str, &option_offer_args, network, &get_option_offer_address)?;

        Ok(Self {
            event_id: event.id,
            pubkey: event.pubkey,
            created_at: event.created_at,
            option_offer_args,
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
    use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_TEST_ASSET_ID_STR, SimplicityNetwork};

    fn get_mocked_data() -> anyhow::Result<(OptionOfferArguments, TaprootPubkeyGen)> {
        let settlement_asset_id = AssetId::from_slice(&hex::decode(LIQUID_TESTNET_TEST_ASSET_ID_STR)?)?;
        let premium_asset_id = AssetId::from_slice(&hex::decode(LIQUID_TESTNET_TEST_ASSET_ID_STR)?)?;

        let args = OptionOfferArguments::new(
            *LIQUID_TESTNET_BITCOIN_ASSET,
            premium_asset_id,
            settlement_asset_id,
            1000,
            50,
            1_700_000_000,
            [1; 32],
        );

        let taproot_pubkey_gen =
            TaprootPubkeyGen::from(&args, SimplicityNetwork::LiquidTestnet, &get_option_offer_address)?;

        Ok((args, taproot_pubkey_gen))
    }

    #[test]
    fn option_offer_created_event_roundtrip() -> anyhow::Result<()> {
        let keys = Keys::generate();
        let (args, taproot_pubkey_gen) = get_mocked_data()?;
        let utxo = OutPoint::new(Txid::all_zeros(), 0);

        let event = OptionOfferCreatedEvent::new(args.clone(), utxo, taproot_pubkey_gen.clone());

        let builder = event.to_event_builder(keys.public_key())?;
        let built_event = builder.sign_with_keys(&keys)?;

        let parsed = OptionOfferCreatedEvent::from_event(&built_event, SimplicityNetwork::LiquidTestnet)?;

        assert_eq!(parsed.option_offer_args, args);
        assert_eq!(parsed.utxo, utxo);
        assert_eq!(parsed.taproot_pubkey_gen.to_string(), taproot_pubkey_gen.to_string());

        Ok(())
    }
}
