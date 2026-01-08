use crate::cli::interactive::{TokenDisplay, SwapDisplay};
use crate::cli::positions::{CollateralDisplay, UserTokenDisplay};
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Table};


pub fn display_token_table(tokens: &[TokenDisplay]) {
    if tokens.is_empty() {
        println!("  (No tokens found)");
        return;
    }

    let mut table = Table::new();

    table.load_preset(UTF8_FULL);

    table.set_header(vec![
        Cell::new("#").add_attribute(Attribute::Bold),
        Cell::new("Collateral/Token").add_attribute(Attribute::Bold),
        Cell::new("Strike/Token").add_attribute(Attribute::Bold),
        Cell::new("Expires").add_attribute(Attribute::Bold),
        Cell::new("Contract").add_attribute(Attribute::Bold),
    ]);

    for token in tokens {
        table.add_row(vec![
            token.index.to_string(),
            token.collateral.clone(),
            token.settlement.clone(),
            token.expires.clone(),
            token.status.clone(),
        ]);
    }

    let table_string = table.to_string();
    for line in table_string.lines() {
        println!("  {}", line);
    }
}

pub fn display_swap_table(swaps: &[SwapDisplay]) {
    if swaps.is_empty() {
        println!("  (No swaps found)");
        return;
    }

    let mut table = Table::new();

    table.load_preset(UTF8_FULL);

    table.set_header(vec![
        Cell::new("#").add_attribute(Attribute::Bold),
        Cell::new("Price").add_attribute(Attribute::Bold),
        Cell::new("Wants").add_attribute(Attribute::Bold),
        Cell::new("Expires").add_attribute(Attribute::Bold),
        Cell::new("Seller").add_attribute(Attribute::Bold),
    ]);

    for swap in swaps {
        table.add_row(vec![
            swap.index.to_string(),
            swap.offering.clone(),
            swap.wants.clone(),
            swap.expires.clone(),
            swap.seller.clone(),
        ]);
    }

    let table_string = table.to_string();
    for line in table_string.lines() {
        println!("  {}", line);
    }
}


pub fn display_collateral_table(displays: &[CollateralDisplay]) {
    if displays.is_empty() {
        println!("  (No locked assets found)");
        return;
    }

    let mut table = Table::new();

    table.load_preset(UTF8_FULL);

    table.set_header(vec![
        Cell::new("#").add_attribute(Attribute::Bold),
        Cell::new("Locked Assets").add_attribute(Attribute::Bold),
        Cell::new("Settlement").add_attribute(Attribute::Bold),
        Cell::new("Expires").add_attribute(Attribute::Bold),
        Cell::new("Contract").add_attribute(Attribute::Bold),
    ]);

    for display in displays {
        table.add_row(vec![
            display.index.to_string(),
            display.collateral.clone(),
            display.settlement.clone(),
            display.expires.clone(),
            display.contract.clone(),
        ]);
    }

    let table_string = table.to_string();
    for line in table_string.lines() {
        println!("  {}", line);
    }
}

pub fn display_user_token_table(displays: &[UserTokenDisplay]) {
    if displays.is_empty() {
        println!("  (No option/grantor tokens found)");
        return;
    }

    let mut table = Table::new();

    table.load_preset(UTF8_FULL);

    table.set_header(vec![
        Cell::new("#").add_attribute(Attribute::Bold),
        Cell::new("Type").add_attribute(Attribute::Bold),
        Cell::new("Amount").add_attribute(Attribute::Bold),
        Cell::new("Strike/Token").add_attribute(Attribute::Bold),
        Cell::new("Expires").add_attribute(Attribute::Bold),
        Cell::new("Contract").add_attribute(Attribute::Bold),
    ]);

    for display in displays {
        table.add_row(vec![
            display.index.to_string(),
            display.token_type.clone(),
            display.amount.to_string(),
            display.strike.clone(),
            display.expires.clone(),
            display.contract.clone(),
        ]);
    }

    let table_string = table.to_string();
    for line in table_string.lines() {
        println!("  {}", line);
    }
}
