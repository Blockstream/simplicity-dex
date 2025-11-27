use crate::common::keys::derive_keypair_from_index;
use crate::common::settings::Settings;
use crate::common::store::SledError;
use crate::common::store::utils::OrderParams;
use crate::contract_handlers::common::{broadcast_or_get_raw_tx, get_order_params};
use contracts::DCDArguments;
use contracts_adapter::dcd::{
    BaseContractContext, CommonContext, DcdContractContext, DcdManager, MakerTerminationSettlementContext,
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
    grantor_settlement_amount_to_burn: u64,
}

#[derive(Debug)]
pub struct ArgsToSave {
    taproot_pubkey_gen: TaprootPubkeyGen,
    dcd_arguments: DCDArguments,
}

#[derive(Debug)]
pub struct Utxos {
    pub fee: OutPoint,
    pub settlement_asset: OutPoint,
    pub grantor_settlement_token: OutPoint,
}

#[instrument(level = "debug", skip_all, err)]
pub async fn process_args(
    account_index: u32,
    grantor_settlement_amount_to_burn: u64,
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
        grantor_settlement_amount_to_burn,
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
        grantor_settlement_amount_to_burn,
    }: ProcessedArgs,
    Utxos {
        fee: fee_utxo,
        settlement_asset: settlement_asset_utxo,
        grantor_settlement_token: grantor_settlement_token_utxo,
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

    let transaction = DcdManager::maker_settlement_termination(
        &CommonContext { keypair },
        MakerTerminationSettlementContext {
            settlement_asset_utxo,
            grantor_settlement_token_utxo,
            fee_utxo,
            fee_amount,
            grantor_settlement_amount_to_burn,
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
