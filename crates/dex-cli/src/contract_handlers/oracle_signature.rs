use crate::common::config::AggregatedConfig;
use crate::common::derive_public_oracle_keypair;
use crate::common::keys::derive_secret_key_from_index;
use contracts::oracle_msg;
use elements::bitcoin::secp256k1;
use elements::secp256k1_zkp::Message;
use nostr::prelude::Signature;
use simplicity::elements::secp256k1_zkp::PublicKey;

pub fn handle(
    index: Option<u32>,
    price_at_current_block_height: u64,
    settlement_height: u32,
    config: &AggregatedConfig,
) -> crate::error::Result<(PublicKey, Message, Signature)> {
    let keypair = match index {
        None => derive_public_oracle_keypair()?,
        Some(index) => {
            secp256k1::Keypair::from_secret_key(secp256k1::SECP256K1, &derive_secret_key_from_index(index, config)?)
        }
    };
    let pubkey = keypair.public_key();
    let msg = secp256k1::Message::from_digest_slice(&oracle_msg(settlement_height, price_at_current_block_height))?;
    let sig = secp256k1::SECP256K1.sign_schnorr(&msg, &keypair);
    Ok((pubkey, msg, sig))
}
