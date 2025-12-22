use crate::common::keys::derive_keypair_from_index;
use crate::common::settings::Settings;
use crate::contract_handlers::common::broadcast_or_get_raw_tx;
use simplicityhl::elements::{AddressParams, OutPoint, Txid};
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, get_p2pk_address};
use tokio::task;

pub async fn handle(
    account_index: u32,
    split_amount: u64,
    fee_utxo: OutPoint,
    fee_amount: u64,
    is_offline: bool,
) -> crate::error::Result<Txid> {
    task::spawn_blocking(move || handle_sync(account_index, split_amount, fee_utxo, fee_amount, is_offline)).await?
}

fn handle_sync(
    account_index: u32,
    split_amount: u64,
    fee_utxo: OutPoint,
    fee_amount: u64,
    is_offline: bool,
) -> crate::error::Result<Txid> {
    let settings = Settings::load().map_err(|err| crate::error::CliError::EnvNotSet(err.to_string()))?;
    let keypair = derive_keypair_from_index(account_index, &settings.seed_hex);

    let recipient_addr = get_p2pk_address(&keypair.x_only_public_key().0, &AddressParams::LIQUID_TESTNET).unwrap();
    let transaction = contracts_adapter::basic::split_native_three(
        &keypair,
        fee_utxo,
        &recipient_addr,
        split_amount,
        fee_amount,
        &AddressParams::LIQUID_TESTNET,
        LIQUID_TESTNET_BITCOIN_ASSET,
        *LIQUID_TESTNET_GENESIS,
    )
    .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    broadcast_or_get_raw_tx(is_offline, &transaction)?;

    Ok(transaction.txid())
}
