use std::path::Path;

use coin_store::Store;
use signer::Signer;
use simplicityhl::elements::AddressParams;

use crate::error::Error;

pub struct Wallet {
    signer: Signer,
    store: Store,
    params: &'static AddressParams,
}

impl Wallet {
    pub async fn create(
        seed: &[u8; 32],
        db_path: impl AsRef<Path>,
        params: &'static AddressParams,
    ) -> Result<Self, Error> {
        let signer = Signer::from_seed(seed)?;
        let store = Store::create(db_path).await?;

        Ok(Self { signer, store, params })
    }

    pub async fn open(
        seed: &[u8; 32],
        db_path: impl AsRef<Path>,
        params: &'static AddressParams,
    ) -> Result<Self, Error> {
        let signer = Signer::from_seed(seed)?;
        let store = Store::connect(db_path).await?;

        Ok(Self { signer, store, params })
    }

    #[must_use]
    pub const fn signer(&self) -> &Signer {
        &self.signer
    }

    #[must_use]
    pub const fn store(&self) -> &Store {
        &self.store
    }

    #[must_use]
    pub const fn params(&self) -> &'static AddressParams {
        self.params
    }
}
