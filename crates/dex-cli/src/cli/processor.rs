use crate::cli::helper::HelperCommands;
use crate::cli::{DexCommands, MakerCommands, TakerCommands};
use crate::common::config::{AggregatedConfig, CliConfigArgs, KeysWrapper};
use crate::common::{DEFAULT_CLIENT_TIMEOUT_SECS, check_file_existence, write_into_stdout};
use crate::contract_handlers;
use clap::{Parser, Subcommand};
use dex_nostr_relay::relay_client::ClientConfig;
use dex_nostr_relay::relay_processor::RelayProcessor;
use nostr::{Keys, RelayUrl};
use simplicityhl::elements::Txid;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use tracing::instrument;

#[derive(Parser)]
pub struct Cli {
    #[arg(
        short = 'k',
        long,
        help = "Private key used to authenticate and sign events on the Nostr relays (hex or bech32)"
    )]
    nostr_key: Option<Keys>,
    #[arg(
        short = 'r',
        long,
        help = "List of Nostr relay URLs to connect to (e.g. wss://relay.example.com)",
        value_delimiter = ','
    )]
    relays_list: Option<Vec<RelayUrl>>,
    #[arg(
        short = 'c',
        long,
        help = "Path to a config file containing the list of relays and(or) nostr keypair to use",
        value_parser = check_file_existence
    )]
    nostr_config_path: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Maker-side commands for creating and managing DCD orders")]
    Maker {
        #[command(subcommand)]
        action: MakerCommands,
    },
    #[command(about = "Taker-side commands for funding and managing DCD positions")]
    Taker {
        #[command(subcommand)]
        action: TakerCommands,
    },
    #[command(flatten)]
    Dex(DexCommands),
    #[command(flatten)]
    Helpers(HelperCommands),
    #[command(about = "Print the aggregated CLI and relay configuration")]
    TestConfigShow,
}

impl Cli {
    pub fn init_config(&self) -> crate::error::Result<AggregatedConfig> {
        let nostr_key = self.nostr_key.clone();
        let relays_list = self.relays_list.clone();
        let nostr_config_path = self.nostr_config_path.clone();
        AggregatedConfig::new(CliConfigArgs {
            nostr_key: nostr_key.map(KeysWrapper),
            relays_list,
            nostr_config_path,
        })
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
                Command::TestConfigShow => {
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
                    } => {
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
                        format!("[Maker] Termination collateral tx result: {tx_id:?}")
                    }
                    MakerCommands::TerminationSettlement {
                        fee_utxos,
                        fee_amount,
                        grantor_settlement_amount_to_burn,
                        dcd_taproot_pubkey_gen,
                        dcd_arguments,
                        account_index,
                        broadcast,
                    } => {
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
                        format!("[Maker] Termination settlement tx result: {tx_id:?}")
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
                    } => {
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
                        format!("[Maker] Final settlement tx result: {tx_id:?}")
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
                    } => {
                        //todo: add reply logic
                        let processed_args = contract_handlers::taker_funding::process_args(
                            account_index,
                            dcd_arguments,
                            dcd_taproot_pubkey_gen,
                            fee_utxos,
                            collateral_amount_to_deposit,
                        )?;
                        let (tx_id, args_to_save) =
                            contract_handlers::taker_funding::handle(processed_args, fee_amount, broadcast)?;
                        contract_handlers::taker_funding::save_args_to_cache(&args_to_save)?;
                        format!("[Taker] Tx fund sending result: {tx_id:?}")
                    }
                    TakerCommands::TerminationEarly {
                        fee_utxos,
                        fee_amount,
                        filler_token_amount_to_return,
                        dcd_taproot_pubkey_gen,
                        dcd_arguments,
                        account_index,
                        broadcast,
                    } => {
                        let processed_args = contract_handlers::taker_early_termination::process_args(
                            account_index,
                            dcd_arguments,
                            dcd_taproot_pubkey_gen,
                            fee_utxos,
                            filler_token_amount_to_return,
                        )?;
                        let (tx_id, args_to_save) =
                            contract_handlers::taker_early_termination::handle(processed_args, fee_amount, broadcast)?;
                        contract_handlers::taker_early_termination::save_args_to_cache(&args_to_save)?;
                        format!("[Taker] Early termination tx result: {tx_id:?}")
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
                    } => {
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
                        format!("[Taker] Final settlement tx result: {tx_id:?}")
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
                        let body = format_items(&res, |e| e.to_string());
                        format!("List of available orders:\n{body}")
                    }
                    DexCommands::GetEventsById { event_id } => {
                        let res = relay_processor.get_event_by_id(event_id).await?;
                        format!("List of available events: {res:#?}")
                    }
                    DexCommands::GetOrderById { event_id } => {
                        let res = relay_processor.get_order_by_id(event_id).await?;
                        let body = format_items(&res, |e| e.to_string());
                        format!("Order {event_id}: {body}")
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
