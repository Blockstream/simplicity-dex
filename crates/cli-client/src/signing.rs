use simplicityhl::elements::{AddressParams, Transaction, TxOut};
use simplicityhl_core::{LIQUID_TESTNET_GENESIS, finalize_p2pk_transaction};

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
/// * `params` - Address parameters for the network (must be static)
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
    params: &'static AddressParams,
    start_index: usize,
) -> Result<Transaction, Error> {
    for i in start_index..utxos.len() {
        let signature = wallet
            .signer()
            .sign_p2pk(&tx, utxos, i, params, *LIQUID_TESTNET_GENESIS)?;

        tx = finalize_p2pk_transaction(
            tx,
            utxos,
            &wallet.signer().public_key(),
            &signature,
            i,
            params,
            *LIQUID_TESTNET_GENESIS,
        )?;
    }

    Ok(tx)
}
