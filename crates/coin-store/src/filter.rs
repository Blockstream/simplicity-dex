use simplicityhl::elements::{AssetId, Script};

#[derive(Clone, Default)]
pub struct Filter {
    pub(crate) asset_id: Option<AssetId>,
    pub(crate) script_pubkey: Option<Script>,
    pub(crate) required_value: Option<u64>,
    pub(crate) limit: Option<usize>,
    pub(crate) include_spent: bool,
}

impl Filter {
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
    pub const fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub const fn include_spent(mut self) -> Self {
        self.include_spent = true;
        self
    }
}
