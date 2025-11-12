use crate::common::keys::derive_secret_key_from_index;
use crate::common::settings::Settings;
use crate::common::store::Store;
use crate::common::{DCDCliArguments, broadcast_tx_inner, decode_hex, vec_to_arr};
use dcd_manager::manager::init::DcdManager;
use dcd_manager::manager::types::AssetEntropyHex;
use elements::bitcoin::hex::DisplayHex;
use elements::bitcoin::secp256k1;
use simplicity::elements::OutPoint;
use simplicity::elements::pset::serialize::Serialize;
use simplicity_contracts::DCDArguments;
use simplicityhl::elements::{AddressParams, Txid};
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS};

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

    let dcd_init_params: DCDArguments = match dcd_init_params {
        None => {
            todo!()
        }
        Some(x) => x.try_into()?,
    };

    Ok(ProcessedArgs {
        keypair,
        dcd_arguments: Default::default(),
        dcd_taproot_pubkey_gen: dcd_taproot_pubkey_gen.as_ref().to_string(),
        filler_token_info: (fee_utxos[0], asset_entropies[0].clone()),
        grantor_collateral_token_info: (fee_utxos[1], asset_entropies[1].clone()),
        grantor_settlement_token_info: (fee_utxos[2], asset_entropies[2].clone()),
        settlement_asset_info: (fee_utxos[3], asset_entropies[3].clone()),
        fee_utxo: fee_utxos[4],
    })
}

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

    let transaction = DcdManager::maker_funding(
        keypair,
        filler_token_info,
        grantor_collateral_token_info,
        grantor_settlement_token_info,
        settlement_asset_info,
        fee_utxo,
        fee_amount,
        dcd_arguments,
        dcd_taproot_pubkey_gen,
        &AddressParams::LIQUID_TESTNET,
        LIQUID_TESTNET_BITCOIN_ASSET,
        *LIQUID_TESTNET_GENESIS,
    )
    .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    match broadcast {
        true => println!("Broadcasted txid: {}", broadcast_tx_inner(&transaction)?),
        false => println!("{}", transaction.serialize().to_lower_hex_string()),
    }

    Ok(transaction.txid())
}

pub fn save_args_to_cache() -> crate::error::Result<()> {
    let store = Store::load()?;
    //todo: move store to cli function
    Ok(())
}
