use crate::common::keys::derive_keypair_from_index;
use crate::common::settings::Settings;
use crate::common::store::Store;
use crate::contract_handlers::common::broadcast_or_get_raw_tx;
use contracts_adapter::basic::{IssueAssetResponse, ReissueAssetResponse};
use simplicity::elements::OutPoint;
use simplicity::hashes::sha256::Midstate;
use simplicityhl::elements::{AddressParams, Txid};
use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, derive_public_blinder_key};
use tokio::task;

pub async fn create_asset(
    account_index: u32,
    asset_name: String,
    fee_utxo: OutPoint,
    fee_amount: u64,
    issue_amount: u64,
    is_offline: bool,
) -> crate::error::Result<Txid> {
    task::spawn_blocking(move || {
        create_asset_sync(
            account_index,
            asset_name,
            fee_utxo,
            fee_amount,
            issue_amount,
            is_offline,
        )
    })
    .await?
}

fn create_asset_sync(
    account_index: u32,
    asset_name: String,
    fee_utxo: OutPoint,
    fee_amount: u64,
    issue_amount: u64,
    is_offline: bool,
) -> crate::error::Result<Txid> {
    let store = Store::load()?;

    if store.is_exist(&asset_name)? {
        return Err(crate::error::CliError::AssetNameExists { name: asset_name });
    }

    let settings = Settings::load().map_err(|err| crate::error::CliError::EnvNotSet(err.to_string()))?;
    let keypair = derive_keypair_from_index(account_index, &settings.seed_hex);
    let blinding_key = derive_public_blinder_key();

    let IssueAssetResponse {
        tx: transaction,
        asset_id,
        reissuance_asset_id,
        asset_entropy,
    } = contracts_adapter::basic::issue_asset(
        &keypair,
        &blinding_key,
        fee_utxo,
        issue_amount,
        fee_amount,
        &AddressParams::LIQUID_TESTNET,
        LIQUID_TESTNET_BITCOIN_ASSET,
        *LIQUID_TESTNET_GENESIS,
    )
    .map_err(|err| crate::error::CliError::DcdManager(err.to_string()))?;

    println!(
        "Test token asset entropy: '{asset_entropy}', asset_id: '{asset_id}', \
         reissue_asset_id: '{reissuance_asset_id}'"
    );
    broadcast_or_get_raw_tx(is_offline, &transaction)?;
    store.insert_value(asset_name, asset_entropy.as_bytes())?;

    Ok(transaction.txid())
}

pub async fn mint_asset(
    account_index: u32,
    asset_name: String,
    reissue_asset_utxo: OutPoint,
    fee_utxo: OutPoint,
    reissue_amount: u64,
    fee_amount: u64,
    is_offline: bool,
) -> crate::error::Result<Txid> {
    task::spawn_blocking(move || {
        mint_asset_sync(
            account_index,
            asset_name,
            reissue_asset_utxo,
            fee_utxo,
            reissue_amount,
            fee_amount,
            is_offline,
        )
    })
    .await?
}

fn mint_asset_sync(
    account_index: u32,
    asset_name: String,
    reissue_asset_utxo: OutPoint,
    fee_utxo: OutPoint,
    reissue_amount: u64,
    fee_amount: u64,
    is_offline: bool,
) -> crate::error::Result<Txid> {
    let store = Store::load()?;

    let Some(asset_entropy) = store.get_value(&asset_name)? else {
        return Err(crate::error::CliError::AssetNameExists { name: asset_name });
    };

    let asset_entropy = String::from_utf8(asset_entropy.to_vec())
        .map_err(|err| crate::error::CliError::Custom(format!("Failed to convert bytes to string, err: {err}")))?;
    let asset_entropy = entropy_to_midstate(&asset_entropy)?;

    let settings = Settings::load().map_err(|err| crate::error::CliError::EnvNotSet(err.to_string()))?;
    let keypair = derive_keypair_from_index(account_index, &settings.seed_hex);

    let blinding_key = derive_public_blinder_key();
    let ReissueAssetResponse {
        tx: transaction,
        asset_id,
        reissuance_asset_id,
    } = contracts_adapter::basic::reissue_asset(
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
    broadcast_or_get_raw_tx(is_offline, &transaction)?;

    Ok(transaction.txid())
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
