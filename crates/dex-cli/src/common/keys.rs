use simplicityhl::elements::secp256k1_zkp as secp256k1;

/// # Panics
///
/// Will panic if `SEED_HEX` is in incorrect encoding that differs from hex
#[must_use]
pub fn derive_secret_key_from_index(index: u32, seed_hex: impl AsRef<[u8]>) -> secp256k1::SecretKey {
    // TODO (Oleks): fix possible panic, propagate error & move this parameter into config
    let seed_vec = hex::decode(seed_hex).expect("SEED_HEX must be hex");
    assert_eq!(seed_vec.len(), 32, "SEED_HEX must be 32 bytes hex");

    let mut seed_bytes = [0u8; 32];
    seed_bytes.copy_from_slice(&seed_vec);

    let mut seed = seed_bytes;
    for (i, b) in index.to_be_bytes().iter().enumerate() {
        seed[24 + i] ^= *b;
    }
    secp256k1::SecretKey::from_slice(&seed).unwrap()
}

pub fn derive_keypair_from_index(index: u32, seed_hex: impl AsRef<[u8]>) -> secp256k1::Keypair {
    elements::bitcoin::secp256k1::Keypair::from_secret_key(
        elements::bitcoin::secp256k1::SECP256K1,
        &derive_secret_key_from_index(index, seed_hex),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use simplicityhl::elements;

    fn check_secret_for_index(index: u32, expected_hex: &str, seed_hex: impl AsRef<[u8]>) -> anyhow::Result<()> {
        let kp = derive_keypair_from_index(index, &seed_hex);

        let secret = elements::bitcoin::secp256k1::Keypair::from_secret_key(
            &elements::bitcoin::secp256k1::SECP256K1,
            &derive_secret_key_from_index(index, &seed_hex),
        )
        .secret_key();

        let sk_bytes = secret.secret_bytes();
        assert_eq!(hex::encode(sk_bytes), expected_hex.to_lowercase());
        Ok(())
    }

    fn check_keypair_determinism(index: u32, seed_hex: impl AsRef<[u8]>) {
        let kp1 = derive_keypair_from_index(index, &seed_hex);
        let kp2 = derive_keypair_from_index(index, &seed_hex);
        assert_eq!(kp1.secret_bytes(), kp2.secret_bytes());
    }

    #[test]
    fn derive_keypair_from_index_is_deterministic_for_seed() -> anyhow::Result<()> {
        const SEED_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        let expected_secrets = [
            (0u32, "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"),
            (1u32, "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e0e"),
            (2u32, "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e0d"),
        ];
        {
            let (index, expected_secret) = expected_secrets[0];
            check_secret_for_index(index, expected_secret, SEED_HEX)?;
        }

        check_keypair_determinism(5, SEED_HEX);
        Ok(())
    }
}
