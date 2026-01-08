use crate::cli::interactive::{TokenDisplay, SwapDisplay};
use crate::cli::positions::{CollateralDisplay, UserTokenDisplay};


pub fn display_token_table(tokens: &[TokenDisplay]) { 
    if tokens.is_empty() {
        println!("  (No tokens found)");
        return;
    }

    println!(
        "  {:<3} | {:<18} | {:<14} | {:<18} | Contract",
        "#", "Collateral/Token", "Strike/Token", "Expires"
    );
    println!("{}", "-".repeat(80));

    for token in tokens {
        println!(
            "  {:<3} | {:<18} | {:<14} | {:<18} | {}",
            token.index, token.collateral, token.settlement, token.expires, token.status
        );
    }
}

pub fn display_swap_table(swaps: &[SwapDisplay]) { 
    if swaps.is_empty() {
        println!("  (No swaps found)");
        return;
    }

    println!(
        "  {:<3} | {:<20} | {:<14} | {:<15} | Seller",
        "#", "Price", "Wants", "Expires"
    );
    println!("{}", "-".repeat(80));

    for swap in swaps {
        println!(
            "  {:<3} | {:<20} | {:<14} | {:<15} | {}",
            swap.index, swap.offering, swap.wants, swap.expires, swap.seller
        );
    }
}


pub fn display_collateral_table(displays: &[CollateralDisplay]) { 
    if displays.is_empty() {
        println!("  (No locked assets found)");
        return;
    }

    println!(
        "  {:<3} | {:<18} | {:<14} | {:<18} | Contract",
        "#", "Locked Assets", "Settlement", "Expires"
    );
    println!("{}", "-".repeat(80));

    for display in displays {
        println!(
            "  {:<3} | {:<18} | {:<14} | {:<18} | {}",
            display.index, display.collateral, display.settlement, display.expires, display.contract
        );
    }
}

pub fn display_user_token_table(displays: &[UserTokenDisplay]) { 
    if displays.is_empty() {
        println!("  (No option/grantor tokens found)");
        return;
    }

    println!(
        "  {:<3} | {:<8} | {:<10} | {:<14} | {:<18} | Contract",
        "#", "Type", "Amount", "Strike/Token", "Expires"
    );
    println!("{}", "-".repeat(90));

    for display in displays {
        println!(
            "  {:<3} | {:<8} | {:<10} | {:<14} | {:<18} | {}",
            display.index, display.token_type, display.amount, display.strike, display.expires, display.contract
        );
    }
}
