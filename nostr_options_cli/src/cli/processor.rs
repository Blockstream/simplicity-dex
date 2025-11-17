use crate::cli::helper::HelperCommands;
use crate::cli::{DexCommands, MakerCommands, TakerCommands};
use crate::common::{
    DCDCliArguments, DEFAULT_CLIENT_TIMEOUT_SECS, InitOrderArgs, check_file_existence, default_key_path,
    default_relays_path, derive_oracle_pubkey, get_valid_key_from_file, get_valid_urls_from_file, vec_to_arr,
    write_into_stdout,
};
use crate::contract_handlers;
use clap::{Args, Parser, Subcommand};
use dcd_manager::manager::types::AssetEntropyHex;
use nostr::{EventId, PublicKey};
use nostr_relay_connector::relay_client::ClientConfig;
use nostr_relay_processor::relay_processor::{OrderPlaceEventTags, OrderReplyEventTags, RelayProcessor};
use simplicityhl::elements::OutPoint;
use simplicityhl::elements::Txid;
use simplicityhl::elements::bitcoin::secp256k1;
use std::path::PathBuf;
use std::str::FromStr;
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
pub enum Command {
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

impl Cli {
    pub async fn init_relays(&self) -> crate::error::Result<RelayProcessor> {
        let keys = {
            match get_valid_key_from_file(&self.key_path.clone().unwrap_or(default_key_path())) {
                Ok(keys) => Some(keys),
                Err(err) => {
                    tracing::warn!("Failed to parse key, {err}");
                    None
                }
            }
        };
        let relays_urls = get_valid_urls_from_file(&self.relays_path.clone().unwrap_or(default_relays_path()))?;
        let relay_processor = RelayProcessor::try_from_config(
            relays_urls,
            keys,
            ClientConfig {
                timeout: Duration::from_secs(DEFAULT_CLIENT_TIMEOUT_SECS),
            },
        )
        .await?;
        Ok(relay_processor)
    }

    #[instrument(skip(self))]
    pub async fn process(self) -> crate::error::Result<()> {
        let relay_processor = self.init_relays().await?;
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
                    MakerCommands::Fund {
                        fee_utxos,
                        token_entropies,
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
                            token_entropies,
                        )?;
                        let event_to_publish = processed_args.extract_event();
                        let tx_id = contract_handlers::maker_funding::handle(processed_args, fee_amount, broadcast)?;
                        // contract_handlers::maker_init::save_args_to_cache(args_to_save)?;
                        let res = relay_processor
                            .place_order(
                                event_to_publish,
                                Txid::from_str("87a4c9b2060ff698d9072d5f95b3dde01efe0994f95c3cd6dd7348cb3a4e4e40")
                                    .unwrap(),
                            )
                            .await?;
                        format!("[Maker] Creating order, tx_id: {tx_id}, event_id: {res:#?}")
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
                        todo!();
                        // let tx_res = contract_handlers::maker_settlement::handle()?;
                        // format!("[Maker] Final settlement tx result: {tx_res:?}")
                        "".to_string()
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
                        split_parts: split_amount,
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
