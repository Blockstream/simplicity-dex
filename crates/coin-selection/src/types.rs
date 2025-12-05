use async_trait::async_trait;
use contracts::DCDArguments;
use simplicity::bitcoin::Txid;
use simplicityhl::elements::bitcoin::OutPoint;
use simplicityhl_core::AssetEntropyHex;

pub type Result<T> = anyhow::Result<T>;

#[async_trait]
pub trait CoinSelectionStorage: Send + Sync {
    async fn mark_outpoints_spent(&self, outpoint: &[OutPoint]) -> Result<()>;
    async fn add_outpoint(&self, info: OutPointInfo) -> Result<()>;
    async fn get_token_outpoints(&self, filter: GetTokenFilter) -> Result<Vec<OutPointInfoRaw>>;
}

#[async_trait]
pub trait DcdParamsStorage: Send + Sync {
    async fn add_dcd_params(&self, taproot_pubkey_gen: &str, dcd_args: &DCDArguments) -> Result<()>;
    async fn get_dcd_params(&self, taproot_pubkey_gen: &str) -> Result<Option<DCDArguments>>;
}

#[async_trait]
pub trait EntropyStorage: Send + Sync {
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

#[derive(Clone, Debug)]
pub enum TransactionOption {
    TakerFundOrder(Txid),
    TakerTerminationEarly(Txid),
    TakerSettlement(Txid),
    MakerFund(Txid),
    MakerTerminationCollateral(Txid),
    MakerTerminationSettlement(Txid),
    MakerSettlement(Txid),
}

#[derive(Clone, Debug)]
pub enum TransactionInputsOption {
    TakerFundOrder(TakerFundInputs),
    TakerTerminationEarly(TakerTerminationEarlyInputs),
    TakerSettlement(TakerSettlementInputs),
    MakerFund(MakerFundInputs),
    MakerTerminationCollateral(MakerTerminationCollateralInputs),
    MakerTerminationSettlement(MakerTerminationSettlmentInputs),
    MakerSettlement(MakerSettlementInputs),
}

#[derive(Clone, Debug)]
pub enum TransactionInputs {
    TakerFundOrder,
    TakerTerminationEarly,
    TakerSettlement(GetSettlementFilter),
    MakerFund,
    MakerTerminationCollateral,
    MakerTerminationSettlement,
    MakerSettlement(GetSettlementFilter),
}

#[async_trait]
pub trait CoinSelector: Send + Sync + CoinSelectionStorage + DcdParamsStorage + EntropyStorage {
    async fn add_outputs(&self, option: TransactionOption) -> Result<()> {
        match option {
            TransactionOption::TakerFundOrder(_) => {}
            TransactionOption::TakerTerminationEarly(_) => {}
            TransactionOption::TakerSettlement(_) => {}
            TransactionOption::MakerFund(_) => {}
            TransactionOption::MakerTerminationCollateral(_) => {}
            TransactionOption::MakerTerminationSettlement(_) => {}
            TransactionOption::MakerSettlement(_) => {}
        }
        Ok(())
    }

    async fn get_inputs(&self, option: TransactionInputs) -> Result<TransactionInputsOption> {
        let res = match option {
            TransactionInputs::TakerFundOrder => TransactionInputsOption::TakerFundOrder(TakerFundInputs {
                filler_token: None,
                collateral_token: None,
            }),
            TransactionInputs::TakerTerminationEarly => {
                TransactionInputsOption::TakerTerminationEarly(TakerTerminationEarlyInputs {
                    filler_token: None,
                    collateral_token: None,
                })
            }
            TransactionInputs::TakerSettlement(_filter) => {
                TransactionInputsOption::TakerSettlement(TakerSettlementInputs {
                    filler_token: None,
                    asset_token: None,
                })
            }
            TransactionInputs::MakerFund => TransactionInputsOption::MakerFund(MakerFundInputs {
                filler_reissuance_tx: None,
                grantor_collateral_reissuance_tx: None,
                grantor_settlement_reissuance_tx: None,
                asset_settlement_tx: None,
            }),
            TransactionInputs::MakerTerminationCollateral => {
                TransactionInputsOption::MakerTerminationCollateral(MakerTerminationCollateralInputs {
                    collateral_token_utxo: None,
                    grantor_collateral_token_utxo: None,
                })
            }
            TransactionInputs::MakerTerminationSettlement => {
                TransactionInputsOption::MakerTerminationSettlement(MakerTerminationSettlmentInputs {
                    settlement_asset_utxo: None,
                    grantor_settlement_token_utxo: None,
                })
            }
            TransactionInputs::MakerSettlement(_filter) => {
                TransactionInputsOption::MakerSettlement(MakerSettlementInputs {
                    asset_utxo: None,
                    grantor_collateral_token_utxo: None,
                    grantor_settlement_token_utxo: None,
                })
            }
        };
        Ok(res)
    }
}

impl<T: Send + Sync + CoinSelectionStorage + EntropyStorage + DcdParamsStorage> CoinSelector for T {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode, PartialEq)]
pub struct DcdContractTokenEntropies {
    pub filler_token_entropy: AssetEntropyHex,
    pub grantor_collateral_token_entropy: AssetEntropyHex,
    pub grantor_settlement_token_entropy: AssetEntropyHex,
}

