use crate::common::keys::derive_secret_key_from_index;
use crate::common::settings::Settings;
use crate::common::store::SledError;
use crate::common::{DCDCliMakerFundArguments, broadcast_tx_inner, decode_hex};
use dex_nostr_relay::relay_processor::OrderPlaceEventTags;
use elements::bitcoin::hex::DisplayHex;
use elements::bitcoin::secp256k1;
use simplicity::elements::OutPoint;
use simplicity::elements::pset::serialize::Serialize;
use simplicity_contracts::DCDArguments;
use simplicity_contracts_adapter::dcd::{
    AssetEntropyProcessed, BaseContractContext, COLLATERAL_ASSET_ID, CreationContext, DcdContractContext, DcdManager,
    MakerFundingContext, raw_asset_entropy_bytes_to_midstate,
};
use simplicityhl::elements::{AddressParams, AssetId, Txid};
use simplicityhl_core::{
    AssetEntropyHex, LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, TaprootPubkeyGen, derive_public_blinder_key,
};
use tracing::instrument;

#[derive(Debug)]
pub struct ProcessedArgs {
    keypair: secp256k1::Keypair,
    dcd_arguments: DCDArguments,
    dcd_taproot_pubkey_gen: String,
    filler_token_entropy: AssetEntropyHex,
    grantor_collateral_token_entropy: AssetEntropyHex,
    grantor_settlement_token_entropy: AssetEntropyHex,
}

#[derive(Debug)]
pub struct ArgsToSave {
    taproot_pubkey_gen: TaprootPubkeyGen,
    dcd_arguments: DCDArguments,
}

#[derive(Debug)]
pub struct Utxos {
    pub filler_token: OutPoint,
    pub grantor_collateral_token: OutPoint,
    pub grantor_settlement_token: OutPoint,
    pub settlement_asset: OutPoint,
    pub fee: OutPoint,
}

impl ProcessedArgs {
    pub fn extract_event(&self) -> OrderPlaceEventTags {
        let convert_entropy_to_asset_id = |x: &str| {
            let x = hex::decode(x).unwrap();
            let token_entropy = simplicity_contracts_adapter::dcd::convert_bytes_to_asset_entropy(x).unwrap();
            let AssetEntropyProcessed {
                entropy: filler_token_asset_entropy,
                reversed_bytes: _filler_reversed_bytes,
            } = raw_asset_entropy_bytes_to_midstate(token_entropy);

            AssetId::from_entropy(filler_token_asset_entropy)
        };

        let filler_asset_id = convert_entropy_to_asset_id(&self.filler_token_entropy);
        let grantor_collateral_asset_id = convert_entropy_to_asset_id(&self.grantor_collateral_token_entropy);
        let grantor_settlement_asset_id = convert_entropy_to_asset_id(&self.grantor_settlement_token_entropy);
        let settlement_asset_id = convert_entropy_to_asset_id(&self.dcd_arguments.settlement_asset_id_hex_le);
        let collateral_asset_id = COLLATERAL_ASSET_ID;

        OrderPlaceEventTags {
            dcd_arguments: self.dcd_arguments.clone(),
            dcd_taproot_pubkey_gen: self.dcd_taproot_pubkey_gen.clone(),
            filler_asset_id,
            grantor_collateral_asset_id,
            grantor_settlement_asset_id,
            settlement_asset_id,
            collateral_asset_id,
        }
    }
}

