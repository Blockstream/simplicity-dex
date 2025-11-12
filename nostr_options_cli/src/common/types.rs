use elements::hex::ToHex;
use simplicity_contracts::{DCDArguments, DCDRatioArguments};
use simplicityhl::elements::bitcoin::secp256k1;

#[derive(Debug, Clone, PartialEq, clap::Args)]
pub struct DCDCliArguments {
    // Time parameters
    pub taker_funding_start_time: u32,
    pub taker_funding_end_time: u32,
    pub contract_expiry_time: u32,
    pub early_termination_end_time: u32,
    pub settlement_height: u32,

    // Pricing parameters
    pub strike_price: u64,
    pub incentive_basis_points: u64,

    // Asset IDs (hex BE strings)
    pub collateral_asset_id_hex_be: String,
    pub settlement_asset_id_hex_be: String,
    pub filler_token_asset_id_hex_be: String,
    pub grantor_collateral_token_asset_id_hex_be: String,
    pub grantor_settlement_token_asset_id_hex_be: String,

    // Additional params for DCDRatioArguments
    principal_collateral_amount: u64,
    filler_per_principal_collateral: u64,

    // Oracle
    pub oracle_public_key: secp256k1::PublicKey,
}

fn be_hex_to_le_hex(be_hex: &str) -> Result<String, crate::error::CliError> {
    let mut bytes = hex::decode(be_hex).map_err(|err| crate::error::CliError::FromHex(err, be_hex.to_string()))?;
    bytes.reverse();
    Ok(hex::encode(bytes))
}

impl TryFrom<DCDCliArguments> for DCDArguments {
    type Error = crate::error::CliError;

    fn try_from(value: DCDCliArguments) -> Result<Self, Self::Error> {
        Ok(DCDArguments {
            taker_funding_start_time: value.taker_funding_start_time,
            taker_funding_end_time: value.taker_funding_end_time,
            contract_expiry_time: value.contract_expiry_time,
            early_termination_end_time: value.early_termination_end_time,
            settlement_height: value.settlement_height,
            strike_price: value.strike_price,
            incentive_basis_points: value.incentive_basis_points,
            collateral_asset_id_hex_le: be_hex_to_le_hex(&value.collateral_asset_id_hex_be)?,
            settlement_asset_id_hex_le: be_hex_to_le_hex(&value.settlement_asset_id_hex_be)?,
            filler_token_asset_id_hex_le: be_hex_to_le_hex(&value.filler_token_asset_id_hex_be)?,
            grantor_collateral_token_asset_id_hex_le: be_hex_to_le_hex(
                &value.grantor_collateral_token_asset_id_hex_be,
            )?,
            grantor_settlement_token_asset_id_hex_le: be_hex_to_le_hex(
                &value.grantor_settlement_token_asset_id_hex_be,
            )?,
            oracle_public_key: value.oracle_public_key.to_hex(),
            ratio_args: DCDRatioArguments::build_from(
                value.principal_collateral_amount,
                value.incentive_basis_points,
                value.strike_price,
                value.filler_per_principal_collateral,
            )
            .map_err(|err| crate::error::CliError::DcdRatioArgs(err.to_string()))?,
        })
    }
}
