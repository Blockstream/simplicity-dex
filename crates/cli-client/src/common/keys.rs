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
    use elements::hex::ToHex;
    use proptest::prelude::*;
    use simplicityhl::elements;
    use simplicityhl::elements::AddressParams;
    use simplicityhl_core::get_p2pk_address;

    fn check_seed_hex_gen(
        index: u32,
        x_only_pubkey: &str,
        p2pk_addr: &str,
        seed_hex: impl AsRef<[u8]>,
    ) -> anyhow::Result<()> {
        let keypair = derive_keypair_from_index(index, &seed_hex);

        let public_key = keypair.x_only_public_key().0;
        let address = get_p2pk_address(&public_key, &AddressParams::LIQUID_TESTNET)?;

        assert_eq!(public_key.to_string(), x_only_pubkey);
        assert_eq!(address.to_string(), p2pk_addr);
        Ok(())
    }

    #[test]
    fn derive_keypair_from_index_is_deterministic_for_seed() -> anyhow::Result<()> {
        const SEED_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        let expected_secrets = [
            (
                0u32,
                "4646ae5047316b4230d0086c8acec687f00b1cd9d1dc634f6cb358ac0a9a8fff",
                "tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm",
            ),
            (
                1u32,
                "16e47b8867bfbeaae66c0345577751c551903eb90ba479e91f783c507c088732",
                "tex1prmytj5v08w6jwjtm4exmuxv0nn8favzyqu3aptzrgvl44nfatqmsykjhk3",
            ),
            (
                2u32,
                "d0d0fce6bc500821c33212666ecfbd9d41a1414d584af4102e7441277d25d872",
                "tex1phctnz400pn7r3rhh8nyc2xmsg2e9h2n299a8ld4pup0v5def9cdsjz3put",
            ),
        ];
        let check_address_with_index = |i| -> anyhow::Result<()> {
            let (index, x_only_pubkey, p2pk_addr) = expected_secrets[i];
            check_seed_hex_gen(index, x_only_pubkey, p2pk_addr, SEED_HEX)?;
            Ok(())
        };

        check_address_with_index(0)?;
        check_address_with_index(1)?;
        check_address_with_index(2)?;
        Ok(())
    }

    proptest! {
        #[test]
        fn prop_keypair_determinism(index in 0u32..u32::MAX, seed in any::<[u8; 32]>()) {
            let seed_hex = seed.to_hex();

            let kp1 = derive_keypair_from_index(index, &seed_hex);
            let kp2 = derive_keypair_from_index(index, &seed_hex);

            prop_assert_eq!(kp1.secret_bytes(), kp2.secret_bytes());
        }
    }
}
