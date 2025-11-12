use crate::common::keys::derive_secret_key_from_index;
use crate::common::settings::Settings;
use crate::common::store::Store;
use crate::common::{broadcast_tx_inner, decode_hex};
use dcd_manager::manager::init::DcdManager;
use elements::bitcoin::hex::DisplayHex;
use elements::bitcoin::secp256k1;
use elements::hex::ToHex;
use simplicity::elements::OutPoint;
use simplicityhl::elements::AddressParams;
use simplicityhl::elements::pset::serialize::Serialize;
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS};

pub fn create_asset(
    account_index: u32,
    asset_name: String,
    fee_utxo: OutPoint,
    fee_amount: u64,
    issue_amount: u64,
    broadcast: bool,
) -> crate::error::Result<()> {
    let store = Store::load()?;

    if store.is_exist(&asset_name)? {
        return Err(crate::error::CliError::AssetNameExists { name: asset_name });
    };

    let settings = Settings::load().map_err(|err| crate::error::CliError::EnvNotSet(err.to_string()))?;
    let keypair = secp256k1::Keypair::from_secret_key(
        secp256k1::SECP256K1,
        &derive_secret_key_from_index(account_index, settings.clone()),
    );

    let (transaction, token_asset_entropy) = DcdManager::create_asset(
        keypair,
        fee_utxo,
        fee_amount,
        issue_amount,
        &AddressParams::LIQUID_TESTNET,
        LIQUID_TESTNET_BITCOIN_ASSET,
        *LIQUID_TESTNET_GENESIS,
    )
    .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    println!("Test token asset entropy: {}", token_asset_entropy);
    match broadcast {
        true => {
            println!("Broadcasted txid: {}", broadcast_tx_inner(&transaction)?);
            store.insert_value(asset_name, token_asset_entropy.as_bytes())?;
        }
        false => println!("{}", transaction.serialize().to_lower_hex_string()),
    }
    Ok(())
}

pub fn mint_asset(
    account_index: u32,
    asset_name: String,
    reissue_asset_utxo: OutPoint,
    fee_utxo: OutPoint,
    reissue_amount: u64,
    fee_amount: u64,
    broadcast: bool,
) -> crate::error::Result<()> {
    let store = Store::load()?;

    let Some(asset_entropy) = store.get_value(&asset_name)? else {
        return Err(crate::error::CliError::AssetNameExists { name: asset_name });
    };
    let asset_entropy = decode_hex(&asset_entropy)?;

    let settings = Settings::load().map_err(|err| crate::error::CliError::EnvNotSet(err.to_string()))?;
    let keypair = secp256k1::Keypair::from_secret_key(
        secp256k1::SECP256K1,
        &derive_secret_key_from_index(account_index, settings.clone()),
    );

    let transaction = DcdManager::mint_asset(
        keypair,
        fee_utxo,
        reissue_asset_utxo,
        reissue_amount,
        fee_amount,
        asset_entropy,
        &AddressParams::LIQUID_TESTNET,
        LIQUID_TESTNET_BITCOIN_ASSET,
        *LIQUID_TESTNET_GENESIS,
    )
    .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    match broadcast {
        true => println!("Broadcasted txid: {}", broadcast_tx_inner(&transaction)?),
        false => println!("{}", transaction.serialize().to_lower_hex_string()),
    }
    Ok(())
}
