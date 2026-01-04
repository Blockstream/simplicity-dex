use crate::cli::Cli;
use crate::cli::interactive::{
    SwapDisplay, TokenDisplay, display_swap_table, display_token_table, format_relative_time, format_settlement_asset,
    truncate_with_ellipsis,
};
use crate::config::Config;
use crate::error::Error;

use options_relay::{OptionCreatedEvent, SwapCreatedEvent};
use simplicityhl::elements::AssetId;
use simplicityhl::elements::hex::ToHex;

impl Cli {
    pub(crate) async fn run_browse(&self, config: Config) -> Result<(), Error> {
        let client = self.get_read_only_client(&config).await?;

        println!("Browsing available options and swaps from NOSTR...");
        println!();

        let options_results = client.fetch_options(config.address_params()).await?;
        let valid_options: Vec<OptionCreatedEvent> = options_results.into_iter().filter_map(Result::ok).collect();

        println!("Available Options:");
        println!("------------------");

        if valid_options.is_empty() {
            println!("  (No options found)");
        } else {
            let option_displays: Vec<TokenDisplay> = valid_options
                .iter()
                .enumerate()
                .map(|(idx, event)| {
                    let args = &event.options_args;
                    TokenDisplay {
                        index: idx + 1,
                        outpoint: event.utxo.to_string(),
                        collateral: format_asset_amount(args.collateral_per_contract(), args.get_collateral_asset_id()),
                        settlement: format_asset_amount(args.settlement_per_contract(), args.get_settlement_asset_id()),
                        expires: format_relative_time(i64::from(args.expiry_time())),
                        status: format!("by {}", truncate_with_ellipsis(&event.pubkey.to_hex(), 12)),
                    }
                })
                .collect();

            display_token_table(&option_displays);
        }

        println!();

        let swaps_results = client.fetch_swaps(config.address_params()).await?;
        let valid_swaps: Vec<SwapCreatedEvent> = swaps_results.into_iter().filter_map(Result::ok).collect();

        println!("Available Swaps (from NOSTR):");
        println!("-----------------------------");

        if valid_swaps.is_empty() {
            println!("  (No swaps found)");
        } else {
            let swap_displays: Vec<SwapDisplay> = valid_swaps
                .iter()
                .enumerate()
                .map(|(idx, event)| {
                    let args = &event.swap_args;
                    SwapDisplay {
                        index: idx + 1,
                        event_id: truncate_with_ellipsis(&event.event_id.to_hex(), 16),
                        offering: format!("{}", args.collateral_per_contract()),
                        wants: format_settlement_asset(&args.get_settlement_asset_id()),
                        expires: format_relative_time(i64::from(args.expiry_time())),
                        seller: truncate_with_ellipsis(&event.pubkey.to_hex(), 12),
                    }
                })
                .collect();

            display_swap_table(&swap_displays);
            println!("  (Note: Actual availability shown in `swap take` after syncing)");
        }

        client.disconnect().await;

        println!();
        println!("To interact with these offers:");
        println!("  1. Run `sync nostr` to sync events to your local wallet");
        println!("  2. Run `sync spent` to update UTXO status from blockchain");
        println!("  3. Run `swap take` to take a swap offer");

        Ok(())
    }
}

fn format_asset_amount(amount: u64, asset_id: AssetId) -> String {
    let hex = asset_id.to_hex();
    let prefix = &hex[..hex.len().min(8)];

    format!("{amount} ({prefix}...)")
}
