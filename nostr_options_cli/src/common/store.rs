use hex::FromHexError;
use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;
use simplicityhl::simplicity::elements::{Address, AddressParams};
use simplicityhl_core::{Encodable, TaprootPubkeyGen};
use sled::IVec;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct Store {
    store: sled::Db,
}

#[derive(Error, Debug)]
pub enum SledError {
    #[error(transparent)]
    Sled(#[from] sled::Error),
    #[error("Arguments not found")]
    ArgumentNotFound,
    #[error("Encodable error, msg: {0}")]
    Encode(String),
    #[error("Hex parsing error, msg: {0}")]
    Hex(#[from] FromHexError),
    #[error("Tap root gen error, msg: {0}")]
    TapRootGen(String),
}

pub type Result<T> = std::result::Result<T, SledError>;

impl Store {
    pub fn load() -> Result<Self> {
        Ok(Self {
            store: sled::open(".cache/store")?,
        })
    }

    pub fn is_exist(&self, asset_name: &str) -> Result<bool> {
        Ok(self.store.get(asset_name)?.is_some())
    }

    pub fn insert_value<K, V>(&self, key: K, value: V) -> Result<Option<IVec>>
    where
        K: AsRef<[u8]>,
        V: Into<IVec>,
    {
        Ok(self.store.insert(key, value)?)
    }

    pub fn get_value<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<IVec>> {
        Ok(self.store.get(key)?)
    }

    pub fn import_arguments<A>(
        &self,
        taproot_pubkey_gen: &str,
        encoded_data: &str,
        params: &'static AddressParams,
        get_address: &impl Fn(&XOnlyPublicKey, &A, &'static AddressParams) -> anyhow::Result<Address>,
    ) -> Result<()>
    where
        A: Encodable + simplicityhl_core::encoding::Decode<()>,
    {
        let decoded_data = hex::decode(encoded_data)?;

        let arguments = Encodable::decode(&decoded_data).map_err(|e| SledError::Encode(e.to_string()))?;
        let _ = TaprootPubkeyGen::build_from_str(taproot_pubkey_gen, &arguments, params, get_address)
            .map_err(|e| SledError::TapRootGen(e.to_string()))?;

        self.store.insert(taproot_pubkey_gen, decoded_data)?;

        Ok(())
    }

    pub fn export_arguments(&self, taproot_pubkey_gen: &str) -> Result<String> {
        if let Some(value) = self.store.get(taproot_pubkey_gen)? {
            return Ok(hex::encode(value));
        }

        Err(SledError::ArgumentNotFound)
    }

    pub fn get_arguments<A>(&self, taproot_pubkey_gen: &str) -> Result<A>
    where
        A: Encodable + simplicityhl_core::encoding::Decode<()>,
    {
        if let Some(value) = self.store.get(taproot_pubkey_gen)? {
            return Encodable::decode(&value).map_err(|err| SledError::Encode(err.to_string()));
        }
        Err(SledError::ArgumentNotFound)
    }
}

#[cfg(test)]
mod tests {
    use simplicity_contracts::get_options_program;
    use simplicityhl::simplicity::elements;
    use simplicityhl_core::{Encodable, TaprootPubkeyGen};
    use simplicityhl_core::{LIQUID_TESTNET_TEST_ASSET_ID_STR, create_p2tr_address};
    use std::collections::HashMap;

    use hex::FromHex;
    use simplicityhl::num::U256;
    use simplicityhl::{Arguments, str::WitnessName, value::UIntValue};

    use super::*;

    #[derive(Debug, Clone, bincode::Encode, bincode::Decode, PartialEq)]
    pub struct OptionsArguments {
        pub start_time: u32,
        pub expiry_time: u32,
        pub collateral_per_contract: u64,
        pub settlement_per_contract: u64,
        pub collateral_asset_id_hex_le: String,
        pub settlement_asset_id_hex_le: String,
        pub option_token_asset_id_hex_le: String,
        pub grantor_token_asset_id_hex_le: String,
    }

    impl Default for OptionsArguments {
        fn default() -> Self {
            Self {
                start_time: 0,
                expiry_time: 0,
                collateral_per_contract: 0,
                settlement_per_contract: 0,
                collateral_asset_id_hex_le: "00".repeat(32),
                option_token_asset_id_hex_le: "00".repeat(32),
                grantor_token_asset_id_hex_le: "00".repeat(32),
                settlement_asset_id_hex_le: "00".repeat(32),
            }
        }
    }

    impl OptionsArguments {
        pub fn build_option_arguments(&self) -> Arguments {
            Arguments::from(HashMap::from([
                (
                    WitnessName::from_str_unchecked("START_TIME"),
                    simplicityhl::Value::from(UIntValue::U32(self.start_time)),
                ),
                (
                    WitnessName::from_str_unchecked("EXPIRY_TIME"),
                    simplicityhl::Value::from(UIntValue::U32(self.expiry_time)),
                ),
                (
                    WitnessName::from_str_unchecked("COLLATERAL_PER_CONTRACT"),
                    simplicityhl::Value::from(UIntValue::U64(self.collateral_per_contract)),
                ),
                (
                    WitnessName::from_str_unchecked("SETTLEMENT_PER_CONTRACT"),
                    simplicityhl::Value::from(UIntValue::U64(self.settlement_per_contract)),
                ),
                (
                    WitnessName::from_str_unchecked("COLLATERAL_ASSET_ID"),
                    simplicityhl::Value::from(UIntValue::U256(u256_from_le_hex(&self.collateral_asset_id_hex_le))),
                ),
                (
                    WitnessName::from_str_unchecked("SETTLEMENT_ASSET_ID"),
                    simplicityhl::Value::from(UIntValue::U256(u256_from_le_hex(&self.settlement_asset_id_hex_le))),
                ),
                (
                    WitnessName::from_str_unchecked("OPTION_TOKEN_ASSET"),
                    simplicityhl::Value::from(UIntValue::U256(u256_from_le_hex(&self.option_token_asset_id_hex_le))),
                ),
                (
                    WitnessName::from_str_unchecked("GRANTOR_TOKEN_ASSET"),
                    simplicityhl::Value::from(UIntValue::U256(u256_from_le_hex(&self.grantor_token_asset_id_hex_le))),
                ),
            ]))
        }
    }

    impl simplicityhl_core::Encodable for OptionsArguments {}

    fn u256_from_le_hex(hex_le: &str) -> U256 {
        let mut bytes = <[u8; 32]>::from_hex(hex_le).expect("expected 32 bytes hex");
        bytes.reverse();
        U256::from_byte_array(bytes)
    }

    fn load_mock() -> Store {
        Store {
            store: sled::Config::new().temporary(true).open().expect("expected store"),
        }
    }

    fn get_mocked_data() -> anyhow::Result<(OptionsArguments, TaprootPubkeyGen)> {
        let args = OptionsArguments {
            start_time: 10,
            expiry_time: 50,
            collateral_per_contract: 100,
            settlement_per_contract: 1000,
            collateral_asset_id_hex_le: elements::AssetId::LIQUID_BTC.to_string(),
            settlement_asset_id_hex_le: LIQUID_TESTNET_TEST_ASSET_ID_STR.to_string(),
            option_token_asset_id_hex_le: elements::AssetId::LIQUID_BTC.to_string(),
            grantor_token_asset_id_hex_le: elements::AssetId::LIQUID_BTC.to_string(),
        };

        let options_taproot_pubkey_gen =
            TaprootPubkeyGen::from(&args, &AddressParams::LIQUID_TESTNET, &get_options_address)?;

        Ok((args, options_taproot_pubkey_gen))
    }

    pub fn get_options_address(
        x_only_public_key: &XOnlyPublicKey,
        arguments: &OptionsArguments,
        params: &'static AddressParams,
    ) -> anyhow::Result<Address> {
        Ok(create_p2tr_address(
            get_options_program(&simplicity_contracts::build_arguments::OptionsArguments {
                start_time: arguments.start_time,
                expiry_time: arguments.expiry_time,
                collateral_per_contract: arguments.collateral_per_contract,
                settlement_per_contract: arguments.settlement_per_contract,
                collateral_asset_id_hex_le: arguments.collateral_asset_id_hex_le.clone(),
                settlement_asset_id_hex_le: arguments.settlement_asset_id_hex_le.clone(),
                option_token_asset_id_hex_le: arguments.option_token_asset_id_hex_le.clone(),
                grantor_token_asset_id_hex_le: arguments.grantor_token_asset_id_hex_le.clone(),
            })?
            .commit()
            .cmr(),
            x_only_public_key,
            params,
        ))
    }

    #[test]
    fn test_sled_serialize_deserialize() -> anyhow::Result<()> {
        let store = load_mock();

        let (args, options_taproot_pubkey_gen) = get_mocked_data()?;

        store.import_arguments(
            &options_taproot_pubkey_gen.to_string(),
            &args.to_hex()?,
            &AddressParams::LIQUID_TESTNET,
            &get_options_address,
        )?;

        let retrieved = store.get_arguments::<OptionsArguments>(&options_taproot_pubkey_gen.to_string())?;

        assert_eq!(args, retrieved);

        Ok(())
    }

    #[test]
    fn test_sled_import_export_roundtrip() -> anyhow::Result<()> {
        let store = load_mock();

        let (args, options_taproot_pubkey_gen) = get_mocked_data()?;

        store.import_arguments(
            &options_taproot_pubkey_gen.to_string(),
            &args.to_hex()?,
            &AddressParams::LIQUID_TESTNET,
            &get_options_address,
        )?;

        let exported_hex = store.export_arguments(&options_taproot_pubkey_gen.to_string())?;

        assert_eq!(exported_hex, args.to_hex()?);

        Ok(())
    }

    #[test]
    fn test_sled_export_get_consistency() -> anyhow::Result<()> {
        let store = load_mock();

        let (args, options_taproot_pubkey_gen) = get_mocked_data()?;

        store.import_arguments(
            &options_taproot_pubkey_gen.to_string(),
            &args.to_hex()?,
            &AddressParams::LIQUID_TESTNET,
            &get_options_address,
        )?;

        let exported_hex = store.export_arguments(&options_taproot_pubkey_gen.to_string())?;
        let exported_bytes = hex::decode(&exported_hex)?;
        let decoded_from_export: OptionsArguments = Encodable::decode(&exported_bytes)?;

        let retrieved = store.get_arguments::<OptionsArguments>(&options_taproot_pubkey_gen.to_string())?;

        assert_eq!(decoded_from_export, retrieved);
        assert_eq!(retrieved, args);

        Ok(())
    }
}
