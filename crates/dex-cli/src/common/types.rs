use crate::common::{derive_oracle_pubkey, entropy_to_asset_id};
use crate::contract_handlers::maker_init::InnerDcdInitParams;
use clap::Args;
use elements::hex::ToHex;
use simplicity_contracts::{DCDArguments, DCDRatioArguments};
use simplicity_contracts_adapter::dcd::COLLATERAL_ASSET_ID;
use simplicityhl::elements::bitcoin::secp256k1;
use simplicityhl_core::{AssetEntropyHex, AssetIdHex};
use tracing::instrument;

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

impl DcdCliAssets {
    /// Convert the CLI representation into the four asset IDs required by `DCDArguments`.
    /// The fourth returned ID is the settlement asset.
    pub(crate) fn to_asset_ids(
        &self,
    ) -> crate::error::Result<(
        AssetIdHex, // filler_token_asset_id_hex_le
        AssetIdHex, // grantor_collateral_token_asset_id_hex_le
        AssetIdHex, // grantor_settlement_token_asset_id_hex_le
        AssetIdHex, // settlement_asset_id_hex_le
    )> {
        match self {
            DcdCliAssets::AssetIds {
                filler_token_asset_id_hex_le,
                grantor_collateral_token_asset_id_hex_le,
                grantor_settlement_token_asset_id_hex_le,
                settlement_token_asset_id_hex_le,
            } => Ok((
                filler_token_asset_id_hex_le.clone(),
                grantor_collateral_token_asset_id_hex_le.clone(),
                grantor_settlement_token_asset_id_hex_le.clone(),
                settlement_token_asset_id_hex_le.clone(),
            )),
            DcdCliAssets::Entropies {
                filler_token_entropy_hex,
                grantor_collateral_token_entropy_hex,
                grantor_settlement_token_entropy_hex,
                settlement_token_asset_id_hex_le,
            } => Ok((
                entropy_to_asset_id(filler_token_entropy_hex.as_str())?.to_string(),
                entropy_to_asset_id(grantor_collateral_token_entropy_hex.as_str())?.to_string(),
                entropy_to_asset_id(grantor_settlement_token_entropy_hex.as_str())?.to_string(),
                entropy_to_asset_id(settlement_token_asset_id_hex_le.as_str())?.to_string(),
            )),
        }
    }
}

// Placeholder value parser for `DcdCliAssets`. Implement parsing logic as needed.
pub(crate) fn parse_dcd_cli_assets(_s: &str) -> Result<DcdCliAssets, String> {
    // TODO: implement real parser
    Err("parse_dcd_cli_assets is not implemented yet".to_string())
}

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
    /// Oracle public key (secp256k1 `PublicKey`). If not provided, a default derived
    /// public key is used when available.
    #[arg(long = "oracle-pubkey", default_value_t = derive_oracle_pubkey().unwrap())]
    pub oracle_public_key: secp256k1::PublicKey,

    /// Either three asset IDs or three asset entropies for the DCD legs.
    /// Parsing is handled by a custom parser (placeholder).
    #[arg(long = "dcd-assets", value_parser = parse_dcd_cli_assets)]
    pub dcd_assets: DcdCliAssets,
}

#[derive(Debug, Clone, PartialEq, clap::Args)]
pub struct DCDCliMakerFundArguments {
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
    /// Fee expressed in basis points (1 bp = 0.01%).
    #[arg(long = "fee-basis-points", default_value_t = 0)]
    pub fee_basis_points: u64,

    // Additional params for DCDRatioArguments
    /// Principal collateral amount (in the collateral asset's minimal units).
    #[arg(long = "principal-collateral-amount")]
    pub principal_collateral_amount: u64,
    /// Number of filler tokens to provide per unit of principal collateral.
    #[arg(long = "filler-per-principal-collateral")]
    pub filler_per_principal_collateral: u64,

    // Oracle
    /// Oracle public key (secp256k1 `PublicKey`). If not provided, a default derived
    /// public key is used when available.
    #[arg(long = "oracle-pubkey", default_value_t = derive_oracle_pubkey().unwrap())]
    pub oracle_public_key: secp256k1::PublicKey,

