use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use simplicityhl::elements::{AssetId, OutPoint, TxOut, TxOutSecrets};
use simplicityhl::{Arguments, CompiledProgram};

use crate::StoreError;
use crate::executor::UtxoRow;

#[derive(Debug, Clone)]
pub struct ContractContext {
    programs: HashMap<[u8; 32], Arc<CompiledProgram>>,
}

impl Default for ContractContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ContractContext {
    #[must_use]
    pub fn new() -> Self {
        Self {
            programs: HashMap::new(),
        }
    }

    pub(crate) fn add_program_from_row(self, row: &UtxoRow) -> Result<Self, StoreError> {
        let (Some(source_bytes), Some(args_bytes)) = (&row.source, &row.arguments) else {
            return Ok(self);
        };

        let source_str =
            String::from_utf8(source_bytes.clone()).map_err(|_| sqlx::Error::Decode("Invalid source UTF-8".into()))?;

        let (arguments, _): (Arguments, usize) =
            bincode::serde::decode_from_slice(args_bytes, bincode::config::standard())?;

        self.add_program(source_str, arguments)
    }

    pub fn add_program(mut self, source: String, arguments: Arguments) -> Result<Self, StoreError> {
        let key = Self::build_key(&source, &arguments)?;

        let program = CompiledProgram::new(source, arguments, false).map_err(StoreError::SimplicityCompilation)?;

        if let Entry::Vacant(v) = self.programs.entry(key) {
            v.insert(Arc::new(program));
        }

        Ok(self)
    }

    pub(crate) fn get_program_from_row(&self, row: &UtxoRow) -> Result<Option<&Arc<CompiledProgram>>, StoreError> {
        let (Some(source_bytes), Some(args_bytes)) = (&row.source, &row.arguments) else {
            return Ok(None);
        };

        let source_str =
            String::from_utf8(source_bytes.clone()).map_err(|_| sqlx::Error::Decode("Invalid source UTF-8".into()))?;

        let (arguments, _): (Arguments, usize) =
            bincode::serde::decode_from_slice(args_bytes, bincode::config::standard())?;

        self.get_program(&source_str, &arguments)
    }

    pub fn get_program(
        &self,
        source: &str,
        arguments: &Arguments,
    ) -> Result<Option<&Arc<CompiledProgram>>, StoreError> {
        let key = Self::build_key(source, arguments)?;

        Ok(self.programs.get(&key))
    }

    fn build_key(source: &str, arguments: &Arguments) -> Result<[u8; 32], StoreError> {
        let mut hasher = Sha256::new();

        hasher.update(source.as_bytes());
        hasher.update(bincode::serde::encode_to_vec(arguments, bincode::config::standard())?);

        Ok(hasher.finalize().into())
    }
}

#[derive(Debug)]
pub struct UtxoEntry {
    outpoint: OutPoint,
    txout: TxOut,
    secrets: Option<TxOutSecrets>,
    contract: Option<Arc<CompiledProgram>>,
}

impl UtxoEntry {
    #[must_use]
    pub const fn new_explicit(outpoint: OutPoint, txout: TxOut) -> Self {
        Self {
            outpoint,
            txout,
            secrets: None,
            contract: None,
        }
    }

    #[must_use]
    pub const fn new_confidential(outpoint: OutPoint, txout: TxOut, secrets: TxOutSecrets) -> Self {
        Self {
            outpoint,
            txout,
            secrets: Some(secrets),
            contract: None,
        }
    }

    #[must_use]
    pub fn with_contract(mut self, contract: Arc<CompiledProgram>) -> Self {
        self.contract = Some(contract);
        self
    }

    #[must_use]
    pub const fn outpoint(&self) -> &OutPoint {
        &self.outpoint
    }

    #[must_use]
    pub const fn txout(&self) -> &TxOut {
        &self.txout
    }

    #[must_use]
    pub fn asset(&self) -> AssetId {
        if let Some(secrets) = self.secrets.as_ref() {
            return secrets.asset;
        }

        // SAFE: due to the internal logic of the executor, asset MUST exist at that point pf time
        self.txout.asset.explicit().unwrap()
    }

    #[must_use]
    pub fn value(&self) -> u64 {
        if let Some(secrets) = self.secrets.as_ref() {
            return secrets.value;
        }

        // SAFE: due to the internal logic of the executor, value MUST exist at that point pf time
        self.txout.value.explicit().unwrap()
    }

    #[must_use]
    pub const fn secrets(&self) -> Option<&TxOutSecrets> {
        self.secrets.as_ref()
    }

    #[must_use]
    pub const fn contract(&self) -> Option<&Arc<CompiledProgram>> {
        self.contract.as_ref()
    }

    #[must_use]
    pub const fn is_confidential(&self) -> bool {
        self.secrets.is_some()
    }

    #[must_use]
    pub const fn is_bound(&self) -> bool {
        self.contract.is_some()
    }
}

#[derive(Debug)]
pub enum UtxoQueryResult {
    Found(Vec<UtxoEntry>, ContractContext),
    InsufficientValue(Vec<UtxoEntry>, ContractContext),
    Empty,
}
