use crate::common::broadcast_tx_inner;
use crate::common::config::AggregatedConfig;
use crate::common::keys::derive_secret_key_from_index;
use crate::common::store::SledError;
use crate::common::store::utils::OrderParams;
use crate::contract_handlers::common::get_order_params;
use dex_nostr_relay::relay_processor::RelayProcessor;
use elements::bitcoin::hex::DisplayHex;
use elements::bitcoin::secp256k1;
use nostr::EventId;
use simplicity::elements::OutPoint;
use simplicity::elements::pset::serialize::Serialize;
use simplicity_contracts::DCDArguments;
use simplicity_contracts_adapter::dcd::{
    BaseContractContext, CommonContext, DcdContractContext, DcdManager, TakerFundingContext,
};
use simplicityhl::elements::{AddressParams, Txid};
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, TaprootPubkeyGen};
use tracing::instrument;

#[derive(Debug)]
pub struct ProcessedArgs {
    keypair: secp256k1::Keypair,
    dcd_arguments: DCDArguments,
    dcd_taproot_pubkey_gen: String,
    collateral_amount_to_deposit: u64,
}

#[derive(Debug)]
pub struct ArgsToSave {
    taproot_pubkey_gen: TaprootPubkeyGen,
    dcd_arguments: DCDArguments,
}

pub struct Utxos {
    pub filler_token_utxo: OutPoint,
    pub collateral_token_utxo: OutPoint,
}

#[instrument(level = "debug", skip_all, err)]
pub async fn process_args(
    account_index: u32,
    collateral_amount_to_deposit: u64,
    maker_order_event_id: EventId,
    relay_processor: &RelayProcessor,
    config: &AggregatedConfig,
) -> crate::error::Result<ProcessedArgs> {
    let keypair = secp256k1::Keypair::from_secret_key(
        secp256k1::SECP256K1,
        &derive_secret_key_from_index(account_index, config)?,
    );

    let order_params: OrderParams = get_order_params(maker_order_event_id, relay_processor).await?;

    Ok(ProcessedArgs {
        keypair,
        dcd_arguments: order_params.dcd_args,
        dcd_taproot_pubkey_gen: order_params.taproot_pubkey_gen,
        collateral_amount_to_deposit,
    })
}

#[instrument(level = "debug", skip_all, err)]
pub fn handle(
    ProcessedArgs {
        keypair,
        dcd_arguments,
        dcd_taproot_pubkey_gen,
        collateral_amount_to_deposit,
    }: ProcessedArgs,
    Utxos {
        filler_token_utxo,
        collateral_token_utxo,
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
        &simplicity_contracts::get_dcd_address,
    )
    .map_err(|e| SledError::TapRootGen(e.to_string()))?;

    let transaction = DcdManager::taker_funding(
        &CommonContext { keypair },
        TakerFundingContext {
            filler_token_utxo,
            collateral_token_utxo,
            fee_amount,
            collateral_amount_to_deposit,
        },
        &DcdContractContext {
            dcd_taproot_pubkey_gen: dcd_taproot_pubkey_gen.clone(),
            dcd_arguments: dcd_arguments.clone(),
            base_contract_context,
        },
    )
    .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    if is_offline {
        println!("{}", transaction.serialize().to_lower_hex_string());
    } else {
        println!("Broadcasted txid: {}", broadcast_tx_inner(&transaction)?);
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
    crate::common::store::utils::save_dcd_args(taproot_pubkey_gen, dcd_arguments)?;
    Ok(())
}
