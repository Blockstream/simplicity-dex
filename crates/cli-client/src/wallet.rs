use std::path::Path;

use coin_store::Store;
use signer::Signer;
use simplicityhl_core::SimplicityNetwork;

use crate::error::Error;

pub struct Wallet {
    signer: Signer,
    store: Store,
    network: SimplicityNetwork,
}

impl Wallet {
    pub async fn create(
        seed: &[u8; Signer::SEED_LEN],
        db_path: impl AsRef<Path>,
        network: SimplicityNetwork,
    ) -> Result<Self, Error> {
        let signer = Signer::from_seed(seed)?;
        let store = Store::create(db_path).await?;

        Ok(Self { signer, store, network })
    }

    pub async fn open(
        seed: &[u8; Signer::SEED_LEN],
        db_path: impl AsRef<Path>,
        network: SimplicityNetwork,
    ) -> Result<Self, Error> {
        let signer = Signer::from_seed(seed)?;
        let store = Store::connect(db_path).await?;

        Ok(Self { signer, store, network })
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
    pub const fn network(&self) -> SimplicityNetwork {
        self.network
    }
}
