use crate::manager::types::AssetEntropyBytes;
use anyhow::anyhow;
use elements::bitcoin::secp256k1;
use elements::hashes::sha256;
use elements::secp256k1_zkp::SecretKey;
use simplicity::elements::TxOut;

pub const PUBLIC_SECRET_KEY: [u8; 32] = [1; 32];

#[inline]
pub fn obtain_utxo_value(tx_out: &TxOut) -> anyhow::Result<u64> {
    tx_out
        .value
        .explicit()
        .ok_or_else(|| anyhow!("No value in utxo, check it, tx_out: {tx_out:?}"))
}

pub fn derive_public_blinder_key() -> anyhow::Result<secp256k1::Keypair> {
    let blinder_key =
        secp256k1::Keypair::from_secret_key(secp256k1::SECP256K1, &SecretKey::from_slice(&PUBLIC_SECRET_KEY)?);
    Ok(blinder_key)
}

#[derive(Debug)]
pub struct AssetEntropyProcessed {
    pub entropy: sha256::Midstate,
    pub reversed_bytes: AssetEntropyBytes,
}

#[inline]
pub fn raw_asset_entropy_bytes_to_midstate(mut bytes: AssetEntropyBytes) -> AssetEntropyProcessed {
    bytes.reverse();
    AssetEntropyProcessed {
        entropy: sha256::Midstate::from_byte_array(bytes),
        reversed_bytes: bytes,
    }
}

pub fn convert_asset_entropy(val: impl AsRef<[u8]>) -> anyhow::Result<AssetEntropyBytes> {
    let asset_entropy_vec = val.as_ref().to_vec();
    let asset_entropy: AssetEntropyBytes = asset_entropy_vec.try_into().map_err(|x: Vec<u8>| {
        anyhow!(
            "Failed to parse asset entropy, got len: {}, has to be: {}",
            x.len(),
            AssetEntropyBytes::default().len()
        )
    })?;
    Ok(asset_entropy)
}
