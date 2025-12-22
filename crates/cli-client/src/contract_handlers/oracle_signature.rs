use crate::common::keys::derive_keypair_from_index;
use crate::common::settings::Settings;
use contracts::oracle_msg;
use elements::bitcoin::secp256k1;
use elements::secp256k1_zkp::Message;
use nostr::prelude::Signature;
use simplicity::elements::secp256k1_zkp::PublicKey;

pub fn handle(
    index: u32,
    price_at_current_block_height: u64,
    settlement_height: u32,
) -> crate::error::Result<(PublicKey, Message, Signature)> {
    let settings = Settings::load()?;
    let keypair = derive_keypair_from_index(index, &settings.seed_hex);
    let pubkey = keypair.public_key();
    let msg = secp256k1::Message::from_digest_slice(&oracle_msg(settlement_height, price_at_current_block_height))?;
    let sig = secp256k1::SECP256K1.sign_schnorr(&msg, &keypair);
    Ok((pubkey, msg, sig))
}
