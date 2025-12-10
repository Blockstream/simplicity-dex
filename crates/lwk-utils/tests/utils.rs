use global_utils::logger::{LoggerGuard, init_logger};
use lwk_common::{Signer, Singlesig, singlesig_desc};
use lwk_wollet::WolletDescriptor;
use std::sync::LazyLock;

pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(init_logger);

pub fn get_descriptor<S: Signer>(signer: &S) -> Result<WolletDescriptor, anyhow::Error> {
    let descriptor_str = singlesig_desc(signer, Singlesig::Wpkh, lwk_common::DescriptorBlindingKey::Slip77)
        .map_err(|e| anyhow::anyhow!("Invalid descriptor: {e}"))?;
    Ok(descriptor_str.parse()?)
}
