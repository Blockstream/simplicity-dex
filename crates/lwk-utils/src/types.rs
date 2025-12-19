/// A trait that can be used to sign messages and verify signatures.
/// The sdk user can implement this trait to use their own signer.
pub trait SimplicitySigner: Send + Sync {
    /// The master xpub encoded as 78 bytes length as defined in bip32 specification.
    /// For reference: <https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki#user-content-Serialization_format>
    fn xpub(&self) -> anyhow::Result<Vec<u8>>;

    /// The derived xpub encoded as 78 bytes length as defined in bip32 specification.
    /// The derivation path is a string represents the shorter notation of the key tree to derive. For example:
    /// m/49'/1'/0'/0/0
    /// m/48'/1'/0'/0/0
    /// For reference: <https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki#user-content-The_key_tree>
    fn derive_xpub(&self, derivation_path: String) -> anyhow::Result<Vec<u8>>;

    /// Sign an ECDSA message using the private key derived from the given derivation path
    fn sign_ecdsa(&self, msg: Vec<u8>, derivation_path: String) -> anyhow::Result<Vec<u8>>;

    /// Sign an ECDSA message using the private key derived from the master key
    fn sign_ecdsa_recoverable(&self, msg: Vec<u8>) -> anyhow::Result<Vec<u8>>;

    /// Return the master blinding key for SLIP77: <https://github.com/satoshilabs/slips/blob/master/slip-0077.md>
    fn slip77_master_blinding_key(&self) -> anyhow::Result<Vec<u8>>;

    /// HMAC-SHA256 using the private key derived from the given derivation path
    /// This is used to calculate the linking key of lnurl-auth specification: <https://github.com/lnurl/luds/blob/luds/05.md>
    fn hmac_sha256(&self, msg: Vec<u8>, derivation_path: String) -> anyhow::Result<Vec<u8>>;
}
