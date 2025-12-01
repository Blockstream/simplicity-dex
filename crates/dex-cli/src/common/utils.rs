use elements::hex::ToHex;
use hex::FromHex;
use simplicityhl::elements::AssetId;
use simplicityhl_core::broadcast_tx;
use std::io::Write;

pub const DEFAULT_CLIENT_TIMEOUT_SECS: u64 = 10;

pub(crate) fn write_into_stdout<T: AsRef<str> + std::fmt::Debug>(text: T) -> std::io::Result<usize> {
    let mut output = text.as_ref().to_string();
    output.push('\n');
    std::io::stdout().write(output.as_bytes())
}

pub(crate) fn broadcast_tx_inner(tx: &simplicityhl::elements::Transaction) -> crate::error::Result<String> {
    broadcast_tx(tx).map_err(|err| crate::error::CliError::Broadcast(err.to_string()))
}

pub(crate) fn decode_hex(str: impl AsRef<[u8]>) -> crate::error::Result<Vec<u8>> {
    let str_to_convert = str.as_ref();
    hex::decode(str_to_convert).map_err(|err| crate::error::CliError::FromHex(err, str_to_convert.to_hex()))
}

pub(crate) fn entropy_to_asset_id(el: impl AsRef<[u8]>) -> crate::error::Result<AssetId> {
    use simplicity::hashes::sha256;
    let el = el.as_ref();
    let mut asset_entropy_bytes =
        <[u8; 32]>::from_hex(el).map_err(|err| crate::error::CliError::FromHex(err, el.to_hex()))?;
    asset_entropy_bytes.reverse();
    let midstate = sha256::Midstate::from_byte_array(asset_entropy_bytes);
    Ok(AssetId::from_entropy(midstate))
}
