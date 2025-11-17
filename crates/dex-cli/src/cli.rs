use crate::utils::{
    DEFAULT_CLIENT_TIMEOUT_SECS, check_file_existence, default_key_path, default_relays_path, get_valid_key_from_file,
    get_valid_urls_from_file, write_into_stdout,
};
use clap::{Parser, Subcommand};
use nostr::{EventId, PublicKey};

use dex_nostr_relay::relay_client::ClientConfig;
use dex_nostr_relay::relay_processor::{OrderPlaceEventTags, OrderReplyEventTags, RelayProcessor};
use std::path::PathBuf;
use std::time::Duration;
use tracing::instrument;

#[derive(Parser)]
pub struct Cli {
    /// Specify private key for posting authorized events on Nostr Relay
    #[arg(
        short = 'k',
        long,
        value_parser = check_file_existence
    )]
    key_path: Option<PathBuf>,
    /// Specify file with list of relays to use
    #[arg(
        short = 'r',
        long,
        value_parser = check_file_existence
    )]
    relays_path: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Commands collection for the maker role")]
    Maker {
        #[command(subcommand)]
        action: MakerCommands,
    },
    #[command(about = "Commands collection for the taker role")]
    Taker {
        #[command(subcommand)]
        action: TakerCommands,
    },
    #[command(about = "Get replies for a specific order by its ID [no authentication required]")]
    GetOrderReplies {
        #[arg(short = 'i', long)]
        event_id: EventId,
    },
    #[command(about = "List available orders from relays [no authentication required]")]
    ListOrders,
    #[command(about = "Get events by its ID [no authentication required]")]
    GetEventsById {
        #[arg(short = 'i', long)]
        event_id: EventId,
    },
}

#[derive(Debug, Subcommand)]
enum MakerCommands {
    #[command(about = "Create order as Maker on Relays specified [authentication required]")]
    CreateOrder {
        #[arg(short = 's', long, default_value = "")]
        asset_to_sell: String,
        #[arg(short = 'b', long, default_value = "")]
        asset_to_buy: String,
        #[arg(short = 'p', long, default_value = "0")]
        price: u64,
        #[arg(short = 'e', long, default_value = "0")]
        expiry: u64,
        #[arg(short = 'c', long, default_value = "")]
        compiler_name: String,
        #[arg(short = 's', long, default_value = "")]
        compiler_build_hash: String,
    },
}

#[derive(Debug, Subcommand)]
enum TakerCommands {
    #[command(about = "Reply order as Taker on Relays specified [authentication required]")]
    ReplyOrder {
        #[arg(short = 'i', long)]
        maker_event_id: EventId,
        #[arg(short = 'p', long, help = " Pubkey in bech32 or hex format")]
        maker_pubkey: PublicKey,
        #[arg(short = 't', long, help = "Txid from funding transaction step", required = false)]
        tx_id: String,
    },
}

impl Cli {
    #[instrument(skip(self))]
    pub async fn process(self) -> crate::error::Result<()> {
        let keys = {
            match get_valid_key_from_file(&self.key_path.unwrap_or(default_key_path())) {
                Ok(keys) => Some(keys),
                Err(err) => {
                    tracing::warn!("Failed to parse key, {err}");
                    None
                }
            }
        };
        let relays_urls = get_valid_urls_from_file(&self.relays_path.unwrap_or(default_relays_path()))?;
        let relay_processor = RelayProcessor::try_from_config(
            relays_urls,
            keys,
            ClientConfig {
                timeout: Duration::from_secs(DEFAULT_CLIENT_TIMEOUT_SECS),
            },
        )
        .await?;

        let msg = {
            match self.command {
                Command::Maker { action } => match action {
                    MakerCommands::CreateOrder {
                        asset_to_sell,
                        asset_to_buy,
                        price,
                        expiry,
                        compiler_name,
                        compiler_build_hash,
                    } => {
                        let res = relay_processor
                            .place_order(OrderPlaceEventTags {
                                asset_to_sell,
                                asset_to_buy,
                                price,
                                expiry,
                                compiler_name,
                                compiler_build_hash,
                            })
                            .await?;
                        format!("Creating order result: {res:#?}")
                    }
                },
                Command::Taker { action } => match action {
                    TakerCommands::ReplyOrder {
                        maker_event_id,
                        maker_pubkey,
                        tx_id,
                    } => {
                        let res = relay_processor
                            .reply_order(maker_event_id, maker_pubkey, OrderReplyEventTags { tx_id })
                            .await?;
                        format!("Replying order result: {res:#?}")
                    }
                },
                Command::GetOrderReplies { event_id } => {
                    let res = relay_processor.get_order_replies(event_id).await?;
                    format!("Order '{event_id}' replies: {res:#?}")
                }
                Command::ListOrders => {
                    let res = relay_processor.list_orders().await?;
                    format!("List of available orders: {res:#?}")
                }
                Command::GetEventsById { event_id } => {
                    let res = relay_processor.get_events_by_id(event_id).await?;
                    format!("List of available events: {res:#?}")
                }
            }
        };
        write_into_stdout(msg)?;
        Ok(())
    }
}
