use crate::contract_handlers::maker_init::InnerDcdInitParams;
use clap::Args;
use contracts_adapter::dcd::COLLATERAL_ASSET_ID;
use simplicityhl_core::{AssetEntropyHex, AssetIdHex};

/// Represents either three asset IDs or three asset entropies as provided on the CLI.
/// This is intended to be parsed by a custom `clap` value parser (placeholder below).
#[derive(Debug, Clone, PartialEq)]
pub enum DcdCliAssets {
    /// Already-constructed asset IDs (little-endian hex strings).
    AssetIds {
        filler_token_asset_id_hex_le: AssetIdHex,
        grantor_collateral_token_asset_id_hex_le: AssetIdHex,
        grantor_settlement_token_asset_id_hex_le: AssetIdHex,
        settlement_token_asset_id_hex_le: AssetIdHex,
    },
    /// Entropies from which asset IDs will be derived.
    Entropies {
        filler_token_entropy_hex: AssetEntropyHex,
        grantor_collateral_token_entropy_hex: AssetEntropyHex,
        grantor_settlement_token_entropy_hex: AssetEntropyHex,
        settlement_token_asset_id_hex_le: AssetEntropyHex,
    },
}

#[derive(Debug, Args)]
pub struct InitOrderArgs {
    /// Taker funding start time as unix timestamp (seconds).
    #[arg(long = "taker-funding-start-time")]
    taker_funding_start_time: u32,
    /// Taker funding end time as unix timestamp (seconds).
    #[arg(long = "taker-funding-end-time")]
    taker_funding_end_time: u32,
    /// Contract expiry time as unix timestamp (seconds).
    #[arg(long = "contract-expiry-time")]
    contract_expiry_time: u32,
    /// Early termination deadline as unix timestamp (seconds).
    #[arg(long = "early-termination-end-time")]
    early_termination_end_time: u32,
    /// Settlement height used for final settlement.
    #[arg(long = "settlement-height")]
    settlement_height: u32,
    /// Principal collateral amount in minimal collateral units.
    #[arg(long = "principal-collateral-amount")]
    principal_collateral_amount: u64,
    /// Incentive fee in basis points (1 bp = 0.01%).
    #[arg(long = "incentive-basis-points")]
    incentive_basis_points: u64,
    /// Filler tokens per principal collateral unit.
    #[arg(long = "filler-per-principal-collateral")]
    filler_per_principal_collateral: u64,
    /// Strike price for the contract (minimal price asset units).
    #[arg(long = "strike-price")]
    strike_price: u64,
    /// Settlement asset entropy as a hex string to be used for this order.
    #[arg(long = "settlement-asset-entropy")]
    settlement_asset_entropy: String,
    /// Oracle public key to use for this init.
    #[arg(long = "oracle-pubkey")]
    oracle_public_key: String,
}

impl From<InitOrderArgs> for InnerDcdInitParams {
    fn from(args: InitOrderArgs) -> Self {
        InnerDcdInitParams {
            taker_funding_start_time: args.taker_funding_start_time,
            taker_funding_end_time: args.taker_funding_end_time,
            contract_expiry_time: args.contract_expiry_time,
            early_termination_end_time: args.early_termination_end_time,
            settlement_height: args.settlement_height,
            principal_collateral_amount: args.principal_collateral_amount,
            incentive_basis_points: args.incentive_basis_points,
            filler_per_principal_collateral: args.filler_per_principal_collateral,
            strike_price: args.strike_price,
            collateral_asset_id: COLLATERAL_ASSET_ID.to_string(),
            settlement_asset_entropy: args.settlement_asset_entropy,
            oracle_public_key: args.oracle_public_key,
        }
    }
}
