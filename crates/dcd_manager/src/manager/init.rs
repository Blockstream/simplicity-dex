use crate::manager::types::{AssetEntropyBytes, AssetEntropyHex};
use anyhow::{Context, anyhow};
use simplicityhl::elements;
use simplicityhl::elements::bitcoin::secp256k1;
use simplicityhl::elements::{AddressParams, AssetId, OutPoint, Transaction};

pub struct DcdManager;

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
        .context("Faucet handler failed")
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
        .context("Faucet handler failed")
    }
    pub fn maker_init(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::maker_init::handle().context("Maker init failed")?;
        Ok(())
    }
    pub fn maker_funding(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::maker_funding::handle().context("Maker funding failed")?;
        Ok(())
    }
    pub fn taker_funding(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::taker_funding::handle().context("Taker funding failed")?;
        Ok(())
    }
    pub fn taker_early_termination(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::taker_termination_early::handle().context("Taker early termination failed")?;
        Ok(())
    }
    pub fn maker_collateral_termination(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::maker_termination_collateral::handle()
            .context("Maker collateral termination failed")?;
        Ok(())
    }
    pub fn maker_settlement_termination(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::maker_termination_settlement::handle()
            .context("Maker settlement termination failed")?;
        Ok(())
    }
    pub fn maker_settlement(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::maker_settlement::handle().context("Maker settlement failed")?;
        Ok(())
    }
    pub fn taker_settlement(
        address_params: &'static AddressParams,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<()> {
        crate::manager::handlers::taker_settlement::handle().context("Taker settlement failed")?;
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
        .context("Splitting of Utxo failed")
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
