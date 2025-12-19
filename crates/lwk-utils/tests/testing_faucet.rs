mod faucet_contract;
mod p2pk_faucet;
mod utils;

use crate::faucet_contract::{TxInfo, issue_asset};
use crate::p2pk_faucet::faucet_p2pk_asset;
use crate::utils::{
    TEST_LOGGER, TestWollet, generate_signer, get_descriptor, test_client_electrum, test_client_esplora,
    wait_update_with_txs,
};
use elements::bitcoin::bip32::DerivationPath;
use elements::hex::ToHex;
use lwk_signer::SwSigner;
use lwk_test_util::{TestEnvBuilder, generate_view_key, regtest_policy_asset};
use lwk_wollet::asyncr::EsploraClient;
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::elements_miniscript::ToPublicKey;
use lwk_wollet::{ElementsNetwork, NoPersist, Wollet, WolletBuilder, WolletDescriptor};
use nostr::secp256k1::Secp256k1;
use simplicity::bitcoin::secp256k1::Keypair;
use simplicityhl::elements::secp256k1_zkp::PublicKey;
use simplicityhl::elements::{AddressParams, TxOut};
use simplicityhl_core::{
    LIQUID_TESTNET_BITCOIN_ASSET, LIQUID_TESTNET_GENESIS, derive_public_blinder_key, get_p2pk_address,
};
use std::str::FromStr;

const DEFAULT_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

#[tokio::test]
async fn test_issue_custom() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let _guard = &*TEST_LOGGER;
    let network = ElementsNetwork::LiquidTestnet;

    let sw_signer = SwSigner::new(DEFAULT_MNEMONIC, false)?;
    let mut sw_wallet = Wollet::new(network, NoPersist::new(), get_descriptor(&sw_signer).unwrap())?;
    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(&secp, &sw_signer.derive_xprv(&DerivationPath::master())?.private_key);

    let mut esplora_client = {
        // let url = match &self.inner {
        //     lwk_wollet::ElementsNetwork::Liquid => "https://blockstream.info/liquid/api",
        //     lwk_wollet::ElementsNetwork::LiquidTestnet => {
        //         "https://blockstream.info/liquidtestnet/api"
        //     }
        //     lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset: _ } => "127.0.0.1:3000",
        // };
        EsploraClient::new(
            ElementsNetwork::LiquidTestnet,
            "https://blockstream.info/liquidtestnet/api/",
        )
        // EsploraClient::new(ElementsNetwork::LiquidTestnet, "https://liquid.network/api/")
    };
    if let Some(update) = esplora_client.full_scan_to_index(&sw_wallet, 0).await? {
        sw_wallet.apply_update(update)?;
    }
    println!("address 0: {:?}", sw_wallet.address(Some(0)));
    println!("assets owned: {:?}", sw_wallet.assets_owned());
    println!("decriptor: {:?}", sw_wallet.wollet_descriptor());
    println!("transactions: {:?}", sw_wallet.transactions());
    println!("balance: {:?}", sw_wallet.balance());
    //
    // let pset = issue_asset(
    //     &keypair,
    //     derive_public_blinder_key().public_key(),
    //     outpoint,
    //     123456,
    //     500,
    //     &AddressParams::LIQUID_TESTNET,
    //     LIQUID_TESTNET_BITCOIN_ASSET,
    //     *LIQUID_TESTNET_GENESIS,
    // )
    // .await?;
    //
    // println!("pset: {:#?}", pset);

    Ok(())
}

#[test]
fn test_issue_custom2() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv().ok();
    let _guard = &*TEST_LOGGER;

    let secp = Secp256k1::new();
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let client = test_client_electrum(&env.electrum_url());

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let mut wallet = TestWollet::new(client, &desc);
    let keypair = Keypair::from_secret_key(&secp, &signer.derive_xprv(&DerivationPath::master())?.private_key);

    let address = wallet.wollet.address(Some(0))?;
    wallet.fund_btc(&env);

    let public_key = keypair.x_only_public_key().0;
    let p2pk_address = get_p2pk_address(&public_key, &AddressParams::LIQUID_TESTNET)?;

    let txid = env.elementsd_sendtoaddress(&p2pk_address, 1_000_011, None);
    println!("txid on p2pk address: {}", txid);

    // wallet.fund(
    //     &env,
    //     10_000_000,
    //     Some(address.address().clone()),
    //     Some(LIQUID_TESTNET_BITCOIN_ASSET),
    // );
    let utxos = wallet.wollet.utxos()?;
    println!("Utxos: {:?}", utxos);
    let asset_owned = wallet.wollet.assets_owned()?;
    println!("asset_owned: {:?}", asset_owned);
    let external_utxos = wallet.wollet.explicit_utxos()?;
    println!("external_utxos: {:?}", external_utxos);

    wallet.sync();

    let utxos = wallet.wollet.utxos()?;
    tracing::info!("Utxos after: {:?}", utxos);

    Ok(())
}

