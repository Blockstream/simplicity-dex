use crate::common::{
    DEFAULT_CLIENT_TIMEOUT_SECS, check_file_existence, default_key_path, default_relays_path, get_valid_key_from_file,
    get_valid_urls_from_file, write_into_stdout,
};
use crate::contract_handlers;
use clap::{Parser, Subcommand};
use nostr::{EventId, PublicKey};
use nostr_relay_connector::relay_client::ClientConfig;
use nostr_relay_processor::relay_processor::{OrderPlaceEventTags, OrderReplyEventTags, RelayProcessor};
use simplicityhl::elements::OutPoint;
use std::path::PathBuf;
use std::time::Duration;
use tracing::instrument;

#[derive(Parser)]
pub struct Cli {
    #[arg(
        short = 'k',
        long,
        help = "Specify private key for posting authorized events on Nostr Relay",
        value_parser = check_file_existence
    )]
    key_path: Option<PathBuf>,
    #[arg(
        short = 'r',
        long, help = "Specify file with list of relays to use",
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
    #[command(flatten)]
    Dex(DexCommands),
    #[command(flatten)]
    Helpers(HelperCommands),
}

#[derive(Debug, Subcommand)]
enum DexCommands {
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
enum HelperCommands {
    #[command(about = "Display P2PK address, which will be used for testing purposes [only testing purposes]")]
    Address {
        /// Account index to use for change address
        #[arg(long = "account-index", default_value = "0")]
        account_index: u32,
    },
    #[command(about = "Create test tokens for user to put some collateral values in order [only testing purposes]")]
    Faucet {
        /// Transaction id (hex) and output index (vout) of the LBTC UTXO used to pay fees and issue the asset
        #[arg(long = "fee-utxo")]
        fee_utxo_outpoint: OutPoint,
        /// Asset name
        #[arg(long = "asset-name")]
        asset_name: String,
        /// Amount to issue of the asset in its satoshi units
        #[arg(long = "issue-sats")]
        issue_amount: u64,
        /// Miner fee in satoshis (LBTC). A separate fee output is added.
        #[arg(long = "fee-sats")]
        fee_amount: u64,
        /// Account index to use for change address
        #[arg(long = "account-index", default_value = "0")]
        account_index: u32,
        /// When set, broadcast the built transaction via Esplora and print txid
        #[arg(long = "broadcast", default_value = "true")]
        broadcast: bool,
    },
    #[command(about = "Splits given utxo into given amount of outs [only testing purposes]")]
    SplitUtxo {
        #[arg(long = "split-amount")]
        split_amount: u64,
        /// Fee utxo
        #[arg(long = "fee-utxo")]
        fee_utxo: OutPoint,
        #[arg(long = "fee-amount")]
        fee_amount: u64,
        /// Account index to use for change address
        #[arg(long = "account-index", default_value = "0")]
        account_index: u32,
        /// When set, broadcast the built transaction via Esplora and print txid
        #[arg(long = "broadcast", default_value = "true")]
        broadcast: bool,
    },
}

#[derive(Debug, Subcommand)]
enum MakerCommands {
    #[command(about = "Responsible for minting three distinct types of tokens. \
        These tokens represent the claims of the Maker and Taker on the collateral and \
        settlement assets they have deposited into the contract (used to manage \
        the contract's lifecycle, including early termination and final settlement)")]
    InitOrder,
    #[command(about = "Constructs funding transaction, which transfers appropriate users tokens \
        onto contract address. Creates order as Maker on Relays specified [authentication required]")]
    PlaceOrder {
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
    #[command(about = "Allows the Maker to withdraw their collateral from the \
        Dual Currency Deposit (DCD) contract by returning their grantor collateral tokens")]
    TerminationCollateral,
    #[command(about = "Allows the Maker to withdraw their settlement asset from the \
        Dual Currency Deposit (DCD) contract by returning their grantor settlement tokens")]
    TerminationSettlement,
    #[command(about = "Allows the Maker to settle their position at the contract's maturity, \
        receiving either the collateral or the settlement asset based on an \
        oracle-provided price")]
    Settlement,
}

#[derive(Debug, Subcommand)]
enum TakerCommands {
    #[command(
        about = "Allows a Taker to exit the Dual Currency Deposit (DCD) contract before its expiry \
            by returning their filler tokens in exchange for their original collateral."
    )]
    TerminationEarly,
    #[command(about = "Allows the Taker to settle their position at the contract's maturity, \
        receiving either the collateral or the settlement asset based on an oracle-provided price")]
    Settlement,
    #[command(about = "Replies order as Taker on Relays specified [authentication required]")]
    ReplyOrder {
        #[arg(short = 'i', long)]
        maker_event_id: EventId,
        #[arg(short = 'p', long, help = " Pubkey in bech32 or hex format")]
        maker_pubkey: PublicKey,
        #[arg(short = 't', long, help = "Txid from funding transaction step", required = false)]
        tx_id: String,
    },
    #[command(about = "Funds order with settlement tokens [authentication required]")]
    FundOrder {
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
                    MakerCommands::PlaceOrder {
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
                        format!("[Maker] Creating order result: {res:#?}")
                    }
                    MakerCommands::InitOrder => {
                        let tx_res = contract_handlers::maker_init::handle()?;
                        format!("[Maker] Init order tx result: {tx_res:?}")
                    }
                    MakerCommands::TerminationCollateral => {
                        let tx_res = contract_handlers::maker_termination_collateral::handle()?;
                        format!("[Maker] Termination collateral tx result: {tx_res:?}")
                    }
                    MakerCommands::TerminationSettlement => {
                        let tx_res = contract_handlers::maker_termination_settlement::handle()?;
                        format!("[Maker] Termination settlement tx result: {tx_res:?}")
                    }
                    MakerCommands::Settlement => {
                        let tx_res = contract_handlers::maker_settlement::handle()?;
                        format!("[Maker] Final settlement tx result: {tx_res:?}")
                    }
                },
                Command::Taker { action } => match action {
                    TakerCommands::ReplyOrder {
                        maker_event_id,
                        maker_pubkey,
                        tx_id,
                    } => {
                        let tx_res = contract_handlers::taker_funding::handle()?;
                        format!("[Taker] Tx sending result: {tx_res:?}")
                    }
                    TakerCommands::FundOrder {
                        maker_event_id,
                        maker_pubkey,
                        tx_id,
                    } => {
                        let res = relay_processor
                            .reply_order(maker_event_id, maker_pubkey, OrderReplyEventTags { tx_id })
                            .await?;
                        format!("[Taker] Replying order result: {res:#?}")
                    }
                    TakerCommands::TerminationEarly => {
                        let tx_res = contract_handlers::taker_early_termination::handle()?;
                        format!("[Taker] Early termination tx result: {tx_res:?}")
                    }
                    TakerCommands::Settlement => {
                        let tx_res = contract_handlers::taker_settlement::handle()?;
                        format!("[Taker] Final settlement tx result: {tx_res:?}")
                    }
                },
                Command::Helpers(x) => match x {
                    HelperCommands::Faucet {
                        fee_utxo_outpoint,
                        asset_name,
                        issue_amount,
                        fee_amount,
                        account_index,
                        broadcast,
                    } => {
                        let tx_res = contract_handlers::faucet::handle(
                            account_index,
                            asset_name,
                            fee_utxo_outpoint,
                            issue_amount,
                            fee_amount,
                            broadcast,
                        )?;
                        format!("Faucet tx result: {tx_res:?}")
                    }
                    HelperCommands::SplitUtxo {
                        split_amount,
                        fee_utxo,
                        fee_amount,
                        account_index,
                        broadcast,
                    } => {
                        let tx_res = contract_handlers::split_utxo::handle(
                            account_index,
                            split_amount,
                            fee_utxo,
                            fee_amount,
                            broadcast,
                        )?;
                        format!("Split utxo result tx_id: {tx_res:?}")
                    }
                    HelperCommands::Address { account_index: index } => {
                        let (x_only_pubkey, addr) = contract_handlers::address::handle(index)?;
                        format!("X Only Public Key: '{}', P2PK Address: '{}'", x_only_pubkey, addr)
                    }
                },
                Command::Dex(x) => match x {
                    DexCommands::GetOrderReplies { event_id } => {
                        let res = relay_processor.get_order_replies(event_id).await?;
                        format!("Order '{event_id}' replies: {res:#?}")
                    }
                    DexCommands::ListOrders => {
                        let res = relay_processor.list_orders().await?;
                        format!("List of available orders: {res:#?}")
                    }
                    DexCommands::GetEventsById { event_id } => {
                        let res = relay_processor.get_events_by_id(event_id).await?;
                        format!("List of available events: {res:#?}")
                    }
                },
            }
        };
        write_into_stdout(msg)?;
        Ok(())
    }
}
