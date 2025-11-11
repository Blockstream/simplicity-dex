use crate::manager::types::AssetEntropy;
use anyhow::Context;
use simplicityhl::elements;
use simplicityhl::elements::bitcoin::secp256k1;
use simplicityhl::elements::{AddressParams, AssetId, OutPoint, Transaction};

pub struct DcdManager;

impl DcdManager {
    pub fn faucet(
        keypair: secp256k1::Keypair,
        fee_utxo: OutPoint,
        fee_amount: u64,
        issue_amount: u64,
        address_params: &'static AddressParams,
        change_asset: AssetId,
        genesis_block_hash: elements::BlockHash,
    ) -> anyhow::Result<(Transaction, AssetEntropy)> {
        crate::manager::handlers::faucet::handle(
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
