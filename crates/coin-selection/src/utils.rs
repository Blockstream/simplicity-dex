use crate::types::OutPointInfo;
use reqwest::Client;
use simplicity::elements::hex::ToHex;
use simplicityhl::elements::{TxIn, Txid};
use simplicityhl::simplicity::elements::{OutPoint, Transaction, TxOut, encode};
use std::time::Duration;

const BASE_URL: &str = "https://blockstream.info/liquidtestnet";

// TODO: reuse from simplicity-core
pub async fn fetch_tx(tx_id: Txid) -> anyhow::Result<Transaction> {
    let url = format!("{BASE_URL}/api/tx/{}/hex", tx_id);

    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    let tx_hex = client.get(&url).send().await?.error_for_status()?.text().await?;
    let tx_bytes = hex::decode(tx_hex.trim())?;
    let transaction: Transaction = encode::deserialize(&tx_bytes)?;
    Ok(transaction)
}

pub async fn extract_outpoint_info_from_tx_in(tx_in: TxIn) -> anyhow::Result<OutPointInfo> {
    let outpoint = tx_in.previous_output;
    let tx = fetch_tx(outpoint.txid).await?;
    let tx_out = tx.output[outpoint.vout as usize].clone();

    let info = OutPointInfo {
        outpoint,
        owner_script_pubkey: tx_out.script_pubkey.to_hex(),
        asset_id: tx_out.asset.to_string(),
        spent: true,
    };
    Ok(info)
}

pub async fn extract_outpoint_info_from_tx_out(outpoint: OutPoint, tx_out: TxOut) -> anyhow::Result<OutPointInfo> {
    Ok(OutPointInfo {
        outpoint,
        owner_script_pubkey: tx_out.script_pubkey.to_hex(),
        asset_id: tx_out.asset.to_string(),
        spent: false,
    })
}
