use crate::common::keys::derive_secret_key_from_index;
use crate::common::settings::Settings;
use crate::common::store::Store;
use crate::common::{DCDCliArguments, broadcast_tx_inner, decode_hex, entropy_to_asset_id, vec_to_arr};
use dcd_manager::manager::common::{AssetEntropyProcessed, convert_asset_entropy, raw_asset_entropy_bytes_to_midstate};
use dcd_manager::manager::init::DcdManager;
use dcd_manager::manager::types::{AssetEntropyHex, COLLATERAL_ASSET_ID};
use elements::bitcoin::hex::DisplayHex;
use elements::bitcoin::secp256k1;
use elements::hex::ToHex;
use nostr_relay_processor::relay_processor::OrderPlaceEventTags;
use simplicity::elements::OutPoint;
use simplicity::elements::pset::serialize::Serialize;
use simplicity_contracts::{DCDArguments, DCDRatioArguments};
use simplicityhl::elements::{AddressParams, AssetId, Txid};
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS};
use std::str::FromStr;
use tracing::instrument;

#[derive(Debug)]
pub struct ProcessedArgs {
    keypair: secp256k1::Keypair,
    dcd_arguments: DCDArguments,
    dcd_taproot_pubkey_gen: String,
    filler_token_info: (OutPoint, AssetEntropyHex),
    grantor_collateral_token_info: (OutPoint, AssetEntropyHex),
    grantor_settlement_token_info: (OutPoint, AssetEntropyHex),
    settlement_asset_info: (OutPoint, AssetEntropyHex),
    fee_utxo: OutPoint,
}

impl ProcessedArgs {
    pub fn extract_event(&self) -> OrderPlaceEventTags {
        let convert_entropy_to_asset_id = |x: &str| {
            let x = hex::decode(x).unwrap();
            let token_entropy = convert_asset_entropy(x).unwrap();
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
        let settlement_asset_id = convert_entropy_to_asset_id(&self.settlement_asset_info.1);
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
    dcd_init_params: Option<DCDCliArguments>,
    dcd_taproot_pubkey_gen: impl AsRef<str>,
    fee_utxos: Vec<OutPoint>,
    tokens_entropies: Vec<AssetEntropyHex>,
) -> crate::error::Result<ProcessedArgs> {
    let store = Store::load()?;

    let settings = Settings::load().map_err(|err| crate::error::CliError::EnvNotSet(err.to_string()))?;

    let keypair = secp256k1::Keypair::from_secret_key(
        secp256k1::SECP256K1,
        &derive_secret_key_from_index(account_index, settings.clone()),
    );

    let fee_utxos = vec_to_arr::<5, OutPoint>(fee_utxos)?;
    let asset_entropies = vec_to_arr::<4, AssetEntropyHex>(tokens_entropies)?;

    let dcd_arguments: DCDArguments = match dcd_init_params {
        None => {
            todo!()
        }
        Some(x) => convert_to_dcd_arguments(x, &asset_entropies)?,
    };

    Ok(ProcessedArgs {
        keypair,
        dcd_arguments,
        dcd_taproot_pubkey_gen: dcd_taproot_pubkey_gen.as_ref().to_string(),
        filler_token_info: (fee_utxos[0], asset_entropies[0].clone()),
        grantor_collateral_token_info: (fee_utxos[1], asset_entropies[1].clone()),
        grantor_settlement_token_info: (fee_utxos[2], asset_entropies[2].clone()),
        settlement_asset_info: (fee_utxos[3], asset_entropies[3].clone()),
        fee_utxo: fee_utxos[4],
    })
}

fn convert_hex_be_to_le(hex_str: impl AsRef<[u8]>) -> crate::error::Result<AssetEntropyHex> {
    let hex_str = hex_str.as_ref();
    let mut bytes = hex::decode(hex_str).map_err(|err| crate::error::CliError::FromHex(err, hex_str.to_hex()))?;
    bytes.reverse();
    Ok(bytes.to_hex())
}

#[instrument(level = "debug", skip_all, err)]
fn convert_to_dcd_arguments(
    value: DCDCliArguments,
    entropies: &[AssetEntropyHex; 4],
) -> crate::error::Result<DCDArguments> {
    Ok(DCDArguments {
        taker_funding_start_time: value.taker_funding_start_time,
        taker_funding_end_time: value.taker_funding_end_time,
        contract_expiry_time: value.contract_expiry_time,
        early_termination_end_time: value.early_termination_end_time,
        settlement_height: value.settlement_height,
        strike_price: value.strike_price,
        incentive_basis_points: value.incentive_basis_points,
        filler_token_asset_id_hex_le: entropy_to_asset_id(&entropies[0])?.to_string(),
        grantor_collateral_token_asset_id_hex_le: entropy_to_asset_id(&entropies[1])?.to_string(),
        grantor_settlement_token_asset_id_hex_le: entropy_to_asset_id(&entropies[2])?.to_string(),
        settlement_asset_id_hex_le: entropy_to_asset_id(&entropies[3])?.to_string(),
        collateral_asset_id_hex_le: COLLATERAL_ASSET_ID.to_hex(),
        oracle_public_key: value.oracle_public_key.x_only_public_key().0.to_string(),
        ratio_args: DCDRatioArguments::build_from(
            value.principal_collateral_amount,
            value.incentive_basis_points,
            value.strike_price,
            value.filler_per_principal_collateral,
        )
        .map_err(|err| crate::error::CliError::DcdRatioArgs(err.to_string()))?,
    })
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
        settlement_asset_info,
        fee_utxo,
    }: ProcessedArgs,
    fee_amount: u64,
    broadcast: bool,
) -> crate::error::Result<Txid> {
    let filler_token_info = (filler_token_info.0, decode_hex(filler_token_info.1)?);
    let grantor_collateral_token_info = (
        grantor_collateral_token_info.0,
        decode_hex(grantor_collateral_token_info.1)?,
    );
    let grantor_settlement_token_info = (
        grantor_settlement_token_info.0,
        decode_hex(grantor_settlement_token_info.1)?,
    );
    let settlement_asset_info = (settlement_asset_info.0, decode_hex(settlement_asset_info.1)?);

    tracing::debug!("=== dcd arguments: {:?}", dcd_arguments);
    todo!();
    // let transaction = DcdManager::maker_settlement(keypair, dcd_arguments, asset_utxo, grantor_collateral_token_utxo, grantor_settlement_token_utxo, fee_utxo, fee_amount, price_at_current_block_height, oracle_signature, grantor_amount_to_burn, dcd_taproot_pubkey_gen,  LIQUID_TESTNET_BITCOIN_ASSET , &AddressParams::LIQUID_TESTNET, *LIQUID_TESTNET_GENESIS)
    //     .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    // match broadcast {
    //     true => println!("Broadcasted txid: {}", broadcast_tx_inner(&transaction)?),
    //     false => println!("{}", transaction.serialize().to_lower_hex_string()),
    // }
    //
    // Ok(transaction.txid())
    Ok(Txid::from_str("").unwrap())
}

pub fn save_args_to_cache() -> crate::error::Result<()> {
    let store = Store::load()?;
    //todo: move store to cli function
    Ok(())
}
