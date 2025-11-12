use crate::common::keys::derive_secret_key_from_index;
use crate::common::settings::Settings;
use crate::common::store::Store;
use crate::common::{broadcast_tx_inner, decode_hex};
use dcd_manager::manager::init::{DcdInitParams, DcdManager};
use dcd_manager::manager::types::{
    AssetEntropyHex, FillerTokenEntropyHex, GrantorCollateralAssetEntropyHex, GrantorSettlementAssetEntropyHex,
};
use elements::bitcoin::hex::DisplayHex;
use elements::bitcoin::secp256k1;
use simplicity::elements::OutPoint;
use simplicity::elements::pset::serialize::Serialize;
use simplicityhl::elements::{AddressParams, Txid};
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, TaprootPubkeyGen};

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
    pub collateral_asset_id: AssetEntropyHex,
    pub settlement_asset_id: AssetEntropyHex,
    pub oracle_public_key: secp256k1::PublicKey,
}

#[derive(Debug)]
pub struct ProcessedArgs {
    keypair: secp256k1::Keypair,
    dcd_init_params: DcdInitParams,
}

pub struct ArgsToSave {
    pub filler_token_entropy: FillerTokenEntropyHex,
    pub grantor_collateral_entropy: GrantorCollateralAssetEntropyHex,
    pub grantor_settlement: GrantorSettlementAssetEntropyHex,
    pub taproot_pubkey: TaprootPubkeyGen,
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
            collateral_asset_id: decode_hex(self.collateral_asset_id)?,
            settlement_asset_id: decode_hex(self.settlement_asset_id)?,
            oracle_public_key: self.oracle_public_key,
        })
    }
}

pub fn process_args(account_index: u32, dcd_init_params: InnerDcdInitParams) -> crate::error::Result<ProcessedArgs> {
    let store = Store::load()?;

    let settings = Settings::load().map_err(|err| crate::error::CliError::EnvNotSet(err.to_string()))?;

    let keypair = secp256k1::Keypair::from_secret_key(
        secp256k1::SECP256K1,
        &derive_secret_key_from_index(account_index, settings.clone()),
    );
    let dcd_init_params: DcdInitParams = dcd_init_params
        .try_into()
        .map_err(|err: anyhow::Error| crate::error::CliError::InnerDcdConversion(err.to_string()))?;

    Ok(ProcessedArgs {
        keypair,
        dcd_init_params,
    })
}

pub fn handle(
    ProcessedArgs {
        keypair,
        dcd_init_params,
    }: ProcessedArgs,
    input_lbtc_utxos: [OutPoint; 3],
    fee_amount: u64,
    broadcast: bool,
) -> crate::error::Result<(Txid, ArgsToSave)> {
    let (transaction, (filler_token_entropy, grantor_collateral_entropy, grantor_settlement), taproot_pubkey) =
        DcdManager::maker_init(
            keypair,
            input_lbtc_utxos,
            dcd_init_params,
            fee_amount,
            &AddressParams::LIQUID_TESTNET,
            LIQUID_TESTNET_BITCOIN_ASSET,
            *LIQUID_TESTNET_GENESIS,
        )
        .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    println!(
        "Filler_token_entropy: '{}', grantor_collateral_entropy: '{}', grantor_settlement: '{}', taproot_pubkey: '{}'",
        filler_token_entropy, grantor_collateral_entropy, grantor_settlement, taproot_pubkey
    );

    match broadcast {
        true => println!("Broadcasted txid: {}", broadcast_tx_inner(&transaction)?),
        false => println!("{}", transaction.serialize().to_lower_hex_string()),
    }
    let args_to_save = ArgsToSave {
        filler_token_entropy,
        grantor_collateral_entropy,
        grantor_settlement,
        taproot_pubkey,
    };
    Ok((transaction.txid(), args_to_save))
}

pub fn save_args_to_cache(
    ArgsToSave {
        filler_token_entropy,
        grantor_collateral_entropy,
        grantor_settlement,
        taproot_pubkey,
    }: ArgsToSave,
) -> crate::error::Result<()> {
    let store = Store::load()?;

    Ok(())
}
