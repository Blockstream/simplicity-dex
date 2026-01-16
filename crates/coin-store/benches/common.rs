use std::collections::HashMap;
use std::fs;
use std::str::FromStr;

use coin_store::executor::UtxoStore;
use coin_store::filter::UtxoFilter;
use coin_store::store::Store;

use simplicityhl::elements::confidential::{Asset, Nonce, Value as ConfidentialValue};
use simplicityhl::elements::hashes::Hash;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::elements::secp256k1_zkp::SecretKey;
use simplicityhl::elements::{AddressParams, AssetId, OutPoint, Script, Transaction, TxOut, TxOutWitness, Txid};
use simplicityhl::simplicity::bitcoin::key::Keypair;
use simplicityhl::simplicity::bitcoin::secp256k1::Secp256k1;

use simplicityhl_core::{LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_TEST_ASSET_ID_STR};

use contracts::options::OPTION_SOURCE;
use contracts::options::OptionsArguments;
use contracts::sdk::build_option_creation;
use contracts::sdk::taproot_pubkey_gen::{TaprootPubkeyGen, get_random_seed};

fn setup_tx_and_contract(
    keypair: &Keypair,
    start_time: u32,
    expiry_time: u32,
    collateral_per_contract: u64,
    settlement_per_contract: u64,
) -> Result<((PartiallySignedTransaction, TaprootPubkeyGen), OptionsArguments), Box<dyn std::error::Error>> {
    let option_outpoint = OutPoint::new(Txid::from_slice(&[1; 32])?, 0);
    let grantor_outpoint = OutPoint::new(Txid::from_slice(&[2; 32])?, 0);

    let issuance_asset_entropy = get_random_seed();

    let option_arguments = OptionsArguments::new(
        start_time,
        expiry_time,
        collateral_per_contract,
        settlement_per_contract,
        *LIQUID_TESTNET_BITCOIN_ASSET,
        AssetId::from_str(LIQUID_TESTNET_TEST_ASSET_ID_STR)?,
        issuance_asset_entropy,
        (option_outpoint, false),
        (grantor_outpoint, false),
    );

    let pst_and_tpg = build_option_creation(
        &keypair.public_key(),
        (
            option_outpoint,
            TxOut {
                asset: Asset::Explicit(*LIQUID_TESTNET_BITCOIN_ASSET),
                value: ConfidentialValue::Explicit(500),
                nonce: Nonce::Null,
                script_pubkey: Script::new(),
                witness: TxOutWitness::default(),
            },
        ),
        (
            grantor_outpoint,
            TxOut {
                asset: Asset::Explicit(*LIQUID_TESTNET_BITCOIN_ASSET),
                value: ConfidentialValue::Explicit(1000),
                nonce: Nonce::Null,
                script_pubkey: Script::new(),
                witness: TxOutWitness::default(),
            },
        ),
        &option_arguments,
        issuance_asset_entropy,
        100,
        &AddressParams::LIQUID_TESTNET,
    )?;

    Ok((pst_and_tpg, option_arguments))
}

pub async fn setup_db() -> (
    Store,
    (Vec<UtxoFilter>, Vec<UtxoFilter>, Vec<UtxoFilter>, Vec<UtxoFilter>),
    String,
) {
    let path = "/tmp/benchmark_stress.db";
    let _ = fs::remove_file(path);

    let mut filters_default = vec![];
    let mut filters_contracts = vec![];
    let mut filters_tokens = vec![];
    let mut filters_entropy = vec![];
    let store = Store::create(path).await.unwrap();

    for _i in 0..10 {
        let secp = Secp256k1::new();
        let keypair = Keypair::from_secret_key(&secp, &SecretKey::from_slice(&[1u8; 32]).unwrap());
        let secret_key = keypair.secret_key();

        let ((pst, tpg), opts_args) = setup_tx_and_contract(&keypair, 0, 0, 20, 25).unwrap();
        let tx: Transaction = pst.extract_tx().unwrap();

        let mut blinder_keys = HashMap::new();
        for (i, _output) in tx.output.iter().enumerate() {
            blinder_keys.insert(i, keypair);
        }
        store.insert_transaction(&tx, blinder_keys).await.unwrap();

        let source_code = OPTION_SOURCE;

        let args = opts_args.build_option_arguments();

        let tpg_for_db = tpg.clone();
        let tpg_for_filter = tpg.clone();
        let tpg_for_token = tpg;

        store.add_contract(source_code, args, tpg_for_db, None).await.unwrap();

        let option_asset_id = opts_args.option_token();

        store
            .insert_contract_token(&tpg_for_token.clone(), option_asset_id, "Name of token")
            .await
            .expect("");

        for output in tx.output.iter() {
            let asset_id = match output.asset {
                Asset::Explicit(id) => id,

                Asset::Confidential(_) => {
                    let unblinded = output.unblind(&secp, secret_key).expect("Missing blinding key");
                    unblinded.asset
                }

                _ => panic!("Match error"),
            };

            filters_default.push(UtxoFilter::new().asset_id(asset_id).required_value(1));

            filters_entropy.push(UtxoFilter::new().asset_id(asset_id).required_value(1).include_entropy());

            filters_contracts.push(
                UtxoFilter::new()
                    .asset_id(asset_id)
                    .required_value(1)
                    .include_entropy()
                    .taproot_pubkey_gen(tpg_for_filter.clone()),
            );

            filters_tokens.push(
                UtxoFilter::new()
                    .asset_id(asset_id)
                    .required_value(1)
                    .include_entropy()
                    .taproot_pubkey_gen(tpg_for_filter.clone())
                    .token_tag("Name of token"),
            );
        }
    }

    (
        store,
        (filters_default, filters_entropy, filters_contracts, filters_tokens),
        path.to_string(),
    )
}
