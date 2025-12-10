use anyhow::anyhow;
use elements::bitcoin::secp256k1;
use elements::hashes::Hash;
use elements::hex::ToHex;
use elements::schnorr::Keypair;
use elements::secp256k1_zkp::rand::thread_rng;
use elements::secp256k1_zkp::{PublicKey, Secp256k1};
use lwk_common::Signer;
use lwk_wollet::elements::{Transaction, TxInWitness};
use lwk_wollet::elements_miniscript::ToPublicKey;
use simplicity::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use simplicity::elements::pset::{Input, Output, PartiallySignedTransaction};
use simplicity::elements::{AddressParams, AssetId, OutPoint, TxOut, TxOutSecrets};
use simplicityhl::simplicity::RedeemNode;
use simplicityhl::simplicity::jet::Elements;
use simplicityhl::simplicity::jet::elements::ElementsEnv;
use simplicityhl::str::WitnessName;
use simplicityhl::value::ValueConstructible;
use simplicityhl::{CompiledProgram, Value};
use simplicityhl_core::{
    RunnerLogLevel, control_block, fetch_utxo, get_and_verify_env, get_new_asset_entropy, get_p2pk_address,
    get_p2pk_program, get_random_seed, obtain_utxo_value, run_program,
};
use std::collections::HashMap;
use std::sync::Arc;

#[expect(clippy::too_many_arguments)]
pub async fn issue_asset(
    signer: &impl Signer,
    blinding_key: PublicKey,
    fee_utxo_outpoint: OutPoint,
    issue_amount: u64,
    fee_amount: u64,
    address_params: &'static AddressParams,
    lbtc_asset: AssetId,
    genesis_block_hash: simplicity::elements::BlockHash,
) -> anyhow::Result<PartiallySignedTransaction> {
    let fee_utxo_tx_out = fetch_utxo(fee_utxo_outpoint)?;

    let total_input_fee = obtain_utxo_value(&fee_utxo_tx_out)?;
    if fee_amount > total_input_fee {
        return Err(anyhow!(
            "fee exceeds fee input value, fee_input: {fee_amount}, total_input_fee: {total_input_fee}"
        ));
    }

    let asset_entropy = get_random_seed();
    let asset_entropy_to_return = get_new_asset_entropy(&fee_utxo_outpoint, asset_entropy).to_hex();

    let mut issuance_tx = Input::from_prevout(fee_utxo_outpoint);
    issuance_tx.witness_utxo = Some(fee_utxo_tx_out.clone());
    issuance_tx.issuance_value_amount = Some(issue_amount);
    issuance_tx.issuance_inflation_keys = Some(1);
    issuance_tx.issuance_asset_entropy = Some(asset_entropy);

    let (asset_id, reissuance_asset_id) = issuance_tx.issuance_ids();

    let change_recipient = get_p2pk_address(
        &get_x_only_pubkey_from_signer(signer)?.x_only_public_key().0,
        address_params,
    )?;

    let mut inp_txout_sec = std::collections::HashMap::new();
    let mut pst = PartiallySignedTransaction::new_v2();

    // Issuance token input
    {
        let issuance_secrets = TxOutSecrets {
            asset_bf: AssetBlindingFactor::zero(),
            value_bf: ValueBlindingFactor::zero(),
            value: fee_utxo_tx_out.value.explicit().unwrap(),
            asset: lbtc_asset,
        };

        issuance_tx.blinded_issuance = Some(0x00);
        pst.add_input(issuance_tx);

        inp_txout_sec.insert(0, issuance_secrets);
    }

    // Passing Reissuance token to new tx_out
    {
        let mut output = Output::new_explicit(
            change_recipient.script_pubkey(),
            1,
            reissuance_asset_id,
            Some(blinding_key.into()),
        );
        output.blinder_index = Some(0);
        pst.add_output(output);
    }

    //  Defining the amount of token issuance
    pst.add_output(Output::new_explicit(
        change_recipient.script_pubkey(),
        issue_amount,
        asset_id,
        None,
    ));

    // Change
    pst.add_output(Output::new_explicit(
        change_recipient.script_pubkey(),
        total_input_fee - fee_amount,
        lbtc_asset,
        None,
    ));

    // Fee
    pst.add_output(Output::from_txout(TxOut::new_fee(fee_amount, lbtc_asset)));

    pst.blind_last(&mut thread_rng(), &Secp256k1::new(), &inp_txout_sec)?;

    let tx = finalize_p2pk_transaction(
        pst.extract_tx()?,
        std::slice::from_ref(&fee_utxo_tx_out),
        signer,
        0,
        address_params,
        genesis_block_hash,
    )?;

    tx.verify_tx_amt_proofs(secp256k1::SECP256K1, &[fee_utxo_tx_out])?;
    Ok(pst)
}

fn get_x_only_pubkey_from_signer(signer: &impl Signer) -> anyhow::Result<PublicKey> {
    Ok(signer
        .xpub()
        .map_err(|err| anyhow::anyhow!("xpub forming error, err: {err:?}"))?
        .public_key)
}

pub fn finalize_p2pk_transaction(
    mut tx: Transaction,
    utxos: &[TxOut],
    signer: &impl Signer,
    input_index: usize,
    params: &'static AddressParams,
    genesis_hash: lwk_wollet::elements::BlockHash,
) -> anyhow::Result<Transaction> {
    let x_only_public_key = get_x_only_pubkey_from_signer(signer)?.x_only_public_key().0;
    let p2pk_program = get_p2pk_program(&x_only_public_key)?;

    let env = get_and_verify_env(
        &tx,
        &p2pk_program,
        &x_only_public_key,
        utxos,
        params,
        genesis_hash,
        input_index,
    )?;

    let pruned = execute_p2pk_program(&p2pk_program, signer, &env, RunnerLogLevel::None)?;

    let (simplicity_program_bytes, simplicity_witness_bytes) = pruned.to_vec_with_witness();
    let cmr = pruned.cmr();

    tx.input[input_index].witness = TxInWitness {
        amount_rangeproof: None,
        inflation_keys_rangeproof: None,
        script_witness: vec![
            simplicity_witness_bytes,
            simplicity_program_bytes,
            cmr.as_ref().to_vec(),
            control_block(cmr, x_only_public_key).serialize(),
        ],
        pegin_witness: vec![],
    };

    Ok(tx)
}

pub fn execute_p2pk_program(
    compiled_program: &CompiledProgram,
    keypair: &impl Signer,
    env: &ElementsEnv<Arc<Transaction>>,
    runner_log_level: RunnerLogLevel,
) -> anyhow::Result<Arc<RedeemNode<Elements>>> {
    let sighash_all = secp256k1::Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());

    let witness_values = simplicityhl::WitnessValues::from(HashMap::from([(
        WitnessName::from_str_unchecked("SIGNATURE"),
        // TODO: sighash has to be signed
        Value::byte_array(keypair.sign_message(sighash_all).serialize()),
    )]));

    Ok(run_program(compiled_program, witness_values, env, runner_log_level)?.0)
}
