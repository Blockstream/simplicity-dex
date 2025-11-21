use crate::common::keys::derive_secret_key_from_index;
use crate::common::settings::Settings;
use crate::common::store::{SledError, Store};
use crate::common::{DCDCliMakerFundArguments, broadcast_tx_inner, decode_hex, vec_to_arr};
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
    filler_token_info: (OutPoint, AssetEntropyHex),
    grantor_collateral_token_info: (OutPoint, AssetEntropyHex),
    grantor_settlement_token_info: (OutPoint, AssetEntropyHex),
    settlement_asset_utxo: OutPoint,
    fee_utxo: OutPoint,
}

#[derive(Debug)]
pub struct ArgsToSave {
    taproot_pubkey_gen: TaprootPubkeyGen,
    dcd_arguments: DCDArguments,
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
            let asset_id = AssetId::from_entropy(filler_token_asset_entropy);
            asset_id
        };

        let filler_asset_id = convert_entropy_to_asset_id(&self.filler_token_info.1);
        let grantor_collateral_asset_id = convert_entropy_to_asset_id(&self.grantor_collateral_token_info.1);
        let grantor_settlement_asset_id = convert_entropy_to_asset_id(&self.grantor_settlement_token_info.1);
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
    fee_utxos: Vec<OutPoint>,
) -> crate::error::Result<ProcessedArgs> {
    const FEE_UTXOS_NEEDED: usize = 5;

    let _store = Store::load()?;

    let settings = Settings::load().map_err(|err| crate::error::CliError::EnvNotSet(err.to_string()))?;

    let keypair = secp256k1::Keypair::from_secret_key(
        secp256k1::SECP256K1,
        &derive_secret_key_from_index(account_index, settings.clone()),
    );

    let fee_utxos = vec_to_arr::<FEE_UTXOS_NEEDED, OutPoint>(fee_utxos)?;
    let taproot_pubkey_gen = dcd_taproot_pubkey_gen.as_ref().to_string();

    let args = match dcd_init_params {
        None => {
            let dcd_args = crate::common::store::store_utils::get_dcd_args(&taproot_pubkey_gen)?;
            let filler_token_info = crate::common::store::store_utils::get_filler_token_entropy(&taproot_pubkey_gen)?;
            let grantor_collateral_token_info =
                crate::common::store::store_utils::get_grantor_collateral_token_entropy(&taproot_pubkey_gen)?;
            let grantor_settlement_token_info =
                crate::common::store::store_utils::get_grantor_settlement_token_entropy(&taproot_pubkey_gen)?;

            ProcessedArgs {
                keypair,
                dcd_arguments: dcd_args,
                dcd_taproot_pubkey_gen: taproot_pubkey_gen,
                filler_token_info: (fee_utxos[0], filler_token_info),
                grantor_collateral_token_info: (fee_utxos[1], grantor_collateral_token_info),
                grantor_settlement_token_info: (fee_utxos[2], grantor_settlement_token_info),
                settlement_asset_utxo: fee_utxos[3],
                fee_utxo: fee_utxos[4],
            }
        }
        Some(x) => {
            let dcd_args = x.convert_to_dcd_arguments()?;
            ProcessedArgs {
                keypair,
                dcd_arguments: dcd_args,
                dcd_taproot_pubkey_gen: taproot_pubkey_gen,
                filler_token_info: (fee_utxos[0], x.filler_asset_entropy.clone()),
                grantor_collateral_token_info: (fee_utxos[1], x.grantor_collateral_asset_entropy.clone()),
                grantor_settlement_token_info: (fee_utxos[2], x.grantor_settlement_asset_entropy.clone()),
                settlement_asset_utxo: fee_utxos[3],
                fee_utxo: fee_utxos[4],
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
        filler_token_info,
        grantor_collateral_token_info,
        grantor_settlement_token_info,
        settlement_asset_utxo,
        fee_utxo,
    }: ProcessedArgs,
    fee_amount: u64,
    broadcast: bool,
) -> crate::error::Result<(Txid, ArgsToSave)> {
    let filler_token_info = (filler_token_info.0, decode_hex(filler_token_info.1)?);
    let grantor_collateral_token_info = (
        grantor_collateral_token_info.0,
        decode_hex(grantor_collateral_token_info.1)?,
    );
    let grantor_settlement_token_info = (
        grantor_settlement_token_info.0,
        decode_hex(grantor_settlement_token_info.1)?,
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

    match broadcast {
        true => println!("Broadcasted txid: {}", broadcast_tx_inner(&transaction)?),
        false => println!("{}", transaction.serialize().to_lower_hex_string()),
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
