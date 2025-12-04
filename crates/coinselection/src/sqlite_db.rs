use crate::types::{CoinSelectionStorage, DcdContractTokenEntropies, GetTokenFilter, OutPointInfo};
use async_trait::async_trait;
use contracts::DCDArguments;
use simplicity::bitcoin::OutPoint;

pub struct SqliteDb {}

#[async_trait]
impl CoinSelectionStorage for SqliteDb {
    async fn add_outpoint(&self, info: OutPointInfo) -> crate::types::Result<()> {
        todo!()
    }

    async fn get_token_outpoint(&self, filter: GetTokenFilter) -> crate::types::Result<Vec<OutPoint>> {
        todo!()
    }

    async fn add_dcd_params(&self, taproot_pubkey_gen: &str, dcd_args: &DCDArguments) -> crate::types::Result<()> {
        todo!()
    }

    async fn get_dcd_params(&self, taproot_pubkey_gen: &str) -> crate::types::Result<Option<DCDArguments>> {
        todo!()
    }

    async fn add_dcd_contract_token_entropies(
        &self,
        taproot_pubkey_gen: &str,
        token_entropies: DcdContractTokenEntropies,
    ) -> crate::types::Result<()> {
        todo!()
    }

    async fn get_dcd_contract_token_entropies(
        &self,
        taproot_pubkey_gen: &str,
    ) -> crate::types::Result<Option<DcdContractTokenEntropies>> {
        todo!()
    }
}
