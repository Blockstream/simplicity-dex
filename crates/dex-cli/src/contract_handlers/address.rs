use crate::common::config::AggregatedConfig;
use crate::contract_handlers::common::derive_keypair_from_config;
use elements::bitcoin::XOnlyPublicKey;
use simplicityhl::elements::{Address, AddressParams};
use simplicityhl_core::get_p2pk_address;

pub fn handle(index: u32, config: &AggregatedConfig) -> crate::error::Result<(XOnlyPublicKey, Address)> {
    let keypair = derive_keypair_from_config(index, config)?;
    let public_key = keypair.x_only_public_key().0;
    let address = get_p2pk_address(&public_key, &AddressParams::LIQUID_TESTNET)
        .map_err(|err| crate::error::CliError::P2pkAddress(err.to_string()))?;
    Ok((public_key, address))
}
