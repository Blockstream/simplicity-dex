use crate::error::{ParseError, RelayError};
use crate::events::kinds::{OPTION_CREATED, TAG_OPTIONS_ARGS, TAG_OPTIONS_UTXO, TAG_TAPROOT_GEN};

use contracts::options::{OptionsArguments, get_options_address};
use contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen;
use nostr::{Event, EventBuilder, EventId, PublicKey, Tag, TagKind, Timestamp};
use simplicityhl::elements::{AddressParams, OutPoint};
use simplicityhl_core::Encodable;

#[derive(Debug, Clone)]
pub struct OptionCreatedEvent {
    pub event_id: EventId,
    pub pubkey: PublicKey,
    pub created_at: Timestamp,
    pub options_args: OptionsArguments,
    pub utxo: OutPoint,
    pub taproot_pubkey_gen: TaprootPubkeyGen,
}

impl OptionCreatedEvent {
    #[must_use]
    pub fn new(options_args: OptionsArguments, utxo: OutPoint, taproot_pubkey_gen: TaprootPubkeyGen) -> Self {
        Self {
            event_id: EventId::all_zeros(),
            pubkey: PublicKey::from_slice(&[1; 32]).unwrap(),
            created_at: Timestamp::now(),
            options_args,
            utxo,
            taproot_pubkey_gen,
        }
    }

    pub fn to_event_builder(&self, creator_pubkey: PublicKey) -> Result<EventBuilder, RelayError> {
        let args_hex = self.options_args.to_hex()?;

        Ok(EventBuilder::new(OPTION_CREATED, "")
            .tag(Tag::public_key(creator_pubkey))
            .tag(Tag::custom(TagKind::custom(TAG_OPTIONS_ARGS), [args_hex]))
            .tag(Tag::custom(TagKind::custom(TAG_OPTIONS_UTXO), [self.utxo.to_string()]))
            .tag(Tag::custom(
                TagKind::custom(TAG_TAPROOT_GEN),
                [self.taproot_pubkey_gen.to_string()],
            )))
    }

    pub fn from_event(event: &Event, params: &'static AddressParams) -> Result<Self, ParseError> {
        event.verify()?;

        if event.kind != OPTION_CREATED {
            return Err(ParseError::InvalidKind);
        }

        let args_hex = event
            .tags
            .iter()
            .find(|t| matches!(t.kind(), TagKind::Custom(s) if s.as_ref() == TAG_OPTIONS_ARGS))
            .and_then(|t| t.content())
            .ok_or(ParseError::MissingTag(TAG_OPTIONS_ARGS))?;

        let options_args = OptionsArguments::from_hex(args_hex)?;

        let utxo_str = event
            .tags
            .iter()
            .find(|t| matches!(t.kind(), TagKind::Custom(s) if s.as_ref() == TAG_OPTIONS_UTXO))
            .and_then(|t| t.content())
            .ok_or(ParseError::MissingTag(TAG_OPTIONS_UTXO))?;

        let utxo: OutPoint = utxo_str.parse()?;

        let taproot_str = event
            .tags
            .iter()
            .find(|t| matches!(t.kind(), TagKind::SingleLetter(l) if l.character == nostr::Alphabet::T))
            .and_then(|t| t.content())
            .ok_or(ParseError::MissingTag(TAG_TAPROOT_GEN))?;

        let taproot_pubkey_gen =
            TaprootPubkeyGen::build_from_str(taproot_str, &options_args, params, &get_options_address)?;

        Ok(Self {
            event_id: event.id,
            pubkey: event.pubkey,
            created_at: event.created_at,
            options_args,
            utxo,
            taproot_pubkey_gen,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use nostr::{Keys, hashes::Hash};

    use contracts::sdk::taproot_pubkey_gen::get_random_seed;

    use simplicityhl::elements::{AssetId, Txid};
    use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_TEST_ASSET_ID_STR};

    fn get_mocked_data() -> anyhow::Result<(OptionsArguments, TaprootPubkeyGen)> {
        let settlement_asset_id = AssetId::from_slice(&hex::decode(LIQUID_TESTNET_TEST_ASSET_ID_STR)?)?;

        let option_creation_outpoint = OutPoint::new(Txid::from_slice(&[1; 32])?, 0);
        let grantor_creation_outpoint = OutPoint::new(Txid::from_slice(&[2; 32])?, 0);

        let args = OptionsArguments::new(
            10,
            50,
            100,
            1000,
            *LIQUID_TESTNET_BITCOIN_ASSET,
            settlement_asset_id,
            get_random_seed(),
            (option_creation_outpoint, false),
            (grantor_creation_outpoint, false),
        );

        let taproot_pubkey_gen = TaprootPubkeyGen::from(&args, &AddressParams::LIQUID_TESTNET, &get_options_address)?;

        Ok((args, taproot_pubkey_gen))
    }

    #[test]
    fn option_created_event_roundtrip() -> anyhow::Result<()> {
        let keys = Keys::generate();
        let (args, taproot_pubkey_gen) = get_mocked_data()?;
        let utxo = OutPoint::new(Txid::all_zeros(), 0);

        let event = OptionCreatedEvent::new(args.clone(), utxo, taproot_pubkey_gen.clone());

        let builder = event.to_event_builder(keys.public_key())?;
        let built_event = builder.sign_with_keys(&keys)?;

        let parsed = OptionCreatedEvent::from_event(&built_event, &AddressParams::LIQUID_TESTNET)?;

        assert_eq!(parsed.options_args, args);
        assert_eq!(parsed.utxo, utxo);
        assert_eq!(parsed.taproot_pubkey_gen.to_string(), taproot_pubkey_gen.to_string());

        Ok(())
    }
}
