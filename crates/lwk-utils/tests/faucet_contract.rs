use anyhow::anyhow;
use elements::bitcoin::secp256k1;
use elements::hashes::Hash;
use elements::hex::ToHex;
use elements::schnorr::Keypair;
use elements::secp256k1_zkp::rand::thread_rng;
use elements::secp256k1_zkp::{PublicKey, Secp256k1};
use lwk_common::Signer;
use lwk_wollet::WalletTx;
use lwk_wollet::elements::{Transaction, TxInWitness};
use serde::Serialize;
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
    RunnerLogLevel, control_block, get_and_verify_env, get_new_asset_entropy, get_p2pk_address, get_p2pk_program,
    get_random_seed, run_program,
};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct TxInfo {
    pub outpoint: OutPoint,
    pub wallet_tx: WalletTx,
}

impl TxInfo {
    fn obtain_tx_out(&self) -> TxOut {
        self.wallet_tx.tx.output[self.outpoint.vout as usize].clone()
    }

    #[inline]
    pub fn obtain_token_value(&self, asset: &AssetId) -> anyhow::Result<u64> {
        println!("{:?}, asset: {asset}", self.wallet_tx.balance.get(asset));
        self.wallet_tx
            .balance
            .get(asset)
            .map(|x| *x as u64)
            .ok_or_else(|| anyhow::anyhow!("No value in utxo, check it, signed tx values for asset: {asset:?}"))
    }
}

#[expect(clippy::too_many_arguments)]
pub async fn issue_asset(
    signer: &Keypair,
    blinding_key: PublicKey,
    fee_tx_info: TxInfo,
    issue_amount: u64,
    fee_amount: u64,
    address_params: &'static AddressParams,
    lbtc_asset: AssetId,
    genesis_block_hash: simplicity::elements::BlockHash,
) -> anyhow::Result<PartiallySignedTransaction> {
    let fee_utxo_tx_out = fee_tx_info.obtain_tx_out();
    println!("fee_tx_out: {:?}", fee_utxo_tx_out);
    let total_input_lbtc_value = fee_tx_info.obtain_token_value(&lbtc_asset)?;

    if fee_amount > total_input_lbtc_value {
        return Err(anyhow!(
            "fee exceeds fee input value, fee_input: {fee_amount}, total_input_fee: {total_input_lbtc_value}"
        ));
    }

    let asset_entropy = get_random_seed();
    let asset_entropy_to_return = get_new_asset_entropy(&fee_tx_info.outpoint, asset_entropy).to_hex();

    let mut issuance_tx = Input::from_prevout(fee_tx_info.outpoint);
    issuance_tx.witness_utxo = Some(fee_utxo_tx_out.clone());
    issuance_tx.issuance_value_amount = Some(issue_amount);
    issuance_tx.issuance_inflation_keys = Some(1);
    issuance_tx.issuance_asset_entropy = Some(asset_entropy);

    let (asset_id, reissuance_asset_id) = issuance_tx.issuance_ids();

    let change_recipient = get_p2pk_address(&signer.x_only_public_key().0, address_params)?;

    let mut inp_txout_sec = std::collections::HashMap::new();
    let mut pst = PartiallySignedTransaction::new_v2();

    // Issuance token input
    {
        let issuance_secrets = TxOutSecrets {
            asset_bf: AssetBlindingFactor::zero(),
            value_bf: ValueBlindingFactor::zero(),
            value: total_input_lbtc_value,
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
        total_input_lbtc_value - fee_amount,
        lbtc_asset,
        None,
    ));

    // Fee
    pst.add_output(Output::from_txout(TxOut::new_fee(fee_amount, lbtc_asset)));

    pst.blind_last(&mut thread_rng(), &Secp256k1::new(), &inp_txout_sec)?;

    let tx = finalize_p2pk_transaction(
        pst.extract_tx()?,
        std::slice::from_ref(&fee_utxo_tx_out.clone()),
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
    signer: &Keypair,
    input_index: usize,
    params: &'static AddressParams,
    genesis_hash: lwk_wollet::elements::BlockHash,
) -> anyhow::Result<Transaction> {
    let x_only_public_key = signer.x_only_public_key().0;
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
    keypair: &Keypair,
    env: &ElementsEnv<Arc<Transaction>>,
    runner_log_level: RunnerLogLevel,
) -> anyhow::Result<Arc<RedeemNode<Elements>>> {
    let sighash_all = secp256k1::Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());

    let witness_values = simplicityhl::WitnessValues::from(HashMap::from([(
        WitnessName::from_str_unchecked("SIGNATURE"),
        Value::byte_array(keypair.sign_schnorr(sighash_all).serialize()),
    )]));

    Ok(run_program(compiled_program, witness_values, env, runner_log_level)?.0)
}

// pub fn fetch_utxo(outpoint: OutPoint) -> anyhow::Result<TxOut> {
//     // Check file cache first
//     let txid_str = outpoint.txid.to_string();
//     let cache_path = cache_path_for_txid(&txid_str)?;
//     if cache_path.exists() {
//         let cached_hex = fs::read_to_string(&cache_path)?;
//         return extract_utxo(&cached_hex, outpoint.vout as usize);
//     }
//
//     let url = format!(
//         "https://blockstream.info/liquidtestnet/api/tx/{}/hex",
//         outpoint.txid
//     );
//
//     let client = Client::builder().timeout(Duration::from_secs(10)).build()?;
//
//     let tx_hex = client.get(&url).send()?.error_for_status()?.text()?;
//     // Persist to cache best-effort
//     if let Err(_e) = fs::write(&cache_path, &tx_hex) {
//         // Ignore cache write errors
//     }
//     extract_utxo(&tx_hex, outpoint.vout as usize)
// }
