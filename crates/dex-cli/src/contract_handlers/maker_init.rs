use crate::common::entropy_to_asset_id;
use crate::common::keys::derive_keypair_from_index;
use crate::common::settings::Settings;
use crate::contract_handlers::common::broadcast_or_get_raw_tx;
use contracts::DCDArguments;
use contracts_adapter::dcd::{
    BaseContractContext, CreationContext, DcdInitParams, DcdInitResponse, DcdManager, FillerTokenEntropyHex,
    GrantorCollateralAssetEntropyHex, GrantorSettlementAssetEntropyHex, MakerInitContext,
};
use elements::bitcoin::secp256k1;
use simplicity::elements::OutPoint;
use simplicityhl::elements::{AddressParams, Txid};
use simplicityhl_core::{
    AssetEntropyHex, AssetIdHex, LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, TaprootPubkeyGen,
    derive_public_blinder_key,
};
use tokio::task;
use tracing::instrument;

#[derive(Debug)]
pub struct InnerDcdInitParams {
    pub taker_funding_start_time: u32,
    pub taker_funding_end_time: u32,
    pub contract_expiry_time: u32,
    pub early_termination_end_time: u32,
    pub settlement_height: u32,
    pub principal_collateral_amount: u64,
    pub incentive_basis_points: u64,
    pub filler_per_principal_collateral: u64,
    pub strike_price: u64,
    pub collateral_asset_id: AssetIdHex,
    pub settlement_asset_entropy: AssetEntropyHex,
    pub oracle_public_key: String,
}

#[derive(Debug)]
pub struct ProcessedArgs {
    keypair: secp256k1::Keypair,
    dcd_init_params: DcdInitParams,
}

pub struct ArgsToSave {
    pub filler_token_entropy: FillerTokenEntropyHex,
    pub grantor_collateral_token_entropy: GrantorCollateralAssetEntropyHex,
    pub grantor_settlement_token_entropy: GrantorSettlementAssetEntropyHex,
    pub taproot_pubkey: TaprootPubkeyGen,
    pub dcd_args: DCDArguments,
}

#[derive(Debug)]
pub struct Utxos {
    pub first: OutPoint,
    pub second: OutPoint,
    pub third: OutPoint,
}

impl TryInto<DcdInitParams> for InnerDcdInitParams {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<DcdInitParams, Self::Error> {
        Ok(DcdInitParams {
            taker_funding_start_time: self.taker_funding_start_time,
            taker_funding_end_time: self.taker_funding_end_time,
            contract_expiry_time: self.contract_expiry_time,
            early_termination_end_time: self.early_termination_end_time,
            settlement_height: self.settlement_height,
            principal_collateral_amount: self.principal_collateral_amount,
            incentive_basis_points: self.incentive_basis_points,
            filler_per_principal_collateral: self.filler_per_principal_collateral,
            strike_price: self.strike_price,
            collateral_asset_id: self.collateral_asset_id,
            settlement_asset_id: entropy_to_asset_id(self.settlement_asset_entropy)?.to_string(),
            oracle_public_key: self.oracle_public_key.clone(),
            // TODO(Illia): replace with actual data
            fee_script_hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            fee_basis_points: 0,
        })
    }
}

#[instrument(level = "debug", skip_all, err)]
pub fn process_args(account_index: u32, dcd_init_params: InnerDcdInitParams) -> crate::error::Result<ProcessedArgs> {
    let settings = Settings::load().map_err(|err| crate::error::CliError::EnvNotSet(err.to_string()))?;

    let keypair = derive_keypair_from_index(account_index, &settings.seed_hex);

    let dcd_init_params: DcdInitParams = dcd_init_params
        .try_into()
        .map_err(|err: anyhow::Error| crate::error::CliError::InnerDcdConversion(err.to_string()))?;

    Ok(ProcessedArgs {
        keypair,
        dcd_init_params,
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
        dcd_init_params,
    }: ProcessedArgs,
    Utxos {
        first: first_lbtc_utxo,
        second: second_lbtc_utxo,
        third: third_lbtc_utxo,
    }: Utxos,
    fee_amount: u64,
    is_offline: bool,
) -> crate::error::Result<(Txid, ArgsToSave)> {
    let DcdInitResponse {
        tx: transaction,
        filler_token_entropy,
        grantor_collateral_token_entropy,
        grantor_settlement_token_entropy,
        taproot_pubkey_gen,
        dcd_args,
    } = DcdManager::maker_init(
        &CreationContext {
            keypair,
            blinding_key: derive_public_blinder_key(),
        },
        MakerInitContext {
            input_utxos: [first_lbtc_utxo, second_lbtc_utxo, third_lbtc_utxo],
            dcd_init_params,
            fee_amount,
        },
        &BaseContractContext {
            address_params: &AddressParams::LIQUID_TESTNET,
            lbtc_asset: LIQUID_TESTNET_BITCOIN_ASSET,
            genesis_block_hash: *LIQUID_TESTNET_GENESIS,
        },
    )
    .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    println!(
        "Filler_token_entropy: '{}', grantor_collateral_entropy: '{}', grantor_settlement: '{}', taproot_pubkey: '{}', dcd_args: '{dcd_args:#?}'",
        filler_token_entropy, grantor_collateral_token_entropy, grantor_settlement_token_entropy, taproot_pubkey_gen
    );

    broadcast_or_get_raw_tx(is_offline, &transaction)?;

    let args_to_save = ArgsToSave {
        filler_token_entropy,
        grantor_collateral_token_entropy,
        grantor_settlement_token_entropy,
        taproot_pubkey: taproot_pubkey_gen,
        dcd_args,
    };
    Ok((transaction.txid(), args_to_save))
}

#[instrument(level = "debug", skip_all, err)]
pub fn save_args_to_cache(
    ArgsToSave {
        filler_token_entropy,
        grantor_collateral_token_entropy,
        grantor_settlement_token_entropy,
        taproot_pubkey,
        dcd_args,
    }: &ArgsToSave,
) -> crate::error::Result<()> {
    crate::common::store::utils::save_filler_token_entropy(taproot_pubkey, filler_token_entropy)?;
    crate::common::store::utils::save_grantor_collateral_token_entropy(
        taproot_pubkey,
        grantor_collateral_token_entropy,
    )?;
    crate::common::store::utils::save_grantor_settlement_token_entropy(
        taproot_pubkey,
        grantor_settlement_token_entropy,
    )?;
    crate::common::store::utils::save_dcd_args(taproot_pubkey, dcd_args)?;
    Ok(())
}
