use crate::cli::interactive::{TokenDisplay, WalletAssetDisplay};
use crate::cli::option_offer::{
    ActiveOptionOfferDisplay, CancellableOptionOfferDisplay, WithdrawableOptionOfferDisplay,
};
use crate::cli::positions::{ActiveOptionsDisplay, CollateralDisplay, UserTokenDisplay};
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Table};

trait TableData {
    fn get_header() -> Vec<String>;
    fn to_row(&self) -> Vec<String>;
}

impl TableData for ActiveOptionsDisplay {
    fn get_header() -> Vec<String> {
        vec!["#", "Role", "Option-tokens", "Grantor-tokens", "Expiry", "Contract"]
            .into_iter()
            .map(String::from)
            .collect()
    }
    fn to_row(&self) -> Vec<String> {
        vec![
            self.index.to_string(),
            self.role.clone(),
            self.option_tokens.to_string(),
            self.grantor_tokens.to_string(),
            self.expires.clone(),
            self.contract_id.clone(),
        ]
    }
}

impl TableData for TokenDisplay {
    fn get_header() -> Vec<String> {
        vec!["#", "Collateral/Token", "Strike/Token", "Expires", "Contract"]
            .into_iter()
            .map(String::from)
            .collect()
    }
    fn to_row(&self) -> Vec<String> {
        vec![
            self.index.to_string(),
            self.collateral.clone(),
            self.settlement.clone(),
            self.expires.clone(),
            self.status.clone(),
        ]
    }
}

impl TableData for CollateralDisplay {
    fn get_header() -> Vec<String> {
        vec!["#", "Locked Assets", "Settlement", "Expires", "Contract"]
            .into_iter()
            .map(String::from)
            .collect()
    }
    fn to_row(&self) -> Vec<String> {
        vec![
            self.index.to_string(),
            self.collateral.clone(),
            self.settlement.clone(),
            self.expires.clone(),
            self.contract.clone(),
        ]
    }
}

impl TableData for UserTokenDisplay {
    fn get_header() -> Vec<String> {
        vec!["#", "Type", "Amount", "Strike/Token", "Expires", "Contract"]
            .into_iter()
            .map(String::from)
            .collect()
    }
    fn to_row(&self) -> Vec<String> {
        vec![
            self.index.to_string(),
            self.token_type.clone(),
            self.amount.clone(),
            self.strike.clone(),
            self.expires.clone(),
            self.contract.clone(),
        ]
    }
}

impl TableData for ActiveOptionOfferDisplay {
    fn get_header() -> Vec<String> {
        vec!["#", "Offering", "Price", "Wants", "Expires", "Seller"]
            .into_iter()
            .map(String::from)
            .collect()
    }
    fn to_row(&self) -> Vec<String> {
        vec![
            self.index.to_string(),
            self.offering.clone(),
            self.price.clone(),
            self.wants.clone(),
            self.expires.clone(),
            self.seller.clone(),
        ]
    }
}

impl TableData for CancellableOptionOfferDisplay {
    fn get_header() -> Vec<String> {
        vec!["#", "Collateral", "Premium", "Asset", "Expired", "Contract"]
            .into_iter()
            .map(String::from)
            .collect()
    }
    fn to_row(&self) -> Vec<String> {
        vec![
            self.index.to_string(),
            self.collateral.clone(),
            self.premium.clone(),
            self.asset.clone(),
            self.expired.clone(),
            self.contract.clone(),
        ]
    }
}

impl TableData for WithdrawableOptionOfferDisplay {
    fn get_header() -> Vec<String> {
        vec!["#", "Settlement Available", "Asset", "Contract"]
            .into_iter()
            .map(String::from)
            .collect()
    }
    fn to_row(&self) -> Vec<String> {
        vec![
            self.index.to_string(),
            self.settlement.clone(),
            self.asset.clone(),
            self.contract.clone(),
        ]
    }
}

impl TableData for WalletAssetDisplay {
    fn get_header() -> Vec<String> {
        vec!["#", "Asset", "Balance"].into_iter().map(String::from).collect()
    }
    fn to_row(&self) -> Vec<String> {
        vec![
            self.index.to_string(),
            self.asset_name.clone(),
            self.balance.to_string(),
        ]
    }
}

pub struct UtxoDisplay {
    pub outpoint: String,
    pub asset: String,
    pub value: String,
}

impl TableData for UtxoDisplay {
    fn get_header() -> Vec<String> {
        vec!["Outpoint", "Asset", "Value"]
            .into_iter()
            .map(String::from)
            .collect()
    }
    fn to_row(&self) -> Vec<String> {
        vec![self.outpoint.clone(), self.asset.clone(), self.value.clone()]
    }
}

fn render_table<T: TableData>(items: &[T], empty_msg: &str) {
    if items.is_empty() {
        println!("  ({empty_msg})");
        return;
    }

    let mut table = Table::new();

    table.load_preset(UTF8_FULL);

    let header_cells: Vec<Cell> = T::get_header()
        .into_iter()
        .map(|h| Cell::new(h).add_attribute(Attribute::Bold))
        .collect();
    table.set_header(header_cells);

    for item in items {
        table.add_row(item.to_row());
    }

    for line in table.to_string().lines() {
        println!("  {line}");
    }
}

pub fn display_active_options_table(active_options: &[ActiveOptionsDisplay]) {
    render_table(active_options, "No active options found");
}

pub fn display_token_table(tokens: &[TokenDisplay]) {
    render_table(tokens, "No tokens found");
}

pub fn display_collateral_table(displays: &[CollateralDisplay]) {
    render_table(displays, "No locked assets found");
}

pub fn display_user_token_table(displays: &[UserTokenDisplay]) {
    render_table(displays, "No option/grantor tokens found");
}

pub fn display_active_option_offers_table(active_offers: &[ActiveOptionOfferDisplay]) {
    render_table(active_offers, "No option offers found");
}

pub fn display_cancellable_option_offers_table(cancellable_offers: &[CancellableOptionOfferDisplay]) {
    render_table(cancellable_offers, "No cancellable option offers found");
}

pub fn display_withdrawable_option_offers_table(withdrawable_offers: &[WithdrawableOptionOfferDisplay]) {
    render_table(withdrawable_offers, "No withdrawable option offers found");
}

pub fn display_utxo_table(utxos: &[UtxoDisplay]) {
    render_table(utxos, "No UTXOs found");
}

pub fn display_wallet_assets_table(assets: &[WalletAssetDisplay]) {
    render_table(assets, "No assets found in wallet");
}
