use contracts::sdk::taproot_pubkey_gen::TaprootPubkeyGen;
use simplicityhl::{
    elements::{AssetId, Script},
    simplicity::Cmr,
};

#[derive(Clone, Default)]
pub struct UtxoFilter {
    pub asset_id: Option<AssetId>,
    pub token_id: Option<AssetId>,
    pub script_pubkey: Option<Script>,
    pub required_value: Option<u64>,
    pub limit: Option<i64>,
    pub include_spent: bool,
    pub include_entropy: bool,
    pub cmr: Option<Cmr>,
    pub taproot_pubkey_gen: Option<TaprootPubkeyGen>,
    pub source: Option<String>,
}

impl UtxoFilter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn asset_id(mut self, id: AssetId) -> Self {
        self.asset_id = Some(id);
        self
    }

    #[must_use]
    pub const fn token_id(mut self, id: AssetId) -> Self {
        self.token_id = Some(id);
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
    pub fn source(mut self, source: String) -> Self {
        self.source = Some(source);
        self
    }

    #[must_use]
    pub(crate) const fn is_contract_join(&self) -> bool {
        self.cmr.is_some() || self.taproot_pubkey_gen.is_some() || self.source.is_some()
    }
}
