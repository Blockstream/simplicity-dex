use crate::common::broadcast_tx_inner;
use crate::common::config::AggregatedConfig;
use crate::contract_handlers::common::derive_keypair_from_config;
use elements::bitcoin::hex::DisplayHex;
use simplicityhl::elements::pset::serialize::Serialize;
use simplicityhl::elements::{AddressParams, OutPoint, Txid};
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, get_p2pk_address};

pub fn handle(
    account_index: u32,
    split_amount: u64,
    fee_utxo: OutPoint,
    fee_amount: u64,
    is_offline: bool,
    config: &AggregatedConfig,
) -> crate::error::Result<Txid> {
    let keypair = derive_keypair_from_config(account_index, config)?;
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

    if is_offline {
        println!("{}", transaction.serialize().to_lower_hex_string());
    } else {
        println!("Broadcasted txid: {}", broadcast_tx_inner(&transaction)?);
    }
    Ok(transaction.txid())
}
