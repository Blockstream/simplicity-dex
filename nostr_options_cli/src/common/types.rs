use crate::common::derive_oracle_pubkey;
use crate::contract_handlers::maker_init::InnerDcdInitParams;
use clap::Args;
use dcd_manager::manager::types::COLLATERAL_ASSET_ID;
use simplicityhl::elements::bitcoin::secp256k1;

#[derive(Debug, Clone, PartialEq, clap::Args)]
pub struct DCDCliArguments {
    // Time parameters
    /// Unix timestamp (seconds) when taker funding starts. Must be <= taker funding end.
    #[arg(long = "taker-funding-start-time")]
    pub taker_funding_start_time: u32,
    /// Unix timestamp (seconds) when taker funding ends. Must be >= taker funding start.
    #[arg(long = "taker-funding-end-time")]
    pub taker_funding_end_time: u32,
    /// Unix timestamp (seconds) when the contract expires.
    #[arg(long = "contract-expiry-time")]
    pub contract_expiry_time: u32,
    /// Unix timestamp (seconds) after which early termination is no longer allowed.
    #[arg(long = "early-termination-end-time")]
    pub early_termination_end_time: u32,
    /// Blockchain settlement height used for enforcing settlement conditions.
    #[arg(long = "settlement-height")]
    pub settlement_height: u32,

    // Pricing parameters
    /// Strike price used by the contract (in minimal units of the price asset).
    #[arg(long = "strike-price")]
    pub strike_price: u64,
    /// Incentive fee expressed in basis points (1 bp = 0.01%).
    #[arg(long = "incentive-basis-points")]
    pub incentive_basis_points: u64,

    // Additional params for DCDRatioArguments
    /// Principal collateral amount (in the collateral asset's minimal units).
    #[arg(long = "principal-collateral-amount")]
    pub principal_collateral_amount: u64,
    /// Number of filler tokens to provide per unit of principal collateral.
    #[arg(long = "filler-per-principal-collateral")]
    pub filler_per_principal_collateral: u64,

    // Oracle
    /// Oracle public key (secp256k1 PublicKey). If not provided, a default derived
    /// public key is used when available.
    #[arg(long = "oracle-pubkey", default_value_t = derive_oracle_pubkey().unwrap())]
    pub oracle_public_key: secp256k1::PublicKey,
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
    /// Oracle public key to use for this init. Defaults to a locally derived key if omitted.
    #[arg(long = "oracle-pubkey", default_value_t = derive_oracle_pubkey().unwrap())]
    oracle_public_key: secp256k1::PublicKey,
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
