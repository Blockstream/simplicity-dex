use crate::common::keys::derive_keypair_from_index;
use crate::common::settings::Settings;
use crate::common::store::SledError;
use crate::common::store::utils::OrderParams;
use crate::contract_handlers::common::{broadcast_or_get_raw_tx, get_order_params};
use crate::error::CliError;
use coin_selection::sqlite_db::SqliteRepo;
use coin_selection::types::{CoinSelectionStorage, DcdParamsStorage, GetTokenFilter, OutPointInfoRaw};
use contracts::DCDArguments;
use contracts_adapter::dcd::{BaseContractContext, CommonContext, DcdContractContext, DcdManager, TakerFundingContext};
use dex_nostr_relay::relay_processor::RelayProcessor;
use elements::bitcoin::secp256k1;
use elements::hex::ToHex;
use elements::schnorr::Keypair;
use nostr::EventId;
use simplicity::elements::OutPoint;
use simplicityhl::elements::{AddressParams, Txid};
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, TaprootPubkeyGen, get_p2pk_address};
use tokio::task;
use tracing::instrument;

#[derive(Debug)]
pub struct ProcessedArgs {
    pub keypair: secp256k1::Keypair,
    pub dcd_arguments: DCDArguments,
    pub dcd_taproot_pubkey_gen: String,
    pub collateral_amount_to_deposit: u64,
}

#[derive(Debug, Clone)]
pub struct ArgsToSave {
    taproot_pubkey_gen: TaprootPubkeyGen,
    dcd_arguments: DCDArguments,
}

#[derive(Debug, Clone)]
pub struct Utxos {
    pub filler_token_utxo: OutPoint,
    pub collateral_token_utxo: OutPoint,
}

fn merge_outpoints_sources(cli: Option<OutPoint>, mut cached: Vec<OutPointInfoRaw>) -> Option<OutPoint> {
    match cli {
        None => {
            if cached.is_empty() {
                None
            } else {
                Some(cached.remove(0).outpoint)
            }
        }
        Some(x) => Some(x),
    }
}

#[instrument(level = "debug", skip_all, err)]
pub async fn merge_utxos(
    sqlite_cache: &SqliteRepo,
    dcd_taproot_pubkey_gen: &str,
    filler_token_utxo: Option<OutPoint>,
    collateral_token_utxo: Option<OutPoint>,
    keypair: &Keypair,
) -> crate::error::Result<Utxos> {
    let dcd_arguments = sqlite_cache
        .get_dcd_params(dcd_taproot_pubkey_gen)
        .await
        .map_err(|err| CliError::SqliteCache(err.to_string()))?
        .unwrap();

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

    let change_recipient = get_p2pk_address(&keypair.x_only_public_key().0, &AddressParams::LIQUID_TESTNET).unwrap();

    let filler_token_utxo = merge_outpoints_sources(
        filler_token_utxo,
        sqlite_cache
            .get_token_outpoints(GetTokenFilter {
                asset_id: Some(dcd_arguments.filler_token_asset_id_hex_le.clone()),
                spent: Some(false),
                owner: Some(dcd_taproot_pubkey_gen.address.script_pubkey().to_hex()),
            })
            .await
            .map_err(|err| CliError::SqliteCache(err.to_string()))?,
    );

    let collateral_token_utxo = merge_outpoints_sources(
        collateral_token_utxo,
        sqlite_cache
            .get_token_outpoints(GetTokenFilter {
                asset_id: Some(dcd_arguments.collateral_asset_id_hex_le.clone()),
                spent: Some(false),
                owner: Some(change_recipient.script_pubkey().to_hex()),
            })
            .await
            .map_err(|err| CliError::SqliteCache(err.to_string()))?,
    );

    Ok(Utxos {
        filler_token_utxo: filler_token_utxo
            .ok_or_else(|| CliError::Custom("Can't find filler_token_utxo, pass it with cli".to_string()))?,
        collateral_token_utxo: collateral_token_utxo
            .ok_or_else(|| CliError::Custom("Can't find collateral_token_utxo, pass it with cli".to_string()))?,
    })
}

#[instrument(level = "debug", skip_all, err)]
pub async fn process_args(
    account_index: u32,
    collateral_amount_to_deposit: u64,
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
        collateral_amount_to_deposit,
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
        &contracts::get_dcd_address,
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
