use crate::cli::helper::HelperCommands;
use crate::cli::{DexCommands, MakerCommands, TakerCommands};
use crate::common::config::AggregatedConfig;
use crate::common::{DEFAULT_CLIENT_TIMEOUT_SECS, write_into_stdout};
use crate::contract_handlers;
use clap::{Parser, Subcommand};
use dex_nostr_relay::relay_client::ClientConfig;
use dex_nostr_relay::relay_processor::{ListOrdersEventFilter, RelayProcessor};
use dex_nostr_relay::types::ReplyOption;
use nostr::{Keys, RelayUrl, Timestamp};
use simplicityhl::elements::Txid;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use tracing::instrument;

pub(crate) const DEFAULT_CONFIG_PATH: &str = ".simplicity-dex.config.toml";

#[derive(Parser)]
pub struct Cli {
    /// Private key used to authenticate and sign events on the Nostr relays (hex or bech32)
    #[arg(short = 'k', long, env = "DEX_NOSTR_KEYPAIR")]
    pub(crate) nostr_key: Option<Keys>,

    /// List of Nostr relay URLs to connect to (e.g. <wss://relay.example.com>)
    #[arg(short = 'r', long, value_delimiter = ',', env = "DEX_NOSTR_RELAYS")]
    pub(crate) relays_list: Option<Vec<RelayUrl>>,

    /// Path to a config file containing the list of relays and(or) nostr keypair to use
    #[arg(short = 'c', long, default_value = DEFAULT_CONFIG_PATH, env = "DEX_NOSTR_CONFIG_PATH")]
    pub(crate) nostr_config_path: PathBuf,

    /// Command to execute
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Maker-side commands for creating and managing DCD orders
    #[command()]
    Maker {
        #[command(subcommand)]
        action: MakerCommands,
    },

    /// Taker-side commands for funding and managing DCD positions
    #[command()]
    Taker {
        #[command(subcommand)]
        action: TakerCommands,
    },

    #[command(flatten)]
    Dex(DexCommands),

    #[command(flatten)]
    Helpers(HelperCommands),

    /// Print the aggregated CLI and relay configuration
    #[command()]
    ShowConfig,
}

impl Cli {
    pub fn init_config(&self) -> crate::error::Result<AggregatedConfig> {
        AggregatedConfig::new(self)
    }

    pub async fn init_relays(
        &self,
        relays: &[RelayUrl],
        keypair: Option<Keys>,
    ) -> crate::error::Result<RelayProcessor> {
        let relay_processor = RelayProcessor::try_from_config(
            relays,
            keypair,
            ClientConfig {
                timeout: Duration::from_secs(DEFAULT_CLIENT_TIMEOUT_SECS),
            },
        )
        .await?;
        Ok(relay_processor)
    }