#[expect(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct GetSettlementFilter {
    /// Flag used to correctly choose between Collateral and Settlement tokens
    price_go_higher: bool,
}

#[derive(Debug, Clone)]
pub struct OutPointInfo {
    pub outpoint: OutPoint,
    pub owner_addr: String,
    pub asset_id: String,
    pub spent: bool,
}

#[derive(Debug, Clone)]
pub struct OutPointInfoRaw {
    pub id: u64,
    pub outpoint: OutPoint,
    pub owner_address: String,
    pub asset_id: String,
    pub spent: bool,
}

#[derive(Debug, Clone)]
pub struct GetTokenFilter {
    /// Token asset id to look for
    pub asset_id: Option<String>,
    /// Whether transaction is spent or not according to db or not
    pub spent: Option<bool>,
    /// Owner of token
    pub owner: Option<String>,
}

#[expect(dead_code)]
#[derive(Debug, Clone)]
pub struct TakerFundInputs {
    filler_token: Option<OutPoint>,
    collateral_token: Option<OutPoint>,
}

#[expect(dead_code)]
#[derive(Debug, Clone)]
pub struct TakerTerminationEarlyInputs {
    filler_token: Option<OutPoint>,
    collateral_token: Option<OutPoint>,
}

#[expect(dead_code)]
#[derive(Debug, Clone)]
pub struct TakerSettlementInputs {
    filler_token: Option<OutPoint>,
    asset_token: Option<OutPoint>,
}

#[expect(dead_code)]
#[derive(Debug, Clone)]
pub struct MakerFundInputs {
    filler_reissuance_tx: Option<OutPoint>,
    grantor_collateral_reissuance_tx: Option<OutPoint>,
    grantor_settlement_reissuance_tx: Option<OutPoint>,
    asset_settlement_tx: Option<OutPoint>,
}

#[expect(dead_code)]
#[derive(Debug, Clone)]
pub struct MakerTerminationCollateralInputs {
    collateral_token_utxo: Option<OutPoint>,
    grantor_collateral_token_utxo: Option<OutPoint>,
}

#[expect(dead_code)]
#[derive(Debug, Clone)]
pub struct MakerTerminationSettlmentInputs {
    settlement_asset_utxo: Option<OutPoint>,
    grantor_settlement_token_utxo: Option<OutPoint>,
}

#[expect(dead_code)]
#[derive(Debug, Clone)]
pub struct MakerSettlementInputs {
    asset_utxo: Option<OutPoint>,
    grantor_collateral_token_utxo: Option<OutPoint>,
    grantor_settlement_token_utxo: Option<OutPoint>,
}

impl GetTokenFilter {
    pub fn get_sql_filter(&self) -> String {
        let mut query = String::new();

        if self.asset_id.is_some() {
            query.push_str(" AND asset_id = ?");
        }
        if self.spent.is_some() {
            query.push_str(" AND spent = ?");
        }
        if self.owner.is_some() {
            query.push_str(" AND owner_address = ?");
        }

        if !query.is_empty() {
            let substr = query.trim_start_matches(" AND");
            let substr = substr.trim();
            query = format!(" WHERE ({})", substr);
        }
        query
    }
}
