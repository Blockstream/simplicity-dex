use simplicity::elements::{AssetId, OutPoint};
use simplicityhl::elements::Txid;
use simplicityhl_core::LIQUID_TESTNET_BITCOIN_ASSET;
use std::net::SocketAddr;

#[derive(Clone, Copy, Debug)]
pub enum LiquidNetwork {
    Liquid,
    LiquidTestnet,
    Elements(SocketAddr),
}

pub type UtxoList = [OutPoint; 3];
pub type AssetEntropyList = [String; 3];
pub type AssetEntropyBytes = [u8; 32];
pub type AssetEntropyHex = String;
pub type AssetIdHex = String;
pub type FillerTokenEntropyHex = AssetEntropyHex;
pub type FillerTokenEntropyBytes = AssetEntropyBytes;
pub type GrantorCollateralAssetEntropyHex = AssetEntropyHex;
pub type GrantorCollateralAssetEntropyBytes = AssetEntropyBytes;
pub type GrantorSettlementAssetEntropyHex = AssetEntropyHex;
pub type GrantorSettlementAssetEntropyBytes = AssetEntropyBytes;
pub const COLLATERAL_ASSET_ID: AssetId = LIQUID_TESTNET_BITCOIN_ASSET;

impl LiquidNetwork {
    pub fn addr_params(&self) -> &'static simplicity::elements::AddressParams {
        match self {
            LiquidNetwork::Liquid => &simplicity::elements::AddressParams::LIQUID,
            LiquidNetwork::LiquidTestnet => &simplicity::elements::AddressParams::LIQUID_TESTNET,
            LiquidNetwork::Elements(_) => &simplicity::elements::AddressParams::ELEMENTS,
        }
    }

    pub fn esplora_asset_base(&self) -> String {
        match self {
            LiquidNetwork::Liquid => "https://blockstream.info/liquid/api/asset/".to_string(),
            LiquidNetwork::LiquidTestnet => "https://blockstream.info/liquidtestnet/api/asset/".to_string(),
            LiquidNetwork::Elements(socket) => format!("http://{socket}/api/asset/"),
        }
    }

    pub fn explore_raw_tx(&self, tx_id: Txid) -> String {
        match self {
            LiquidNetwork::Liquid => format!("https://blockstream.info/liquid/api/api/tx/{}/hex", tx_id),
            LiquidNetwork::LiquidTestnet => format!("https://blockstream.info/liquidtestnet/api/tx/{}/hex", tx_id),
            LiquidNetwork::Elements(socket) => format!("http://{socket}/api/tx/{}/hex", tx_id),
        }
    }
}
