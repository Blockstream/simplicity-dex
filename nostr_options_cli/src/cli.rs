use crate::common::{
    DCDCliArguments, DEFAULT_CLIENT_TIMEOUT_SECS, check_file_existence, default_key_path, default_relays_path,
    derive_oracle_pubkey, get_valid_key_from_file, get_valid_urls_from_file, vec_to_arr, write_into_stdout,
};
use crate::contract_handlers;
use crate::contract_handlers::maker_init::InnerDcdInitParams;
use clap::{Args, Parser, Subcommand};
use dcd_manager::manager::init::DcdInitParams;
use dcd_manager::manager::types::{AssetEntropyHex, COLLATERAL_ASSET_ID};
use nostr::{EventId, PublicKey};
use nostr_relay_connector::relay_client::ClientConfig;
use nostr_relay_processor::relay_processor::{OrderPlaceEventTags, OrderReplyEventTags, RelayProcessor};
use simplicity_contracts::DCDArguments;
use simplicityhl::elements::OutPoint;
use simplicityhl::elements::bitcoin::secp256k1;
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
        #[arg(long = "account-index", default_value_t = 0)]
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
        #[arg(long = "issue-sats", default_value_t = 1000000000000000)]
        issue_amount: u64,
        /// Miner fee in satoshis (LBTC). A separate fee output is added.
        #[arg(long = "fee-sats", default_value_t = 500)]
        fee_amount: u64,
        /// Account index to use for change address
        #[arg(long = "account-index", default_value_t = 0)]
        account_index: u32,
        /// When set, broadcast the built transaction via Esplora and print txid
        #[arg(long = "broadcast", default_value_t = true)]
        broadcast: bool,
    },
    #[command(about = "Mint already created test tokens from already saved asset [only testing purposes]")]
    MintTokens {
        /// Transaction id (hex) and output index (vout) of the REISSUANCE ASSET UTXO you will spend
        #[arg(long = "reissue-asset-utxo")]
        reissue_asset_outpoint: OutPoint,
        /// Transaction id (hex) and output index (vout) of the LBTC UTXO used to pay fees and reissue the asset
        #[arg(long = "fee-utxo")]
        fee_utxo_outpoint: OutPoint,
        /// Asset name
        #[arg(long = "asset-name")]
        asset_name: String,
        /// Amount to reissue of the asset in its satoshi units
        #[arg(long = "reissue-sats", default_value_t = 1000000000000000)]
        reissue_amount: u64,
        /// Miner fee in satoshis (LBTC). A separate fee output is added.
        #[arg(long = "fee-sats", default_value_t = 500)]
        fee_amount: u64,
        /// Account index to use for change address
        #[arg(long = "account-index", default_value_t = 0)]
        account_index: u32,
        /// When set, broadcast the built transaction via Esplora and print txid
        #[arg(long = "broadcast", default_value_t = true)]
        broadcast: bool,
    },
    #[command(about = "Splits given utxo into given amount of outs [only testing purposes]")]
    SplitUtxo {
        #[arg(long = "split-amount")]
        split_amount: u64,
        /// Fee utxo
        #[arg(long = "fee-utxo")]
        fee_utxo: OutPoint,
        #[arg(long = "fee-amount", default_value_t = 500)]
        fee_amount: u64,
        /// Account index to use for change address
        #[arg(long = "account-index", default_value_t = 0)]
        account_index: u32,
        /// When set, broadcast the built transaction via Esplora and print txid
        #[arg(long = "broadcast", default_value_t = true)]
        broadcast: bool,
    },
}
#[derive(Debug, Args)]
struct InitOrderArgs {
    /// Taker funding start time
    #[arg(long = "taker-funding-start-time")]
    taker_funding_start_time: u32,
    /// Taker funding end time
    #[arg(long = "taker-funding-end-time")]
    taker_funding_end_time: u32,
    /// Contract expiry time
    #[arg(long = "contract-expiry-time")]
    contract_expiry_time: u32,
    /// Early termination end time
    #[arg(long = "early-termination-end-time")]
    early_termination_end_time: u32,
    /// Settlement height
    #[arg(long = "settlement-height")]
    settlement_height: u32,
    /// Principal collateral amount
    #[arg(long = "principal-collateral-amount")]
    principal_collateral_amount: u64,
    /// Incentive basis points
    #[arg(long = "incentive-basis-points")]
    incentive_basis_points: u64,
    /// Filler per principal collateral
    #[arg(long = "filler-per-principal-collateral")]
    filler_per_principal_collateral: u64,
    /// Strike price
    #[arg(long = "strike-price")]
    strike_price: u64,
    /// Settlement asset id
    #[arg(long = "settlement-asset-id")]
    settlement_asset_id: String,
    /// Oracle public key
    #[arg(long = "oracle-public-key", default_value_t = derive_oracle_pubkey().unwrap())]
    oracle_public_key: secp256k1::PublicKey,
}