#[tokio::test]
async fn test_issue_custom2_p2pk() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv().ok();
    let _guard = &*TEST_LOGGER;

    let secp = Secp256k1::new();
    let env = TestEnvBuilder::from_env().with_esplora().build();
    let mut client = test_client_esplora(&env.esplora_url());

    let signer = generate_signer();
    let view_key = generate_view_key();
    let regtest_bitcoin_asset = regtest_policy_asset();

    let descriptor = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let network = ElementsNetwork::default_regtest();
    let descriptor: WolletDescriptor = descriptor.parse()?;
    let mut wollet = WolletBuilder::new(network, descriptor).build()?;
    let signer_keypair = Keypair::from_secret_key(&secp, &signer.derive_xprv(&DerivationPath::master())?.private_key);

    let update = client.full_scan(&wollet).await?;
    if let Some(update) = update {
        wollet.apply_update(update)?;
    }

    println!("Utxos1: {:?}", wollet.utxos()?);

    let address = wollet.address(None)?;
    let txid = env.elementsd_sendtoaddress(address.address(), 2_000_011, None);
    println!("txid sendtoaddress: {}", txid);

    env.elementsd_generate(10);
    let update = wait_update_with_txs(&mut client, &wollet).await;
    wollet.apply_update(update)?;
    println!("Utxos2: {:?}", wollet.utxos()?);

    let public_key = signer_keypair.x_only_public_key().0;
    let p2pk_address = get_p2pk_address(&public_key, &AddressParams::ELEMENTS)?;

    env.elementsd_generate(10);
    let update = client.full_scan(&wollet).await?;
    if let Some(update) = update {
        wollet.apply_update(update)?;
    }

    println!("Utxos3: {:?}", wollet.utxos()?);

    let msg = faucet_p2pk_asset(
        &mut client,
        &signer,
        &mut wollet,
        &p2pk_address,
        1_000_000,
        regtest_policy_asset(),
    )
    .await?;
    println!("txid on p2pk address: '{}', addr: {p2pk_address}", msg.0);
    wollet.apply_transaction(msg.1)?;
    println!("Utxos4: {:?}", wollet.utxos()?);

    let utxos = wollet.utxos()?;
    println!("Utxos5: {:?}", utxos);
    let asset_owned = wollet.assets_owned()?;
    println!("asset_owned: {:?}", asset_owned);
    let external_utxos = wollet.explicit_utxos()?;
    println!("external_utxos: {:?}", external_utxos);

    let update = client.full_scan(&wollet).await?;
    if let Some(update) = update {
        wollet.apply_update(update)?;
    }

    let utxos = wollet.utxos()?;
    tracing::info!("Utxos after: {:?}", utxos);

    // retrieve utxos from pt2tr wallet

    let view_key = view_key;
    let pk = signer_keypair.public_key().to_hex();
    let pubkey = PublicKey::from_str(&pk)?.to_x_only_pubkey();
    let p2pk_address = simplicityhl_core::get_p2pk_address(&pubkey, &AddressParams::ELEMENTS)?;
    println!("pk for p2pk_address: {}, p2pk addr: {}", pubkey, p2pk_address);

    let desc = format!("ct({view_key},elwpkh({pubkey}))");
    println!("desc: {}", desc);

    let descriptor: WolletDescriptor = desc.parse()?;
    let mut p2pk_wollet = WolletBuilder::new(network, descriptor).build()?;

    let update = client.full_scan(&p2pk_wollet).await?;
    if let Some(update) = update {
        p2pk_wollet.apply_update(update)?;
    }

    println!("Utxos p2pk1: {:?}", p2pk_wollet.utxos()?);

    assert!(p2pk_wollet.utxos().is_ok());
    assert_eq!(p2pk_wollet.utxos().unwrap().len(), 1);

    Ok(())
}

