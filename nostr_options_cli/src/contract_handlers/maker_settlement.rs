use dcd_manager::manager::init::DcdManager;
use simplicityhl::elements::AddressParams;
use simplicityhl_core::LIQUID_TESTNET_GENESIS;

pub fn handle() -> crate::error::Result<()> {
    DcdManager::maker_settlement(&AddressParams::LIQUID_TESTNET, *LIQUID_TESTNET_GENESIS)
        .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    Ok(())
}
