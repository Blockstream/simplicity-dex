use anyhow::anyhow;
use simplicity::elements::TxOut;

#[inline]
pub fn obtain_utxo_value(tx_out: &TxOut) -> anyhow::Result<u64> {
    tx_out
        .value
        .explicit()
        .ok_or_else(|| anyhow!("No value in utxo, check it, tx_out: {tx_out:?}"))
}