impl From<InitOrderArgs> for InnerDcdInitParams {
    fn from(args: InitOrderArgs) -> Self {
        InnerDcdInitParams {
            taker_funding_start_time: args.taker_funding_start_time,
            taker_funding_end_time: args.taker_funding_end_time,
            contract_expiry_time: args.contract_expiry_time,
            early_termination_end_time: args.early_termination_end_time,
            settlement_height: args.settlement_height,
            principal_collateral_amount: args.principal_collateral_amount,
            incentive_basis_points: args.incentive_basis_points,
            filler_per_principal_collateral: args.filler_per_principal_collateral,
            strike_price: args.strike_price,
            collateral_asset_id: COLLATERAL_ASSET_ID.to_string(),
            settlement_asset_id: args.settlement_asset_id,
            oracle_public_key: args.oracle_public_key,
        }
    }
}
#[derive(Debug, Subcommand)]
enum MakerCommands {
    #[command(about = "Responsible for minting three distinct types of tokens. \
        These tokens represent the claims of the Maker and Taker on the collateral and \
        settlement assets they have deposited into the contract (used to manage \
        the contract's lifecycle, including early termination and final settlement)")]
    InitOrder {
        /// Utxos to construct assets on them
        #[arg(long = "fee-utxos")]
        fee_utxos: Vec<OutPoint>,
        #[command(flatten)]
        init_order_args: InitOrderArgs,
        /// Fee amount
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Account index to use for change address
        #[arg(long = "account-index", default_value_t = 0)]
        account_index: u32,
        /// When set, broadcast the built transaction via Esplora and print txid
        #[arg(long = "broadcast", default_value_t = true)]
        broadcast: bool,
    },
    #[command(about = "Constructs funding transaction, which transfers appropriate users tokens \
        onto contract address. Creates order as Maker on Relays specified [authentication required]")]
    FundAndPlaceOrder {
        /// Expects only 5 reissue utxos in this order (filler_token, grantor_collateral_token, grantor_settlement_token, settlement_asset, fee_utxo)
        #[arg(long = "fee-utxos")]
        fee_utxos: Vec<OutPoint>,
        /// Expects only 5 assets hex entropy in BE (filler_token, grantor_collateral_token, grantor_settlement_token, settlement_asset, fee_utxo)
        #[arg(long = "fee-utxos")]
        tokens_entropies: Vec<AssetEntropyHex>,
        #[command(flatten)]
        init_order_args: InitOrderArgs,
        /// Fee amount
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Storage taproot pubkey gen
        #[arg(long = "taproot-pubkey-gen")]
        dcd_taproot_pubkey_gen: String,
        #[command(flatten)]
        dcd_arguments: Option<DCDCliArguments>,
        /// Account index to use for change address
        #[arg(long = "account-index", default_value_t = 0)]
        account_index: u32,
        /// When set, broadcast the built transaction via Esplora and print txid
        #[arg(long = "broadcast", default_value_t = true)]
        broadcast: bool,
        //TODO: review params
        #[arg(short = 's', long, default_value = "")]
        asset_to_sell: String,
        #[arg(short = 'b', long, default_value = "")]
        asset_to_buy: String,
        #[arg(short = 'p', long, default_value_t = 0)]
        price: u64,
        #[arg(short = 'e', long, default_value_t = 0)]
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
                    MakerCommands::InitOrder {
                        fee_utxos,
                        init_order_args,
                        fee_amount,
                        account_index,
                        broadcast,
                    } => {
                        let processed_args =
                            contract_handlers::maker_init::process_args(account_index, init_order_args.into())?;
                        let (tx_res, args_to_save) = contract_handlers::maker_init::handle(
                            processed_args,
                            vec_to_arr::<3, OutPoint>(fee_utxos)?,
                            fee_amount,
                            broadcast,
                        )?;
                        contract_handlers::maker_init::save_args_to_cache(args_to_save)?;
                        format!("[Maker] Init order tx result: {tx_res:?}")
                    }
                    MakerCommands::FundAndPlaceOrder {
                        fee_utxos,
                        tokens_entropies,
                        init_order_args,
                        fee_amount,
                        dcd_taproot_pubkey_gen,
                        dcd_arguments,
                        account_index,
                        broadcast,
                        asset_to_sell,
                        asset_to_buy,
                        price,
                        expiry,
                        compiler_name,
                        compiler_build_hash,
                    } => {
                        let dcd_arguments = contract_handlers::maker_funding::process_args(
                            account_index,
                            dcd_arguments,
                            dcd_taproot_pubkey_gen,
                            fee_utxos,
                            tokens_entropies,
                        )?;
                        let tx_res = contract_handlers::maker_funding::handle(dcd_arguments, fee_amount, broadcast)?;
                        // contract_handlers::maker_init::save_args_to_cache(args_to_save)?;
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
                        let tx_res = contract_handlers::faucet::create_asset(
                            account_index,
                            asset_name,
                            fee_utxo_outpoint,
                            fee_amount,
                            issue_amount,
                            broadcast,
                        )?;
                        format!("Faucet tx result: {tx_res:?}")
                    }
                    HelperCommands::MintTokens {
                        reissue_asset_outpoint,
                        fee_utxo_outpoint,
                        asset_name,
                        reissue_amount,
                        fee_amount,
                        account_index,
                        broadcast,
                    } => {
                        let tx_res = contract_handlers::faucet::mint_asset(
                            account_index,
                            asset_name,
                            reissue_asset_outpoint,
                            fee_utxo_outpoint,
                            reissue_amount,
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