    // Entropies
    /// Settlement asset entropy as a hex string to be used for this order.
    #[arg(long = "filler-asset-entropy")]
    pub filler_asset_entropy: AssetEntropyHex,
    /// Settlement asset entropy as a hex string to be used for this order.
    #[arg(long = "grantor-collateral-asset-entropy")]
    pub grantor_collateral_asset_entropy: AssetEntropyHex,
    /// Settlement asset entropy as a hex string to be used for this order.
    #[arg(long = "grantor-settlement-asset-entropy")]
    pub grantor_settlement_asset_entropy: AssetEntropyHex,
    /// Settlement asset entropy as a hex string to be used for this order.
    #[arg(long = "settlement-asset-entropy")]
    pub settlement_asset_entropy: AssetEntropyHex,
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

impl DCDCliArguments {
    #[instrument(level = "debug", skip_all, err)]
    pub(crate) fn convert_to_dcd_arguments(self) -> crate::error::Result<DCDArguments> {
        let (
            filler_token_asset_id_hex_le,
            grantor_collateral_token_asset_id_hex_le,
            grantor_settlement_token_asset_id_hex_le,
            settlement_asset_id_hex_le,
        ) = self.dcd_assets.to_asset_ids()?;
        Ok(DCDArguments {
            taker_funding_start_time: self.taker_funding_start_time,
            taker_funding_end_time: self.taker_funding_end_time,
            contract_expiry_time: self.contract_expiry_time,
            early_termination_end_time: self.early_termination_end_time,
            settlement_height: self.settlement_height,
            strike_price: self.strike_price,
            incentive_basis_points: self.incentive_basis_points,
            filler_token_asset_id_hex_le,
            grantor_collateral_token_asset_id_hex_le,
            grantor_settlement_token_asset_id_hex_le,
            settlement_asset_id_hex_le,
            collateral_asset_id_hex_le: COLLATERAL_ASSET_ID.to_hex(),
            oracle_public_key: self.oracle_public_key.x_only_public_key().0.to_string(),
            ratio_args: DCDRatioArguments::build_from(
                self.principal_collateral_amount,
                self.incentive_basis_points,
                self.strike_price,
                self.filler_per_principal_collateral,
            )
            .map_err(|err| crate::error::CliError::DcdRatioArgs(err.to_string()))?,
            fee_basis_points: 0,
        })
    }
}

impl DCDCliMakerFundArguments {
    #[instrument(level = "debug", skip_all, err)]
    pub(crate) fn convert_to_dcd_arguments(&self) -> crate::error::Result<DCDArguments> {
        Ok(DCDArguments {
            taker_funding_start_time: self.taker_funding_start_time,
            taker_funding_end_time: self.taker_funding_end_time,
            contract_expiry_time: self.contract_expiry_time,
            early_termination_end_time: self.early_termination_end_time,
            settlement_height: self.settlement_height,
            strike_price: self.strike_price,
            incentive_basis_points: self.incentive_basis_points,
            filler_token_asset_id_hex_le: entropy_to_asset_id(&self.filler_asset_entropy)?.to_string(),
            grantor_collateral_token_asset_id_hex_le: entropy_to_asset_id(&self.grantor_collateral_asset_entropy)?
                .to_string(),
            grantor_settlement_token_asset_id_hex_le: entropy_to_asset_id(&self.grantor_settlement_asset_entropy)?
                .to_string(),
            settlement_asset_id_hex_le: entropy_to_asset_id(&self.settlement_asset_entropy)?.to_string(),
            collateral_asset_id_hex_le: COLLATERAL_ASSET_ID.to_hex(),
            oracle_public_key: self.oracle_public_key.x_only_public_key().0.to_string(),
            ratio_args: DCDRatioArguments::build_from(
                self.principal_collateral_amount,
                self.incentive_basis_points,
                self.strike_price,
                self.filler_per_principal_collateral,
            )
            .map_err(|err| crate::error::CliError::DcdRatioArgs(err.to_string()))?,
            fee_basis_points: self.fee_basis_points,
        })
    }
}
