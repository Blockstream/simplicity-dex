use crate::manager::types::{
    AssetEntropyBytes, AssetEntropyHex, FillerTokenEntropyHex, GrantorCollateralAssetEntropyHex,
    GrantorSettlementAssetEntropyHex,
};
use anyhow::anyhow;
use simplicity_contracts::DCDArguments;
use simplicityhl::elements;
use simplicityhl::elements::bitcoin::secp256k1;
use simplicityhl::elements::{AddressParams, AssetId, OutPoint, Transaction};
use simplicityhl_core::TaprootPubkeyGen;

pub struct DcdManager;

#[derive(Debug)]
pub struct DcdInitParams {
    pub taker_funding_start_time: u32,
    pub taker_funding_end_time: u32,
    pub contract_expiry_time: u32,
    pub early_termination_end_time: u32,
    pub settlement_height: u32,
    pub principal_collateral_amount: u64,
    pub incentive_basis_points: u64,
    pub filler_per_principal_collateral: u64,
    pub strike_price: u64,
    pub collateral_asset_id: Vec<u8>,
    pub settlement_asset_id: Vec<u8>,
    pub oracle_public_key: secp256k1::PublicKey,
}

impl DcdManager {
    pub fn create_asset(
        keypair: secp256k1::Keypair,
        fee_utxo: OutPoint,
        fee_amount: u64,
        issue_amount: u64,
        address_params: &'static AddressParams,
        change_asset: AssetId,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<(Transaction, AssetEntropyHex)> {
        crate::manager::handlers::faucet::handle_creation(
            keypair,
            fee_utxo,
            fee_amount,
            issue_amount,
            address_params,
            change_asset,
            genesis_block_hash,
        )
    }
    pub fn mint_asset(
        keypair: secp256k1::Keypair,
        fee_utxo: OutPoint,
        reissue_asset_utxo: OutPoint,
        reissue_amount: u64,
        fee_amount: u64,
        asset_entropy: impl AsRef<[u8]>,
        address_params: &'static AddressParams,
        change_asset: AssetId,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<Transaction> {
        {
            let asset_entropy = convert_asset_entropy(asset_entropy)?;
            crate::manager::handlers::faucet::handle_minting(
                keypair,
                fee_utxo,
                reissue_asset_utxo,
                reissue_amount,
                fee_amount,
                asset_entropy,
                address_params,
                change_asset,
                genesis_block_hash,
            )
        }
    }
    pub fn maker_init(
        keypair: secp256k1::Keypair,
        input_utxos: [OutPoint; 3],
        dcd_init_params: DcdInitParams,
        fee_amount: u64,
        address_params: &'static AddressParams,
        change_asset: AssetId,
        genesis_block_hash: simplicity::elements::BlockHash,
    ) -> anyhow::Result<(
        Transaction,
        (
            FillerTokenEntropyHex,
            GrantorCollateralAssetEntropyHex,
            GrantorSettlementAssetEntropyHex,
        ),
        TaprootPubkeyGen,
    )> {
        crate::manager::handlers::maker_init::handle(
            keypair,
            input_utxos,
            dcd_init_params.try_into()?,
            fee_amount,
            address_params,
            change_asset,
            genesis_block_hash,
        )
    }
    pub fn maker_funding(
        keypair: secp256k1::Keypair,
        filler_token_info: (OutPoint, impl AsRef<[u8]>),
        grantor_collateral_token_info: (OutPoint, impl AsRef<[u8]>),
        grantor_settlement_token_info: (OutPoint, impl AsRef<[u8]>),
        settlement_asset_info: (OutPoint, impl AsRef<[u8]>),
        fee_utxo: OutPoint,
        fee_amount: u64,
        dcd_arguments: DCDArguments,
        dcd_taproot_pubkey_gen: impl AsRef<str>,
        address_params: &'static AddressParams,
        change_asset: AssetId,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<Transaction> {
        let dcd_taproot_pubkey_gen = TaprootPubkeyGen::build_from_str(
            dcd_taproot_pubkey_gen.as_ref(),
            &dcd_arguments,
            address_params,
            &simplicity_contracts::get_dcd_address,
        )?;
        let filler_token_info = (filler_token_info.0, convert_asset_entropy(filler_token_info.1)?);
        let grantor_collateral_token_info = (
            grantor_collateral_token_info.0,
            convert_asset_entropy(grantor_collateral_token_info.1)?,
        );
        let grantor_settlement_token_info = (
            grantor_settlement_token_info.0,
            convert_asset_entropy(grantor_settlement_token_info.1)?,
        );
        let settlement_asset_info = (settlement_asset_info.0, convert_asset_entropy(settlement_asset_info.1)?);
        crate::manager::handlers::maker_funding::handle(
            keypair,
            filler_token_info,
            grantor_collateral_token_info,
            grantor_settlement_token_info,
            settlement_asset_info,
            fee_utxo,
            fee_amount,
            dcd_taproot_pubkey_gen,
            dcd_arguments,
            address_params,
            change_asset,
            genesis_block_hash,
        )
    }
    pub fn taker_funding(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::taker_funding::handle()?;
        Ok(())
    }
    pub fn taker_early_termination(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::taker_termination_early::handle()?;
        Ok(())
    }
    pub fn maker_collateral_termination(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::maker_termination_collateral::handle()?;
        Ok(())
    }
    pub fn maker_settlement_termination(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::maker_termination_settlement::handle()?;
        Ok(())
    }
    pub fn maker_settlement(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::maker_settlement::handle()?;
        Ok(())
    }
    pub fn taker_settlement(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::taker_settlement::handle()?;
        Ok(())
    }
    pub fn split_utxo_native(
        keypair: secp256k1::Keypair,
        fee_utxo: OutPoint,
        parts_to_split: u64,
        fee_amount: u64,
        address_params: &'static AddressParams,
        change_asset: AssetId,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<Transaction> {
        crate::manager::handlers::split_utxo::handle(
            keypair,
            fee_utxo,
            parts_to_split,
            fee_amount,
            address_params,
            change_asset,
            genesis_block_hash,
        )
    }
}

fn convert_asset_entropy(val: impl AsRef<[u8]>) -> anyhow::Result<AssetEntropyBytes> {
    let asset_entropy_vec = val.as_ref().to_vec();
    let asset_entropy: AssetEntropyBytes = asset_entropy_vec.try_into().map_err(|x: Vec<u8>| {
        anyhow!(
            "Failed to parse asset entropy, got len: {}, has to be: {}",
            x.len(),
            AssetEntropyBytes::default().len()
        )
    })?;
    Ok(asset_entropy)
}