#[tokio::test]
async fn async_test_issue_custom2() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv().ok();
    let _guard = &*TEST_LOGGER;

    let secp = Secp256k1::new();
    let env = TestEnvBuilder::from_env().with_esplora().build();
    let mut client = test_client_esplora(&env.esplora_url());

    let signer = generate_signer();
    let view_key = generate_view_key();
    let regtest_bitcoin_asset = regtest_policy_asset();

    let descriptor = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let network = ElementsNetwork::default_regtest();
    let descriptor: WolletDescriptor = descriptor.parse()?;
    let mut wollet = WolletBuilder::new(network, descriptor).build()?;
    let keypair = Keypair::from_secret_key(&secp, &signer.derive_xprv(&DerivationPath::master())?.private_key);

    let update = client.full_scan(&wollet).await?.unwrap();
    wollet.apply_update(update)?;

    let address = wollet.address(None)?;
    let txid = env.elementsd_sendtoaddress(address.address(), 1_000_011, None);

    let update = wait_update_with_txs(&mut client, &wollet).await;
    wollet.apply_update(update)?;
    let tx = wollet.transaction(&txid)?.unwrap();
    assert!(tx.height.is_none());
    assert!(wollet.tip().timestamp().is_some());

    env.elementsd_generate(10);
    let update = wait_update_with_txs(&mut client, &wollet).await;
    wollet.apply_update(update)?;
    let tx = wollet.transaction(&txid)?.unwrap();

    assert!(tx.height.is_some());
    assert!(wollet.tip().timestamp().is_some());

    let utxos = wollet.utxos()?;
    println!("Utxos: {:#?}", utxos);
    let asset_owned = wollet.assets_owned()?;
    println!("asset_owned: {:?}", asset_owned);
    let external_utxos = wollet.explicit_utxos()?;
    println!("external_utxos: {:?}", external_utxos);

    let outpoint = utxos[0].outpoint;
    let wallet_tx = wollet.transaction(&outpoint.txid)?.unwrap();
    println!("wallet_tx: {:?}", wallet_tx);
    println!("signed balance: {:#?}", wallet_tx.balance);
    // println!("wallet_tx outs: {:?}", wallet_tx.outputs[0].unwrap().outpoint);

    let mut pset = issue_asset(
        &keypair,
        derive_public_blinder_key().public_key(),
        TxInfo { outpoint, wallet_tx },
        123456,
        500,
        &AddressParams::ELEMENTS,
        regtest_bitcoin_asset,
        *LIQUID_TESTNET_GENESIS,
    )
    .await?;

    let tx_to_send = wollet.finalize(&mut pset)?;
    client.broadcast(&tx_to_send).await?;

    env.elementsd_generate(10);
    let update = wait_update_with_txs(&mut client, &wollet).await;
    wollet.apply_update(update)?;

    let utxos = wollet.utxos()?;
    println!("[after] Utxos: {:?}", utxos);
    let asset_owned = wollet.assets_owned()?;
    println!("[after] asset_owned: {:?}", asset_owned);
    let external_utxos = wollet.explicit_utxos()?;
    println!("[after] external_utxos: {:?}", external_utxos);
    let wallet_tx = wollet.transaction(&utxos[0].outpoint.txid)?;
    println!("[after] wallet_tx: {:?}", wallet_tx.unwrap());

    Ok(())
}

#[tokio::test]
async fn get_addr() -> anyhow::Result<()> {
    let sw_signer = SwSigner::new(DEFAULT_MNEMONIC, false)?;
    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(&secp, &sw_signer.derive_xprv(&DerivationPath::master())?.private_key);

    let public_key = keypair.x_only_public_key().0;
    let address = simplicityhl_core::get_p2pk_address(&public_key, &AddressParams::LIQUID_TESTNET)?;
    println!("X Only Public Key: '{public_key}', P2PK Address: '{address}'");

    Ok(())
}
