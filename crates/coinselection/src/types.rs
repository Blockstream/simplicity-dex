use async_trait::async_trait;
use contracts::DCDArguments;
use simplicity::bitcoin::Txid;
use simplicityhl::elements::bitcoin::OutPoint;
use simplicityhl_core::AssetEntropyHex;

pub type Result<T> = anyhow::Result<T>;

#[async_trait]
pub trait CoinSelectionStorage: Send + Sync {
    async fn add_outpoint(&self, info: OutPointInfo) -> Result<()>;
    async fn get_token_outpoint(&self, filter: GetTokenFilter) -> Result<Vec<OutPoint>>;
    async fn add_dcd_params(&self, taproot_pubkey_gen: &str, dcd_args: &DCDArguments) -> Result<()>;
    async fn get_dcd_params(&self, taproot_pubkey_gen: &str) -> Result<Option<DCDArguments>>;
    async fn add_dcd_contract_token_entropies(
        &self,
        taproot_pubkey_gen: &str,
        token_entropies: DcdContractTokenEntropies,
    ) -> Result<()>;
    async fn get_dcd_contract_token_entropies(
        &self,
        taproot_pubkey_gen: &str,
    ) -> Result<Option<DcdContractTokenEntropies>>;
}

#[async_trait]
pub trait CoinSelector: Send + Sync + CoinSelectionStorage {
    async fn add_taker_fund_order_outputs(&self, tx_id: Txid) -> Result<()> {
        //todo: add inputs of transaction, mark the mas spent and

        //todo: add implementation
        Ok(())
    }
    async fn add_taker_termination_early_outputs(&self, tx_id: Txid) -> Result<()> {
        //todo: add inputs of transaction, mark the mas spent and
        //todo: add implementation
        Ok(())
    }
    async fn add_taker_settlement_outputs(&self, tx_id: Txid) -> Result<()> {
        //todo: add inputs of transaction, mark the mas spent and
        //todo: add implementation
        Ok(())
    }
    async fn add_maker_fund_outputs(&self, tx_id: Txid) -> Result<()> {
        //todo: add inputs of transaction, mark the mas spent and
        //todo: add implementation
        Ok(())
    }
    async fn add_maker_termination_collateral_outputs(&self, tx_id: Txid) -> Result<()> {
        //todo: add inputs of transaction, mark the mas spent and
        //todo: add implementation
        Ok(())
    }
    async fn add_maker_termination_settlement_outputs(&self, tx_id: Txid) -> Result<()> {
        //todo: add inputs of transaction, mark the mas spent and
        //todo: add implementation
        Ok(())
    }
    async fn add_maker_settlement_outputs(&self, tx_id: Txid) -> Result<()> {
        //todo: add inputs of transaction, mark the mas spent and
        //todo: add implementation
        Ok(())
    }
    async fn get_taker_fund_order_inputs(&self) -> Result<TakerFundInputs> {
        //todo: add implementation
        Ok(TakerFundInputs {
            filler_token: None,
            collateral_token: None,
        })
    }
    async fn get_taker_termination_early_inputs(&self) -> Result<TakerTerminationEarlyInputs> {
        //todo: add implementation
        Ok(TakerTerminationEarlyInputs {
            filler_token: None,
            collateral_token: None,
        })
    }
    async fn get_taker_settlement_inputs(&self, filter: GetSettlementFilter) -> Result<TakerSettlementInputs> {
        //todo: add implementation
        Ok(TakerSettlementInputs {
            filler_token: None,
            asset_token: None,
        })
    }
    async fn get_maker_fund_inputs(&self) -> Result<MakerFundInputs> {
        //todo: add implementation
        Ok(MakerFundInputs {
            filler_reissuance_tx: None,
            grantor_collateral_reissuance_tx: None,
            grantor_settlement_reissuance_tx: None,
            asset_settlement_tx: None,
        })
    }
    async fn get_maker_termination_collateral_inputs(&self) -> Result<MakerTerminationCollateralInputs> {
        //todo: add implementation
        Ok(MakerTerminationCollateralInputs {
            collateral_token_utxo: None,
            grantor_collateral_token_utxo: None,
        })
    }
    async fn get_maker_termination_settlement_inputs(&self) -> Result<MakerTerminationSettlmentInputs> {
        //todo: add implementation
        Ok(MakerTerminationSettlmentInputs {
            settlement_asset_utxo: None,
            grantor_settlement_token_utxo: None,
        })
    }
    async fn get_maker_settlement_inputs(&self, filter: GetSettlementFilter) -> Result<MakerSettlementInputs> {
        //todo: add implementation
        Ok(MakerSettlementInputs {
            asset_utxo: None,
            grantor_collateral_token_utxo: None,
            grantor_settlement_token_utxo: None,
        })
    }
}

impl<T: Send + Sync + CoinSelectionStorage> CoinSelector for T {}

pub struct DcdContractTokenEntropies {
    filler_token_entropy: AssetEntropyHex,
    grantor_collateral_token_entropy: AssetEntropyHex,
    grantor_settlement_token_entropy: AssetEntropyHex,
}

#[derive(Clone, Copy, Debug)]
pub struct GetSettlementFilter {
    /// Flag used to correctly choose between Collateral and Settlement tokens
    price_go_higher: bool,
}

pub struct OutPointInfo {
    pub outpoint: OutPoint,
    pub owner_addr: String,
}

pub struct GetTokenFilter {
    /// Token asset id to look for
    asset_id: Option<String>,
    /// Whether transaction is spent or not according to db or not
    spent: Option<bool>,
    /// Owner of
    owner: Option<String>,
}

pub struct TakerFundInputs {
    filler_token: Option<OutPoint>,
    collateral_token: Option<OutPoint>,
}

pub struct TakerTerminationEarlyInputs {
    filler_token: Option<OutPoint>,
    collateral_token: Option<OutPoint>,
}

pub struct TakerSettlementInputs {
    filler_token: Option<OutPoint>,
    asset_token: Option<OutPoint>,
}

pub struct MakerFundInputs {
    filler_reissuance_tx: Option<OutPoint>,
    grantor_collateral_reissuance_tx: Option<OutPoint>,
    grantor_settlement_reissuance_tx: Option<OutPoint>,
    asset_settlement_tx: Option<OutPoint>,
}

pub struct MakerTerminationCollateralInputs {
    collateral_token_utxo: Option<OutPoint>,
    grantor_collateral_token_utxo: Option<OutPoint>,
}
pub struct MakerTerminationSettlmentInputs {
    settlement_asset_utxo: Option<OutPoint>,
    grantor_settlement_token_utxo: Option<OutPoint>,
}
pub struct MakerSettlementInputs {
    asset_utxo: Option<OutPoint>,
    grantor_collateral_token_utxo: Option<OutPoint>,
    grantor_settlement_token_utxo: Option<OutPoint>,
}
