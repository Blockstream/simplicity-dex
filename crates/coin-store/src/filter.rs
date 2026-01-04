use contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen;
use simplicityhl::elements::hashes::{sha256, Hash};
use simplicityhl::{
    elements::{AssetId, Script},
    simplicity::Cmr,
};

#[derive(Clone, Default)]
pub struct UtxoFilter {
    pub asset_id: Option<AssetId>,
    pub script_pubkey: Option<Script>,
    pub required_value: Option<u64>,
    pub limit: Option<i64>,
    pub include_spent: bool,
    pub include_entropy: bool,
    pub cmr: Option<Cmr>,
    pub taproot_pubkey_gen: Option<TaprootPubkeyGen>,
    pub source_hash: Option<[u8; 32]>,
}

impl UtxoFilter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn hash_source(source: &str) -> [u8; 32] {
        sha256::Hash::hash(source.as_bytes()).to_byte_array()
    }

    #[must_use]
    pub const fn asset_id(mut self, id: AssetId) -> Self {
        self.asset_id = Some(id);
        self
    }

    #[must_use]
    pub fn script_pubkey(mut self, script: Script) -> Self {
        self.script_pubkey = Some(script);
        self
    }

    #[must_use]
    pub const fn required_value(mut self, value: u64) -> Self {
        self.required_value = Some(value);
        self
    }

    #[must_use]
    pub const fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub const fn include_spent(mut self) -> Self {
        self.include_spent = true;
        self
    }

    #[must_use]
    pub const fn include_entropy(mut self) -> Self {
        self.include_entropy = true;
        self
    }

    #[must_use]
    pub const fn cmr(mut self, cmr: Cmr) -> Self {
        self.cmr = Some(cmr);
        self
    }

    #[must_use]
    pub fn taproot_pubkey_gen(mut self, tpg: TaprootPubkeyGen) -> Self {
        self.taproot_pubkey_gen = Some(tpg);
        self
    }

    #[must_use]
    pub const fn source_hash(mut self, hash: [u8; 32]) -> Self {
        self.source_hash = Some(hash);
        self
    }

    #[must_use]
    pub fn source(self, source: &str) -> Self {
        self.source_hash(Self::hash_source(source))
    }

    #[must_use]
    pub(crate) const fn is_contract_join(&self) -> bool {
        self.cmr.is_some() || self.taproot_pubkey_gen.is_some() || self.source_hash.is_some()
    }

    #[must_use]
    pub(crate) const fn is_entropy_join(&self) -> bool {
        self.include_entropy
    }
}