#[instrument(level = "debug", skip_all, err)]
pub fn process_args(
    account_index: u32,
    dcd_init_params: Option<DCDCliMakerFundArguments>,
    dcd_taproot_pubkey_gen: impl AsRef<str>,
) -> crate::error::Result<ProcessedArgs> {
    let settings = Settings::load().map_err(|err| crate::error::CliError::EnvNotSet(err.to_string()))?;

    let keypair = secp256k1::Keypair::from_secret_key(
        secp256k1::SECP256K1,
        &derive_secret_key_from_index(account_index, settings.clone()),
    );

    let taproot_pubkey_gen = dcd_taproot_pubkey_gen.as_ref().to_string();

    let args = match dcd_init_params {
        None => {
            let dcd_args = crate::common::store::store_utils::get_dcd_args(&taproot_pubkey_gen)?;
            let filler_token_entropy =
                crate::common::store::store_utils::get_filler_token_entropy(&taproot_pubkey_gen)?;
            let grantor_collateral_token_entropy =
                crate::common::store::store_utils::get_grantor_collateral_token_entropy(&taproot_pubkey_gen)?;
            let grantor_settlement_token_entropy =
                crate::common::store::store_utils::get_grantor_settlement_token_entropy(&taproot_pubkey_gen)?;

            ProcessedArgs {
                keypair,
                dcd_arguments: dcd_args,
                dcd_taproot_pubkey_gen: taproot_pubkey_gen,
                filler_token_entropy,
                grantor_collateral_token_entropy,
                grantor_settlement_token_entropy,
            }
        }
        Some(x) => {
            let dcd_args = x.convert_to_dcd_arguments()?;
            ProcessedArgs {
                keypair,
                dcd_arguments: dcd_args,
                dcd_taproot_pubkey_gen: taproot_pubkey_gen,
                filler_token_entropy: x.filler_asset_entropy,
                grantor_collateral_token_entropy: x.grantor_collateral_asset_entropy,
                grantor_settlement_token_entropy: x.settlement_asset_entropy,
            }
        }
    };
    Ok(args)
}

#[instrument(level = "debug", skip_all, err)]
pub fn handle(
    ProcessedArgs {
        keypair,
        dcd_arguments,
        dcd_taproot_pubkey_gen,
        filler_token_entropy,
        grantor_collateral_token_entropy,
        grantor_settlement_token_entropy,
    }: ProcessedArgs,
    Utxos {
        filler_token: filler_token_utxo,
        grantor_collateral_token: grantor_collateral_token_utxo,
        grantor_settlement_token: grantor_settlement_token_utxo,
        settlement_asset: settlement_asset_utxo,
        fee: fee_utxo,
    }: Utxos,
    fee_amount: u64,
    broadcast: bool,
) -> crate::error::Result<(Txid, ArgsToSave)> {
    let filler_token_info = (filler_token_utxo, decode_hex(filler_token_entropy)?);
    let grantor_collateral_token_info = (
        grantor_collateral_token_utxo,
        decode_hex(grantor_collateral_token_entropy)?,
    );
    let grantor_settlement_token_info = (
        grantor_settlement_token_utxo,
        decode_hex(grantor_settlement_token_entropy)?,
    );

    let base_contract_context = BaseContractContext {
        address_params: &AddressParams::LIQUID_TESTNET,
        lbtc_asset: LIQUID_TESTNET_BITCOIN_ASSET,
        genesis_block_hash: *LIQUID_TESTNET_GENESIS,
    };
    let dcd_taproot_pubkey_gen = TaprootPubkeyGen::build_from_str(
        &dcd_taproot_pubkey_gen,
        &dcd_arguments,
        base_contract_context.address_params,
        &simplicity_contracts::get_dcd_address,
    )
    .map_err(|e| SledError::TapRootGen(e.to_string()))?;
    tracing::debug!("=== dcd arguments: {:?}", dcd_arguments);

    let transaction = DcdManager::maker_funding(
        &CreationContext {
            keypair,
            blinding_key: derive_public_blinder_key(),
        },
        MakerFundingContext {
            filler_token_info,
            grantor_collateral_token_info,
            grantor_settlement_token_info,
            settlement_asset_utxo,
            fee_utxo,
            fee_amount,
        },
        &DcdContractContext {
            dcd_taproot_pubkey_gen: dcd_taproot_pubkey_gen.clone(),
            dcd_arguments: dcd_arguments.clone(),
            base_contract_context,
        },
    )
    .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    if broadcast {
        println!("Broadcasted txid: {}", broadcast_tx_inner(&transaction)?);
    } else {
        println!("{}", transaction.serialize().to_lower_hex_string());
    }

    Ok((
        transaction.txid(),
        ArgsToSave {
            taproot_pubkey_gen: dcd_taproot_pubkey_gen,
            dcd_arguments,
        },
    ))
}

pub fn save_args_to_cache(
    ArgsToSave {
        taproot_pubkey_gen,
        dcd_arguments,
    }: &ArgsToSave,
) -> crate::error::Result<()> {
    crate::common::store::store_utils::save_dcd_args(taproot_pubkey_gen, dcd_arguments)?;
    Ok(())
}
