use crate::common::keys::derive_keypair_from_index;
use crate::common::settings::Settings;
use crate::common::store::SledError;
use crate::common::store::utils::OrderParams;
use crate::contract_handlers::common::{broadcast_or_get_raw_tx, get_order_params};
use contracts::DCDArguments;
use contracts_adapter::dcd::{
    BaseContractContext, CommonContext, DcdContractContext, DcdManager, TakerSettlementContext,
};
use dex_nostr_relay::relay_processor::RelayProcessor;
use elements::bitcoin::secp256k1;
use nostr::EventId;
use simplicity::elements::OutPoint;
use simplicityhl::elements::{AddressParams, Txid};
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, TaprootPubkeyGen};
use tokio::task;
use tracing::instrument;

#[derive(Debug)]
pub struct ProcessedArgs {
    keypair: secp256k1::Keypair,
    dcd_arguments: DCDArguments,
    dcd_taproot_pubkey_gen: String,
    price_at_current_block_height: u64,
    filler_amount_to_burn: u64,
    oracle_signature: String,
}

#[derive(Debug)]
pub struct ArgsToSave {
    taproot_pubkey_gen: TaprootPubkeyGen,
    dcd_arguments: DCDArguments,
}

#[derive(Debug)]
pub struct Utxos {
    pub filler_token: OutPoint,
    pub asset: OutPoint,
    pub fee: OutPoint,
}

#[instrument(level = "debug", skip_all, err)]
pub async fn process_args(
    account_index: u32,
    price_at_current_block_height: u64,
    filler_amount_to_burn: u64,
    oracle_signature: String,
    maker_order_event_id: EventId,
    relay_processor: &RelayProcessor,
) -> crate::error::Result<ProcessedArgs> {
    let settings = Settings::load().map_err(|err| crate::error::CliError::EnvNotSet(err.to_string()))?;

    let keypair = derive_keypair_from_index(account_index, &settings.seed_hex);

    let order_params: OrderParams = get_order_params(maker_order_event_id, relay_processor).await?;

    Ok(ProcessedArgs {
        keypair,
        dcd_arguments: order_params.dcd_args,
        dcd_taproot_pubkey_gen: order_params.taproot_pubkey_gen,
        price_at_current_block_height,
        filler_amount_to_burn,
        oracle_signature,
    })
}

#[instrument(level = "debug", skip_all, err)]
pub async fn handle(
    processed_args: ProcessedArgs,
    utxos: Utxos,
    fee_amount: u64,
    is_offline: bool,
) -> crate::error::Result<(Txid, ArgsToSave)> {
    task::spawn_blocking(move || handle_sync(processed_args, utxos, fee_amount, is_offline)).await?
}

#[instrument(level = "debug", skip_all, err)]
fn handle_sync(
    ProcessedArgs {
        keypair,
        dcd_arguments,
        dcd_taproot_pubkey_gen,
        price_at_current_block_height,
        filler_amount_to_burn,
        oracle_signature,
    }: ProcessedArgs,
    Utxos {
        filler_token: filler_token_utxo,
        asset: asset_utxo,
        fee: fee_utxo,
    }: Utxos,
    fee_amount: u64,
    is_offline: bool,
) -> crate::error::Result<(Txid, ArgsToSave)> {
    tracing::debug!("=== dcd arguments: {:?}", dcd_arguments);
    let base_contract_context = BaseContractContext {
        address_params: &AddressParams::LIQUID_TESTNET,
        lbtc_asset: LIQUID_TESTNET_BITCOIN_ASSET,
        genesis_block_hash: *LIQUID_TESTNET_GENESIS,
    };
    let dcd_taproot_pubkey_gen = TaprootPubkeyGen::build_from_str(
        &dcd_taproot_pubkey_gen,
        &dcd_arguments,
        base_contract_context.address_params,
        &contracts::get_dcd_address,
    )
    .map_err(|e| SledError::TapRootGen(e.to_string()))?;

    let transaction = DcdManager::taker_settlement(
        &CommonContext { keypair },
        TakerSettlementContext {
            asset_utxo,
            filler_token_utxo,
            fee_utxo,
            fee_amount,
            price_at_current_block_height,
            filler_amount_to_burn,
            oracle_signature,
        },
        &DcdContractContext {
            dcd_taproot_pubkey_gen: dcd_taproot_pubkey_gen.clone(),
            dcd_arguments: dcd_arguments.clone(),
            base_contract_context,
        },
    )
    .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    broadcast_or_get_raw_tx(is_offline, &transaction)?;

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
    crate::common::store::utils::save_dcd_args(taproot_pubkey_gen, dcd_arguments)?;
    Ok(())
}
