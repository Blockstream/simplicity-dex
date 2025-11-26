use crate::common::config::AggregatedConfig;
use crate::common::keys::derive_secret_key_from_index;
use crate::common::store::Store;
use crate::common::{broadcast_tx_inner, decode_hex};
use elements::bitcoin::hex::DisplayHex;
use elements::bitcoin::secp256k1;
use simplicity::elements::OutPoint;
use simplicity::hashes::sha256::Midstate;
use simplicity_contracts_adapter::basic::{IssueAssetResponse, ReissueAssetResponse};
use simplicityhl::elements::AddressParams;
use simplicityhl::elements::pset::serialize::Serialize;
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, derive_public_blinder_key};

pub fn create_asset(
    account_index: u32,
    asset_name: String,
    fee_utxo: OutPoint,
    fee_amount: u64,
    issue_amount: u64,
    is_offline: bool,
    config: &AggregatedConfig,
) -> crate::error::Result<()> {
    let store = Store::load()?;

    if store.is_exist(&asset_name)? {
        return Err(crate::error::CliError::AssetNameExists { name: asset_name });
    }

    let keypair = secp256k1::Keypair::from_secret_key(
        secp256k1::SECP256K1,
        &derive_secret_key_from_index(account_index, config)?,
    );
    let blinding_key = derive_public_blinder_key();

    let IssueAssetResponse {
        tx,
        asset_id,
        reissuance_asset_id,
        asset_entropy,
    } = simplicity_contracts_adapter::basic::issue_asset(
        &keypair,
        &blinding_key,
        fee_utxo,
        fee_amount,
        issue_amount,
        &AddressParams::LIQUID_TESTNET,
        LIQUID_TESTNET_BITCOIN_ASSET,
        *LIQUID_TESTNET_GENESIS,
    )
    .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    println!(
        "Test token asset entropy: '{asset_entropy}', asset_id: '{asset_id}', reissue_asset_id: '{reissuance_asset_id}'"
    );
    if is_offline {
        println!("{}", tx.serialize().to_lower_hex_string());
    } else {
        println!("Broadcasted txid: {}", broadcast_tx_inner(&tx)?);
        store.insert_value(asset_name, asset_entropy.as_bytes())?;
    }
    Ok(())
}

//#[allow(clippy::too_many_arguments)] todo ??
pub fn mint_asset(
    account_index: u32,
    asset_name: String,
    reissue_asset_utxo: OutPoint,
    fee_utxo: OutPoint,
    reissue_amount: u64,
    fee_amount: u64,
    is_offline: bool,
    config: &AggregatedConfig,
) -> crate::error::Result<()> {
    let store = Store::load()?;

    let Some(asset_entropy) = store.get_value(&asset_name)? else {
        return Err(crate::error::CliError::AssetNameExists { name: asset_name });
    };
    let asset_entropy = decode_hex(&asset_entropy)?;
    let asset_entropy = entropy_to_midstate(asset_entropy)?;

    let keypair = secp256k1::Keypair::from_secret_key(
        secp256k1::SECP256K1,
        &derive_secret_key_from_index(account_index, config)?,
    );
    let blinding_key = derive_public_blinder_key();
    let ReissueAssetResponse {
        tx,
        asset_id,
        reissuance_asset_id,
    } = simplicity_contracts_adapter::basic::reissue_asset(
        &keypair,
        &blinding_key,
        reissue_asset_utxo,
        fee_utxo,
        reissue_amount,
        fee_amount,
        asset_entropy,
        &AddressParams::LIQUID_TESTNET,
        LIQUID_TESTNET_BITCOIN_ASSET,
        *LIQUID_TESTNET_GENESIS,
    )
    .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    println!("Minting asset: '{asset_id}', Reissue asset id: '{reissuance_asset_id}'");
    if is_offline {
        println!("{}", tx.serialize().to_lower_hex_string());
    } else {
        println!("Broadcasted txid: {}", broadcast_tx_inner(&tx)?);
    }
    Ok(())
}

pub fn entropy_to_midstate(el: impl AsRef<[u8]>) -> crate::error::Result<Midstate> {
    use elements::hex::ToHex;
    use hex::FromHex;
    use simplicity::hashes::sha256;
    let el = el.as_ref();
    let mut asset_entropy_bytes =
        <[u8; 32]>::from_hex(el).map_err(|err| crate::error::CliError::FromHex(err, el.to_hex()))?;
    asset_entropy_bytes.reverse();
    let midstate = sha256::Midstate::from_byte_array(asset_entropy_bytes);
    Ok(midstate)
}
