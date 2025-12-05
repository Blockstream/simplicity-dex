use crate::utils::{extract_outpoint_info_from_tx_in, extract_outpoint_info_from_tx_out, fetch_tx};
use async_trait::async_trait;
use contracts::DCDArguments;
use simplicityhl::elements::{OutPoint, Txid};
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
    MakerFund {
        taproot_pubkey_gen: String,
        script_pubkey: String,
    },
    TakerFundOrder {
        taproot_pubkey_gen: String,
    },
    TakerTerminationEarly {
        taproot_pubkey_gen: String,
    },
    TakerSettlement {
        taproot_pubkey_gen: String,
        filter: GetSettlementFilter,
    },
    MakerTerminationCollateral {
        taproot_pubkey_gen: String,
    },
    MakerTerminationSettlement {
        taproot_pubkey_gen: String,
    },
    MakerSettlement {
        taproot_pubkey_gen: String,
        filter: GetSettlementFilter,
    },
}

fn extract_one_value_from_vec<T: Clone>(vec: Vec<T>) -> Option<T> {
    if vec.is_empty() { None } else { Some(vec[0].clone()) }
}

#[async_trait]
pub trait CoinSelector: Send + Sync + CoinSelectionStorage + DcdParamsStorage + EntropyStorage {
    async fn add_outputs(&self, option: TransactionOption) -> Result<()> {
        match option {
            TransactionOption::TakerFundOrder(_) => {}
            TransactionOption::TakerTerminationEarly(_) => {}
            TransactionOption::TakerSettlement(_) => {}
            TransactionOption::MakerFund(order) => {
                // TODO: add fetching of only needed outs - not all
                let fetched_transaction = fetch_tx(order).await?;
                let mut outpoints_to_add =
                    Vec::with_capacity(fetched_transaction.input.len() + fetched_transaction.output.len());
                for tx_in in fetched_transaction.input {
                    outpoints_to_add.push(extract_outpoint_info_from_tx_in(tx_in).await?);
                }
                for (i, tx_out) in fetched_transaction.output.into_iter().enumerate() {
                    outpoints_to_add
                        .push(extract_outpoint_info_from_tx_out(OutPoint::new(order, i as u32), tx_out).await?);
                }
                for outpoint in outpoints_to_add {
                    tracing::debug!("[Coinselector] Adding outpoint {:?}", outpoint);
                    self.add_outpoint(outpoint).await?;
                }
            }
            TransactionOption::MakerTerminationCollateral(_) => {}
            TransactionOption::MakerTerminationSettlement(_) => {}
            TransactionOption::MakerSettlement(_) => {}
        }
        Ok(())
    }

    async fn get_inputs(&self, option: TransactionInputs) -> Result<TransactionInputsOption> {
        let res = match option {
            TransactionInputs::MakerFund {
                taproot_pubkey_gen,
                script_pubkey,
            } => {
                let dcd_params = self
                    .get_dcd_params(&taproot_pubkey_gen)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("No dcd params found, taproot_pubkey_gen: {taproot_pubkey_gen}"))?;

                let filler = extract_one_value_from_vec(
                    self.get_token_outpoints(GetTokenFilter {
                        asset_id: Some(dcd_params.filler_token_asset_id_hex_le),
                        spent: Some(false),
                        owner: Some(script_pubkey),
                    })
                    .await?,
                );

                TransactionInputsOption::MakerFund(MakerFundInputs {
                    filler_reissuance_tx: filler,
                    grantor_collateral_reissuance_tx: None,
                    grantor_settlement_reissuance_tx: None,
                    asset_settlement_tx: None,
                })
            }
            TransactionInputs::TakerFundOrder { .. } => TransactionInputsOption::TakerFundOrder(TakerFundInputs {
                filler_token: None,
                collateral_token: None,
            }),
            TransactionInputs::TakerTerminationEarly { .. } => {
                TransactionInputsOption::TakerTerminationEarly(TakerTerminationEarlyInputs {
                    filler_token: None,
                    collateral_token: None,
                })
            }
            TransactionInputs::TakerSettlement { .. } => {
                TransactionInputsOption::TakerSettlement(TakerSettlementInputs {
                    filler_token: None,
                    asset_token: None,
                })
            }
            TransactionInputs::MakerTerminationCollateral { .. } => {
                TransactionInputsOption::MakerTerminationCollateral(MakerTerminationCollateralInputs {
                    collateral_token_utxo: None,
                    grantor_collateral_token_utxo: None,
                })
            }
            TransactionInputs::MakerTerminationSettlement { .. } => {
                TransactionInputsOption::MakerTerminationSettlement(MakerTerminationSettlmentInputs {
                    settlement_asset_utxo: None,
                    grantor_settlement_token_utxo: None,
                })
            }
            TransactionInputs::MakerSettlement { .. } => {
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
    pub owner_script_pubkey: String,
    pub asset_id: String,
    pub spent: bool,
}

#[derive(Debug, Clone)]
pub struct OutPointInfoRaw {
    pub id: u64,
    pub outpoint: OutPoint,
    pub owner_script_pubkey: String,
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
    filler_reissuance_tx: Option<OutPointInfoRaw>,
    grantor_collateral_reissuance_tx: Option<OutPointInfoRaw>,
    grantor_settlement_reissuance_tx: Option<OutPointInfoRaw>,
    asset_settlement_tx: Option<OutPointInfoRaw>,
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
            query.push_str(" AND owner_script_pubkey = ?");
        }

        if !query.is_empty() {
            let substr = query.trim_start_matches(" AND");
            let substr = substr.trim();
            query = format!(" WHERE ({})", substr);
        }
        query
    }
}
