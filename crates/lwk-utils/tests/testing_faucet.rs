mod utils;

mod tests{
    use lwk_signer::SwSigner;
    use lwk_wollet::{ElementsNetwork, NoPersist, Wollet};
    use nostr::secp256k1::Secp256k1;
    use simplicity::bitcoin::secp256k1::Keypair;
    use crate::utils::get_descriptor;

    #[test]
    fn test() -> anyhow::Result<()>{
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let network = ElementsNetwork::LiquidTestnet;

        // 1. Create a wallet using SwSigner
        let sw_signer = SwSigner::new(mnemonic, false)?;
        let sw_wallet = Wollet::new(
            network,
            NoPersist::new(),
            get_descriptor(&sw_signer).unwrap(),
        )
            ?;
        let secp = Secp256k1::new();
        let keypair = Keypair::from_seckey_str(&secp, sw_signer.xpub().)?

        Ok(())
    }
}