    #[instrument(skip(self))]
    pub async fn process(self) -> crate::error::Result<()> {
        let agg_config = self.init_config()?;
        let relay_processor = self
            .init_relays(&agg_config.relays, agg_config.nostr_keypair.clone())
            .await?;
        let msg = {
            match self.command {
                Command::ShowConfig => {
                    format!("config: {agg_config:#?}")
                }
                Command::Maker { action } => match action {
                    MakerCommands::InitOrder {
                        fee_utxos,
                        init_order_args,
                        fee_amount,
                        account_index,
                        broadcast,
                    } => {
                        let processed_args = contract_handlers::maker_init::process_args(
                            account_index,
                            init_order_args.into(),
                            fee_utxos,
                        )?;
                        let (tx_res, args_to_save) =
                            contract_handlers::maker_init::handle(processed_args, fee_amount, broadcast)?;
                        contract_handlers::maker_init::save_args_to_cache(&args_to_save)?;
                        format!("[Maker] Init order tx result: {tx_res:?}")
                    }
                    MakerCommands::Fund {
                        fee_utxos,
                        fee_amount,
                        dcd_taproot_pubkey_gen,
                        dcd_arguments,
                        account_index,
                        broadcast,
                    } => {
                        agg_config.check_nostr_keypair_existence()?;
                        let processed_args = contract_handlers::maker_funding::process_args(
                            account_index,
                            dcd_arguments,
                            dcd_taproot_pubkey_gen,
                            fee_utxos,
                        )?;
                        let event_to_publish = processed_args.extract_event();
                        let (tx_id, args_to_save) =
                            contract_handlers::maker_funding::handle(processed_args, fee_amount, broadcast)?;
                        let res = relay_processor
                            .place_order(
                                event_to_publish,
                                Txid::from_str("87a4c9b2060ff698d9072d5f95b3dde01efe0994f95c3cd6dd7348cb3a4e4e40")
                                    .unwrap(),
                            )
                            .await?;
                        contract_handlers::maker_funding::save_args_to_cache(&args_to_save)?;
                        format!("[Maker] Creating order, tx_id: {tx_id}, event_id: {res:#?}")
                    }
                    MakerCommands::TerminationCollateral {
                        fee_utxos,
                        fee_amount,
                        dcd_taproot_pubkey_gen,
                        grantor_collateral_amount_to_burn,
                        dcd_arguments,
                        account_index,
                        broadcast,
                        maker_order_event_id,
                    } => {
                        agg_config.check_nostr_keypair_existence()?;
                        let processed_args = contract_handlers::maker_termination_collateral::process_args(
                            account_index,
                            dcd_arguments,
                            dcd_taproot_pubkey_gen,
                            fee_utxos,
                            grantor_collateral_amount_to_burn,
                        )?;
                        let (tx_id, args_to_save) = contract_handlers::maker_termination_collateral::handle(
                            processed_args,
                            fee_amount,
                            broadcast,
                        )?;
                        contract_handlers::maker_termination_collateral::save_args_to_cache(&args_to_save)?;
                        let reply_event_id = relay_processor
                            .reply_order(maker_order_event_id, ReplyOption::MakerTerminationCollateral { tx_id })
                            .await?;
                        format!("[Maker] Termination collateral tx result: {tx_id:?}, reply event id: {reply_event_id}")
                    }
                    MakerCommands::TerminationSettlement {
                        fee_utxos,
                        fee_amount,
                        grantor_settlement_amount_to_burn,
                        dcd_taproot_pubkey_gen,
                        dcd_arguments,
                        account_index,
                        broadcast,
                        maker_order_event_id,
                    } => {
                        agg_config.check_nostr_keypair_existence()?;
                        let processed_args = contract_handlers::maker_termination_settlement::process_args(
                            account_index,
                            dcd_arguments,
                            dcd_taproot_pubkey_gen,
                            fee_utxos,
                            grantor_settlement_amount_to_burn,
                        )?;
                        let (tx_id, args_to_save) = contract_handlers::maker_termination_settlement::handle(
                            processed_args,
                            fee_amount,
                            broadcast,
                        )?;
                        contract_handlers::maker_termination_settlement::save_args_to_cache(&args_to_save)?;
                        let reply_event_id = relay_processor
                            .reply_order(maker_order_event_id, ReplyOption::MakerTerminationSettlement { tx_id })
                            .await?;
                        format!(
                            "[Maker] Termination settlement tx result: {tx_id:?},  reply event id: {reply_event_id}"
                        )
                    }
                    MakerCommands::Settlement {
                        fee_utxos,
                        fee_amount,
                        price_at_current_block_height,
                        oracle_signature,
                        grantor_amount_to_burn,
                        dcd_taproot_pubkey_gen,
                        dcd_arguments,
                        account_index,
                        broadcast,
                        maker_order_event_id,
                    } => {
                        agg_config.check_nostr_keypair_existence()?;
                        let processed_args = contract_handlers::maker_settlement::process_args(
                            account_index,
                            dcd_arguments,
                            dcd_taproot_pubkey_gen,
                            fee_utxos,
                            price_at_current_block_height,
                            oracle_signature,
                            grantor_amount_to_burn,
                        )?;
                        let (tx_id, args_to_save) =
                            contract_handlers::maker_settlement::handle(processed_args, fee_amount, broadcast)?;
                        contract_handlers::maker_settlement::save_args_to_cache(&args_to_save)?;
                        let reply_event_id = relay_processor
                            .reply_order(maker_order_event_id, ReplyOption::MakerSettlement { tx_id })
                            .await?;
                        format!("[Maker] Final settlement tx result: {tx_id:?}, reply event id: {reply_event_id}")
                    }
                },
                Command::Taker { action } => match action {
                    TakerCommands::FundOrder {
                        fee_utxos,
                        fee_amount,
                        collateral_amount_to_deposit,
                        dcd_taproot_pubkey_gen,
                        dcd_arguments,
                        account_index,
                        broadcast,
                        maker_order_event_id: maker_event_id,
                    } => {
                        agg_config.check_nostr_keypair_existence()?;
                        let processed_args = contract_handlers::taker_funding::process_args(
                            account_index,
                            dcd_arguments,
                            dcd_taproot_pubkey_gen,
                            fee_utxos,
                            collateral_amount_to_deposit,
                        )?;
                        let (tx_id, args_to_save) =
                            contract_handlers::taker_funding::handle(processed_args, fee_amount, broadcast)?;
                        let reply_event_id = relay_processor
                            .reply_order(maker_event_id, ReplyOption::TakerFund { tx_id })
                            .await?;
                        contract_handlers::taker_funding::save_args_to_cache(&args_to_save)?;
                        format!("[Taker] Tx fund sending result: {tx_id:?}, reply event id: {reply_event_id}")
                    }
                    TakerCommands::TerminationEarly {
                        fee_utxos,
                        fee_amount,
                        filler_token_amount_to_return,
                        dcd_taproot_pubkey_gen,
                        dcd_arguments,
                        account_index,
                        broadcast,
                        maker_order_event_id,
                    } => {
                        agg_config.check_nostr_keypair_existence()?;
                        let processed_args = contract_handlers::taker_early_termination::process_args(
                            account_index,
                            dcd_arguments,
                            dcd_taproot_pubkey_gen,
                            fee_utxos,
                            filler_token_amount_to_return,
                        )?;
                        let (tx_id, args_to_save) =
                            contract_handlers::taker_early_termination::handle(processed_args, fee_amount, broadcast)?;
                        let reply_event_id = relay_processor
                            .reply_order(maker_order_event_id, ReplyOption::TakerTerminationEarly { tx_id })
                            .await?;
                        contract_handlers::taker_early_termination::save_args_to_cache(&args_to_save)?;
                        format!("[Taker] Early termination tx result: {tx_id:?}, reply event id: {reply_event_id}")
                    }
                    TakerCommands::Settlement {
                        fee_utxos,
                        fee_amount,
                        price_at_current_block_height,
                        filler_amount_to_burn,
                        oracle_signature,
                        dcd_taproot_pubkey_gen,
                        dcd_arguments,
                        account_index,
                        broadcast,
                        maker_order_event_id,
                    } => {
                        agg_config.check_nostr_keypair_existence()?;
                        let processed_args = contract_handlers::taker_settlement::process_args(
                            account_index,
                            dcd_arguments,
                            dcd_taproot_pubkey_gen,
                            fee_utxos,
                            price_at_current_block_height,
                            filler_amount_to_burn,
                            oracle_signature,
                        )?;
                        let (tx_id, args_to_save) =
                            contract_handlers::taker_settlement::handle(processed_args, fee_amount, broadcast)?;
                        contract_handlers::taker_settlement::save_args_to_cache(&args_to_save)?;
                        let reply_event_id = relay_processor
                            .reply_order(maker_order_event_id, ReplyOption::TakerSettlement { tx_id })
                            .await?;
                        format!("[Taker] Final settlement tx result: {tx_id:?}, reply event id: {reply_event_id}")
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
                        contract_handlers::faucet::create_asset(
                            account_index,
                            asset_name,
                            fee_utxo_outpoint,
                            fee_amount,
                            issue_amount,
                            broadcast,
                        )?;
                        "Asset creation -- done".to_string()
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
                        contract_handlers::faucet::mint_asset(
                            account_index,
                            asset_name,
                            reissue_asset_outpoint,
                            fee_utxo_outpoint,
                            reissue_amount,
                            fee_amount,
                            broadcast,
                        )?;
                        "Asset minting -- done".to_string()
                    }
                    HelperCommands::SplitNativeThree {
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
                        format!("X Only Public Key: '{x_only_pubkey}', P2PK Address: '{addr}'")
                    }
                },
                Command::Dex(x) => match x {
                    DexCommands::GetOrderReplies { event_id } => {
                        let res = relay_processor.get_order_replies(event_id).await?;
                        format!("Order '{event_id}' replies: {res:#?}")
                    }
                    DexCommands::ListOrders {
                        authors,
                        time_to_filter,
                        limit,
                    } => {
                        let (since, until) = if let Some(time_filter) = time_to_filter {
                            (time_filter.compute_since(), time_filter.compute_until())
                        } else {
                            (None, None)
                        };

                        let filter = ListOrdersEventFilter {
                            authors,
                            since: since.map(Timestamp::from),
                            until: until.map(Timestamp::from),
                            limit,
                        };

                        let res = relay_processor.list_orders(filter).await?;
                        let body = format_items(&res, std::string::ToString::to_string);
                        format!("List of available orders:\n{body}")
                    }
                    DexCommands::GetEventsById { event_id } => {
                        let res = relay_processor.get_event_by_id(event_id).await?;
                        format!("List of available events: {res:#?}")
                    }
                    DexCommands::GetOrderById { event_id } => {
                        let res = relay_processor.get_order_by_id(event_id).await?;
                        let body = format_items(&[res], std::string::ToString::to_string);
                        format!("Order {event_id}: {body}")
                    }
                    DexCommands::ImportParams { event_id } => {
                        let res = relay_processor.get_order_by_id(event_id).await?;
                        crate::common::store::store_utils::save_dcd_args(
                            &res.dcd_taproot_pubkey_gen,
                            &res.dcd_arguments,
                        )?;
                        format!("Order {event_id}: {res}")
                    }
                },
            }
        };
        write_into_stdout(msg)?;
        Ok(())
    }
}

fn format_items<T, F>(items: &[T], map: F) -> String
where
    F: Fn(&T) -> String,
{
    items.iter().map(map).collect::<Vec<_>>().join("\n")
}
