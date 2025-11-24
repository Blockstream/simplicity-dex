use crate::common::settings::Settings;
use simplicityhl::elements::secp256k1_zkp as secp256k1;

/// # Panics
///
/// Will panic if `SEED_HEX` is in incorrect encoding that differs from hex
#[must_use]
pub fn derive_secret_key_from_index(index: u32, settings: Settings) -> secp256k1::SecretKey {
    // TODO (Oleks): fix possible panic, propagate error & move this parameter into config
    let seed_vec = hex::decode(settings.seed_hex).expect("SEED_HEX must be hex");
    assert_eq!(seed_vec.len(), 32, "SEED_HEX must be 32 bytes hex");

    let mut seed_bytes = [0u8; 32];
    seed_bytes.copy_from_slice(&seed_vec);

    let mut seed = seed_bytes;
    for (i, b) in index.to_be_bytes().iter().enumerate() {
        seed[24 + i] ^= *b;
    }
    secp256k1::SecretKey::from_slice(&seed).unwrap()
}
