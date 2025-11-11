use crate::manager::common::obtain_utxo_value;
use crate::manager::types::AssetEntropy;
use anyhow::bail;
use simplicity::bitcoin::secp256k1::Keypair;
use simplicity::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use simplicity::elements::hex::ToHex;
use simplicity::elements::pset::{Input, Output, PartiallySignedTransaction};
use simplicity::elements::{TxOut, TxOutSecrets};
use simplicityhl::elements;
use simplicityhl::elements::secp256k1_zkp::rand::thread_rng;
use simplicityhl::elements::secp256k1_zkp::{Secp256k1, SecretKey};
use simplicityhl::elements::{AddressParams, AssetId, OutPoint, Transaction};
use simplicityhl::simplicity::bitcoin::secp256k1;
use simplicityhl_core::{
    fetch_utxo, finalize_p2pk_transaction, get_new_asset_entropy, get_p2pk_address, get_random_seed,
};

pub fn handle(
    keypair: secp256k1::Keypair,
    fee_utxo: OutPoint,
    fee_amount: u64,
    issue_amount: u64,
    address_params: &'static AddressParams,
    change_asset: AssetId,
    genesis_block_hash: elements::BlockHash,
) -> anyhow::Result<(Transaction, AssetEntropy)> {
    let utxo_fee = fetch_utxo(fee_utxo)?;

    let fee_utxo_value = obtain_utxo_value(&utxo_fee)?;
    if fee_amount > fee_utxo_value {
        bail!("Fee exceeds fee input, fee_amount: {fee_amount}, total_input_fee: {fee_utxo_value}")
    }

    let blinding_key = secp256k1::Keypair::from_secret_key(secp256k1::SECP256K1, &SecretKey::from_slice(&[1; 32])?);

    let asset_entropy = get_random_seed();

    let mut issuance_info_input = Input::from_prevout(fee_utxo);
    issuance_info_input.witness_utxo = Some(utxo_fee.clone());
    issuance_info_input.issuance_value_amount = Some(issue_amount);
    issuance_info_input.issuance_inflation_keys = Some(1);
    issuance_info_input.issuance_asset_entropy = Some(asset_entropy);

    let (asset, reissuance_asset) = issuance_info_input.issuance_ids();
    tracing::info!("Asset: '{asset}', Reissuance Asset: '{reissuance_asset}'");
    let asset_entropy_to_return = get_new_asset_entropy(&fee_utxo, asset_entropy).to_hex();

    let change_recipient = get_p2pk_address(&keypair.x_only_public_key().0, &AddressParams::LIQUID_TESTNET)?;

    let mut pst = PartiallySignedTransaction::new_v2();

    {
        issuance_info_input.blinded_issuance = Some(0x00);
        pst.add_input(issuance_info_input);
    }
    {
        let mut output = Output::new_explicit(
            change_recipient.script_pubkey(),
            1,
            reissuance_asset,
            Some(blinding_key.public_key().into()),
        );
        output.blinder_index = Some(0);
        pst.add_output(output);
    }

    pst.add_output(Output::new_explicit(
        change_recipient.script_pubkey(),
        issue_amount,
        asset,
        None,
    ));

    pst.add_output(Output::new_explicit(
        change_recipient.script_pubkey(),
        fee_utxo_value - fee_amount,
        change_asset,
        None,
    ));

    pst.add_output(Output::from_txout(TxOut::new_fee(fee_amount, change_asset)));

    let issuance_secrets = TxOutSecrets {
        asset_bf: AssetBlindingFactor::zero(),
        value_bf: ValueBlindingFactor::zero(),
        value: fee_utxo_value,
        asset: change_asset,
    };

    let mut inp_txout_sec = std::collections::HashMap::new();
    inp_txout_sec.insert(0, issuance_secrets);

    pst.blind_last(&mut thread_rng(), &Secp256k1::new(), &inp_txout_sec)?;

    let tx = pst.extract_tx()?;
    let utxos_to_spend = std::slice::from_ref(&utxo_fee);

    let tx = finalize_p2pk_transaction(tx, utxos_to_spend, &keypair, 0, &address_params, genesis_block_hash)?;
    tx.verify_tx_amt_proofs(secp256k1::SECP256K1, utxos_to_spend)?;

    Ok((tx, asset_entropy_to_return))
}
