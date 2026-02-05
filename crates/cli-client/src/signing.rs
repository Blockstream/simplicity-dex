use simplicityhl::elements::{Transaction, TxOut};
use simplicityhl::tracker::TrackerLogLevel;
use simplicityhl_core::{SimplicityNetwork, finalize_p2pk_transaction};

use crate::error::Error;
use crate::wallet::Wallet;

/// Sign multiple P2PK inputs in a transaction.
///
/// This helper function handles the common pattern of iterating over UTXO inputs,
/// signing each one with P2PK, and finalizing the transaction.
///
/// # Arguments
///
/// * `tx` - The transaction to sign
/// * `utxos` - The UTXOs being spent (must correspond to the transaction inputs)
/// * `wallet` - The wallet containing the signing key
/// * `network` - The Simplicity network to use
/// * `start_index` - The index of the first input to sign (allows skipping contract inputs)
///
/// # Returns
///
/// The transaction with all specified inputs signed.
///
/// # Errors
///
/// Returns an error if signing or finalization fails for any input.
pub fn sign_p2pk_inputs(
    mut tx: Transaction,
    utxos: &[TxOut],
    wallet: &Wallet,
    network: SimplicityNetwork,
    start_index: usize,
) -> Result<Transaction, Error> {
    for i in start_index..utxos.len() {
        let signature = wallet.signer().sign_p2pk(&tx, utxos, i, network)?;

        tx = finalize_p2pk_transaction(
            tx,
            utxos,
            &wallet.signer().public_key(),
            &signature,
            i,
            network,
            TrackerLogLevel::None,
        )?;
    }

    Ok(tx)
}
