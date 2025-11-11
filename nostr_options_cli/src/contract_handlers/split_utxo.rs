use crate::common::broadcast_tx_inner;
use crate::common::keys::derive_secret_key_from_index;
use crate::common::settings::Settings;
use dcd_manager::manager::init::DcdManager;
use elements::bitcoin::hex::DisplayHex;
use nostr::secp256k1;
use simplicityhl::elements::pset::serialize::Serialize;
use simplicityhl::elements::{AddressParams, OutPoint, Txid};
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS};

pub fn handle(
    account_index: u32,
    split_amount: u64,
    fee_utxo: OutPoint,
    fee_amount: u64,
    broadcast: bool,
) -> crate::error::Result<Txid> {
    let settings = Settings::load().map_err(|err| crate::error::CliError::EnvNotSet(err.to_string()))?;
    let keypair = secp256k1::Keypair::from_secret_key(
        secp256k1::SECP256K1,
        &derive_secret_key_from_index(account_index, settings.clone()),
    );

    let transaction = DcdManager::split_utxo_native(
        keypair,
        fee_utxo,
        split_amount,
        fee_amount,
        &AddressParams::LIQUID_TESTNET,
        LIQUID_TESTNET_BITCOIN_ASSET,
        *LIQUID_TESTNET_GENESIS,
    )
    .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    match broadcast {
        true => println!("Broadcasted txid: {}", broadcast_tx_inner(&transaction)?),
        false => println!("{}", transaction.serialize().to_lower_hex_string()),
    }
    Ok(transaction.txid())
}
