use crate::cli::helper::HelperCommands;
use crate::cli::{DexCommands, MakerCommands, TakerCommands};
use crate::common::config::AggregatedConfig;
use crate::common::{DEFAULT_CLIENT_TIMEOUT_SECS, InitOrderArgs, write_into_stdout};
use crate::contract_handlers;
use clap::{Parser, Subcommand};
use dex_nostr_relay::relay_client::ClientConfig;
use dex_nostr_relay::relay_processor::{ListOrdersEventFilter, RelayProcessor};
use dex_nostr_relay::types::ReplyOption;
use elements::hex::ToHex;
use nostr::{EventId, Keys, RelayUrl, Timestamp};
use simplicity::elements::OutPoint;
use std::path::PathBuf;
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

/// Common CLI options shared between maker/taker commands that build and (optionally) broadcast a tx.
#[derive(Debug, Clone, Copy, Parser)]
pub struct CommonOrderOptions {
    /// Account index used to derive internal/change addresses from the wallet
    #[arg(long = "account-index", default_value_t = 0)]
    pub account_index: u32,
    /// When set, the transaction would be only printed, otherwise it'd ve broadcasted the built transaction via Esplora
    #[arg(long = "offline")]
    pub is_offline: bool,
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
    /// Dex commands that is related with nostr and interaction with it
    #[command()]
    Dex {
        #[command(subcommand)]
        action: DexCommands,
    },
    /// Helper commands for ease of testing use
    #[command()]
    Helpers {
        #[command(subcommand)]
        action: HelperCommands,
    },
    /// Print the aggregated CLI and relay configuration
    #[command()]
    ShowConfig,
}

#[derive(Debug, Clone)]
struct CliAppContext {
    agg_config: AggregatedConfig,
    relay_processor: RelayProcessor,
}

struct MakerSettlementCliContext {
    grantor_collateral_token_utxo: OutPoint,
    grantor_settlement_token_utxo: OutPoint,
    fee_utxo: OutPoint,
    asset_utxo: OutPoint,
    fee_amount: u64,
    price_at_current_block_height: u64,
    oracle_signature: String,
    grantor_amount_to_burn: u64,
    maker_order_event_id: EventId,
}

struct MakerSettlementTerminationCliContext {
    fee_utxo: OutPoint,
    settlement_asset_utxo: OutPoint,
    grantor_settlement_token_utxo: OutPoint,
    fee_amount: u64,
    grantor_settlement_amount_to_burn: u64,
    maker_order_event_id: EventId,
}

struct MakerCollateralTerminationCliContext {
    grantor_collateral_token_utxo: OutPoint,
    fee_utxo: OutPoint,
    collateral_token_utxo: OutPoint,
    fee_amount: u64,
    grantor_collateral_amount_to_burn: u64,
    maker_order_event_id: EventId,
}

struct MakerFundCliContext {
    filler_token_utxo: OutPoint,
    grantor_collateral_token_utxo: OutPoint,
    grantor_settlement_token_utxo: OutPoint,
    settlement_asset_utxo: OutPoint,
    fee_utxo: OutPoint,
    fee_amount: u64,
    dcd_taproot_pubkey_gen: String,
}

struct MakerInitCliContext {
    first_lbtc_utxo: OutPoint,
    second_lbtc_utxo: OutPoint,
    third_lbtc_utxo: OutPoint,
    init_order_args: InitOrderArgs,
    fee_amount: u64,
}

struct MergeTokens2CliContext {
    token_utxo_1: OutPoint,
    token_utxo_2: OutPoint,
    fee_utxo: OutPoint,
    fee_amount: u64,
    maker_order_event_id: EventId,
}

struct MergeTokens3CliContext {
    token_utxo_1: OutPoint,
    token_utxo_2: OutPoint,
    token_utxo_3: OutPoint,
    fee_utxo: OutPoint,
    fee_amount: u64,
    maker_order_event_id: EventId,
}

struct MergeTokens4CliContext {
    token_utxo_1: OutPoint,
    token_utxo_2: OutPoint,
    token_utxo_3: OutPoint,
    token_utxo_4: OutPoint,
    fee_utxo: OutPoint,
    fee_amount: u64,
    maker_order_event_id: EventId,
}

impl Cli {
    /// Initialize aggregated CLI configuration from CLI args, config file and env.
    ///
    /// # Errors
    ///
    /// Returns an error if building or validating the aggregated configuration
    /// (including loading the config file or environment overrides) fails.
    pub fn init_config(&self) -> crate::error::Result<AggregatedConfig> {
        AggregatedConfig::new(self)
    }

    /// Initialize the relay processor using the provided relays and optional keypair.
    ///
    /// # Errors
    ///
    /// Returns an error if creating or configuring the underlying Nostr relay
    /// client fails, or if connecting to the specified relays fails.
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

    /// Process the CLI command and execute the selected action.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Loading or validating the aggregated configuration fails.
    /// - Initializing or communicating with Nostr relays fails.
    /// - Any underlying contract handler (maker, taker, or helper) fails.
    /// - Writing the resulting message to stdout fails.
    #[instrument(skip(self))]
    pub async fn process(self) -> crate::error::Result<()> {
        let agg_config = self.init_config()?;

        let relay_processor = self
            .init_relays(&agg_config.relays, agg_config.nostr_keypair.clone())
            .await?;

        let cli_app_context = CliAppContext {
            agg_config,
            relay_processor,
        };
        let msg = {
            match self.command {
                Command::ShowConfig => {
                    format!("Config: {:#?}", cli_app_context.agg_config)
                }
                Command::Maker { action } => Self::process_maker_commands(&cli_app_context, action).await?,
                Command::Taker { action } => Self::process_taker_commands(&cli_app_context, action).await?,
                Command::Helpers { action } => Self::process_helper_commands(&cli_app_context, action).await?,
                Command::Dex { action } => Self::process_dex_commands(&cli_app_context, action).await?,
            }
        };
        write_into_stdout(msg)?;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    async fn process_maker_commands(
        cli_app_context: &CliAppContext,
        action: MakerCommands,
    ) -> crate::error::Result<String> {
        Ok(match action {
            MakerCommands::InitOrder {
                first_lbtc_utxo,
                second_lbtc_utxo,
                third_lbtc_utxo,
                init_order_args,
                fee_amount,
                common_options,
            } => {
                Self::_process_maker_init_order(
                    MakerInitCliContext {
                        first_lbtc_utxo,
                        second_lbtc_utxo,
                        third_lbtc_utxo,
                        init_order_args,
                        fee_amount,
                    },
                    common_options,
                )
                .await?
            }
            MakerCommands::Fund {
                filler_token_utxo,
                grantor_collateral_token_utxo,
                grantor_settlement_token_utxo,
                settlement_asset_utxo,
                fee_utxo,
                fee_amount,
                dcd_taproot_pubkey_gen,
                common_options,
            } => {
                Self::_process_maker_fund(
                    cli_app_context,
                    MakerFundCliContext {
                        filler_token_utxo,
                        grantor_collateral_token_utxo,
                        grantor_settlement_token_utxo,
                        settlement_asset_utxo,
                        fee_utxo,
                        fee_amount,
                        dcd_taproot_pubkey_gen,
                    },
                    common_options,
                )
                .await?
            }
            MakerCommands::TerminationCollateral {
                grantor_collateral_token_utxo,
                fee_utxo,
                collateral_token_utxo,
                fee_amount,
                grantor_collateral_amount_to_burn,
                maker_order_event_id,
                common_options,
            } => {
                Self::_process_maker_termination_collateral(
                    cli_app_context,
                    MakerCollateralTerminationCliContext {
                        grantor_collateral_token_utxo,
                        fee_utxo,
                        collateral_token_utxo,
                        fee_amount,
                        grantor_collateral_amount_to_burn,
                        maker_order_event_id,
                    },
                    common_options,
                )
                .await?
            }
            MakerCommands::TerminationSettlement {
                fee_utxo,
                settlement_asset_utxo,
                grantor_settlement_token_utxo,
                fee_amount,
                grantor_settlement_amount_to_burn,
                maker_order_event_id,
                common_options,
            } => {
                Self::_process_maker_termination_settlement(
                    cli_app_context,
                    MakerSettlementTerminationCliContext {
                        fee_utxo,
                        settlement_asset_utxo,
                        grantor_settlement_token_utxo,
                        fee_amount,
                        grantor_settlement_amount_to_burn,
                        maker_order_event_id,
                    },
                    common_options,
                )
                .await?
            }
            MakerCommands::Settlement {
                grantor_collateral_token_utxo,
                grantor_settlement_token_utxo,
                asset_utxo,
                fee_utxo,
                fee_amount,
                price_at_current_block_height,
                oracle_signature,
                grantor_amount_to_burn,
                maker_order_event_id,
                common_options,
            } => {
                Self::_process_maker_settlement(
                    cli_app_context,
                    MakerSettlementCliContext {
                        grantor_collateral_token_utxo,
                        grantor_settlement_token_utxo,
                        fee_utxo,
                        asset_utxo,
                        fee_amount,
                        price_at_current_block_height,
                        oracle_signature,
                        grantor_amount_to_burn,
                        maker_order_event_id,
                    },
                    common_options,
                )
                .await?
            }
        })
    }

    async fn _process_maker_init_order(
        MakerInitCliContext {
            first_lbtc_utxo,
            second_lbtc_utxo,
            third_lbtc_utxo,
            init_order_args,
            fee_amount,
        }: MakerInitCliContext,
        CommonOrderOptions {
            account_index,
            is_offline,
        }: CommonOrderOptions,
    ) -> crate::error::Result<String> {
        use contract_handlers::maker_init::{Utxos, handle, process_args, save_args_to_cache};

        let processed_args = process_args(account_index, init_order_args.into())?;
        let (tx_res, args_to_save) = handle(
            processed_args,
            Utxos {
                first: first_lbtc_utxo,
                second: second_lbtc_utxo,
                third: third_lbtc_utxo,
            },
            fee_amount,
            is_offline,
        )
        .await?;
        save_args_to_cache(&args_to_save)?;
        Ok(format!("[Maker] Init order tx result: {tx_res:?}"))
    }

    async fn _process_maker_fund(
        CliAppContext {
            agg_config,
            relay_processor,
        }: &CliAppContext,
        MakerFundCliContext {
            filler_token_utxo,
            grantor_collateral_token_utxo,
            grantor_settlement_token_utxo,
            settlement_asset_utxo,
            fee_utxo,
            fee_amount,
            dcd_taproot_pubkey_gen,
        }: MakerFundCliContext,
        CommonOrderOptions {
            account_index,
            is_offline,
        }: CommonOrderOptions,
    ) -> crate::error::Result<String> {
        use contract_handlers::maker_funding::{Utxos, handle, process_args, save_args_to_cache};

        agg_config.check_nostr_keypair_existence()?;

        let processed_args = process_args(account_index, dcd_taproot_pubkey_gen)?;
        let event_to_publish = processed_args.extract_event();
        let (tx_id, args_to_save) = handle(
            processed_args,
            Utxos {
                filler_token: filler_token_utxo,
                grantor_collateral_token: grantor_collateral_token_utxo,
                grantor_settlement_token: grantor_settlement_token_utxo,
                settlement_asset: settlement_asset_utxo,
                fee: fee_utxo,
            },
            fee_amount,
            is_offline,
        )
        .await?;
        let res = relay_processor.place_order(event_to_publish, tx_id).await?;
        save_args_to_cache(&args_to_save)?;
        Ok(format!("[Maker] Creating order, tx_id: {tx_id}, event_id: {res:#?}"))
    }

    async fn _process_maker_termination_collateral(
        CliAppContext {
            agg_config,
            relay_processor,
        }: &CliAppContext,
        MakerCollateralTerminationCliContext {
            grantor_collateral_token_utxo,
            fee_utxo,
            collateral_token_utxo,
            fee_amount,
            grantor_collateral_amount_to_burn,
            maker_order_event_id,
        }: MakerCollateralTerminationCliContext,
        CommonOrderOptions {
            account_index,
            is_offline,
        }: CommonOrderOptions,
    ) -> crate::error::Result<String> {
        use contract_handlers::maker_termination_collateral::{Utxos, handle, save_args_to_cache};

        agg_config.check_nostr_keypair_existence()?;
        let processed_args = contract_handlers::maker_termination_collateral::process_args(
            account_index,
            grantor_collateral_amount_to_burn,
            maker_order_event_id,
            relay_processor,
        )
        .await?;
        let (tx_id, args_to_save) = handle(
            processed_args,
            Utxos {
                grantor_collateral_token: grantor_collateral_token_utxo,
                fee: fee_utxo,
                collateral_token: collateral_token_utxo,
            },
            fee_amount,
            is_offline,
        )
        .await?;
        save_args_to_cache(&args_to_save)?;
        let reply_event_id = relay_processor
            .reply_order(maker_order_event_id, ReplyOption::MakerTerminationCollateral { tx_id })
            .await?;
        Ok(format!(
            "[Maker] Termination collateral tx result: {tx_id:?}, reply event id: {reply_event_id}"
        ))
    }

    async fn _process_maker_termination_settlement(
        CliAppContext {
            agg_config,
            relay_processor,
        }: &CliAppContext,
        MakerSettlementTerminationCliContext {
            fee_utxo,
            settlement_asset_utxo,
            grantor_settlement_token_utxo,
            fee_amount,
            grantor_settlement_amount_to_burn,
            maker_order_event_id,
        }: MakerSettlementTerminationCliContext,
        CommonOrderOptions {
            account_index,
            is_offline,
        }: CommonOrderOptions,
    ) -> crate::error::Result<String> {
        use contract_handlers::maker_termination_settlement::{Utxos, handle, save_args_to_cache};

        agg_config.check_nostr_keypair_existence()?;
        let processed_args = contract_handlers::maker_termination_settlement::process_args(
            account_index,
            grantor_settlement_amount_to_burn,
            maker_order_event_id,
            relay_processor,
        )
        .await?;
        let (tx_id, args_to_save) = handle(
            processed_args,
            Utxos {
                fee: fee_utxo,
                settlement_asset: settlement_asset_utxo,
                grantor_settlement_token: grantor_settlement_token_utxo,
            },
            fee_amount,
            is_offline,
        )
        .await?;
        save_args_to_cache(&args_to_save)?;
        let reply_event_id = relay_processor
            .reply_order(maker_order_event_id, ReplyOption::MakerTerminationSettlement { tx_id })
            .await?;
        Ok(format!(
            "[Maker] Termination settlement tx result: {tx_id:?},  reply event id: {reply_event_id}"
        ))
    }

    #[allow(clippy::too_many_lines)]
    async fn _process_maker_settlement(
        CliAppContext {
            agg_config,
            relay_processor,
        }: &CliAppContext,
        MakerSettlementCliContext {
            grantor_collateral_token_utxo,
            grantor_settlement_token_utxo,
            fee_utxo,
            asset_utxo,
            fee_amount,
            price_at_current_block_height,
            oracle_signature,
            grantor_amount_to_burn,
            maker_order_event_id,
        }: MakerSettlementCliContext,
        CommonOrderOptions {
            account_index,
            is_offline,
        }: CommonOrderOptions,
    ) -> crate::error::Result<String> {
        use contract_handlers::maker_settlement::{Utxos, handle, process_args, save_args_to_cache};

        agg_config.check_nostr_keypair_existence()?;
        let processed_args = process_args(
            account_index,
            price_at_current_block_height,
            oracle_signature,
            grantor_amount_to_burn,
            maker_order_event_id,
            relay_processor,
        )
        .await?;
        let (tx_id, args_to_save) = handle(
            processed_args,
            Utxos {
                grantor_collateral_token: grantor_collateral_token_utxo,
                grantor_settlement_token: grantor_settlement_token_utxo,
                fee: fee_utxo,
                asset: asset_utxo,
            },
            fee_amount,
            is_offline,
        )
        .await?;
        save_args_to_cache(&args_to_save)?;
        let reply_event_id = relay_processor
            .reply_order(maker_order_event_id, ReplyOption::MakerSettlement { tx_id })
            .await?;
        Ok(format!(
            "[Maker] Final settlement tx result: {tx_id:?}, reply event id: {reply_event_id}"
        ))
    }

    #[allow(clippy::too_many_lines)]
    async fn process_taker_commands(
        CliAppContext {
            agg_config,
            relay_processor,
        }: &CliAppContext,
        action: TakerCommands,
    ) -> crate::error::Result<String> {
        Ok(match action {
            TakerCommands::FundOrder {
                filler_token_utxo,
                collateral_token_utxo,
                fee_amount,
                collateral_amount_to_deposit,
                common_options,
                maker_order_event_id,
            } => {
                use contract_handlers::taker_funding::{Utxos, handle, process_args, save_args_to_cache};

                agg_config.check_nostr_keypair_existence()?;
                let processed_args = process_args(
                    common_options.account_index,
                    collateral_amount_to_deposit,
                    maker_order_event_id,
                    relay_processor,
                )
                .await?;
                let (tx_id, args_to_save) = handle(
                    processed_args,
                    Utxos {
                        filler_token_utxo,
                        collateral_token_utxo,
                    },
                    fee_amount,
                    common_options.is_offline,
                )
                .await?;
                let reply_event_id = relay_processor
                    .reply_order(maker_order_event_id, ReplyOption::TakerFund { tx_id })
                    .await?;
                save_args_to_cache(&args_to_save)?;
                format!("[Taker] Tx fund sending result: {tx_id:?}, reply event id: {reply_event_id}")
            }
            TakerCommands::TerminationEarly {
                filler_token_utxo,
                collateral_token_utxo,
                fee_utxo,
                fee_amount,
                filler_token_amount_to_return,
                common_options,
                maker_order_event_id,
            } => {
                use contract_handlers::taker_early_termination::{Utxos, handle, process_args, save_args_to_cache};

                agg_config.check_nostr_keypair_existence()?;
                let processed_args = process_args(
                    common_options.account_index,
                    filler_token_amount_to_return,
                    maker_order_event_id,
                    relay_processor,
                )
                .await?;
                let (tx_id, args_to_save) = handle(
                    processed_args,
                    Utxos {
                        filler_token: filler_token_utxo,
                        collateral_token: collateral_token_utxo,
                        fee: fee_utxo,
                    },
                    fee_amount,
                    common_options.is_offline,
                )
                .await?;
                let reply_event_id = relay_processor
                    .reply_order(maker_order_event_id, ReplyOption::TakerTerminationEarly { tx_id })
                    .await?;
                save_args_to_cache(&args_to_save)?;
                format!("[Taker] Early termination tx result: {tx_id:?}, reply event id: {reply_event_id}")
            }
            TakerCommands::Settlement {
                filler_token_utxo,
                asset_utxo,
                fee_utxo,
                fee_amount,
                price_at_current_block_height,
                filler_amount_to_burn,
                oracle_signature,
                common_options,
                maker_order_event_id,
            } => {
                use contract_handlers::taker_settlement::{Utxos, handle, process_args, save_args_to_cache};

                agg_config.check_nostr_keypair_existence()?;
                let processed_args = process_args(
                    common_options.account_index,
                    price_at_current_block_height,
                    filler_amount_to_burn,
                    oracle_signature,
                    maker_order_event_id,
                    relay_processor,
                )
                .await?;
                let (tx_id, args_to_save) = handle(
                    processed_args,
                    Utxos {
                        filler_token: filler_token_utxo,
                        asset: asset_utxo,
                        fee: fee_utxo,
                    },
                    fee_amount,
                    common_options.is_offline,
                )
                .await?;
                save_args_to_cache(&args_to_save)?;
                let reply_event_id = relay_processor
                    .reply_order(maker_order_event_id, ReplyOption::TakerSettlement { tx_id })
                    .await?;
                format!("[Taker] Final settlement tx result: {tx_id:?}, reply event id: {reply_event_id}")
            }
        })
    }

    #[allow(clippy::too_many_lines)]
    async fn process_helper_commands(
        cli_app_context: &CliAppContext,
        action: HelperCommands,
    ) -> crate::error::Result<String> {
        Ok(match action {
            HelperCommands::Faucet {
                fee_utxo_outpoint,
                asset_name,
                issue_amount,
                fee_amount,
                common_options,
            } => {
                Self::_process_helper_faucet(fee_utxo_outpoint, asset_name, issue_amount, fee_amount, common_options)
                    .await?
            }
            HelperCommands::MintTokens {
                reissue_asset_outpoint,
                fee_utxo_outpoint,
                asset_name,
                reissue_amount,
                fee_amount,
                common_options,
            } => {
                Self::_process_helper_mint_tokens(
                    reissue_asset_outpoint,
                    fee_utxo_outpoint,
                    asset_name,
                    reissue_amount,
                    fee_amount,
                    common_options,
                )
                .await?
            }
            HelperCommands::SplitNativeThree {
                split_amount,
                fee_utxo,
                fee_amount,
                common_options,
            } => Self::_process_helper_split_native_three(split_amount, fee_utxo, fee_amount, common_options).await?,
            HelperCommands::Address { account_index: index } => Self::_process_helper_address(index)?,
            HelperCommands::OracleSignature {
                price_at_current_block_height,
                settlement_height,
                oracle_account_index,
            } => Self::_process_helper_oracle_signature(
                price_at_current_block_height,
                settlement_height,
                oracle_account_index,
            )?,
            HelperCommands::MergeTokens2 {
                token_utxo_1,
                token_utxo_2,
                fee_utxo,
                fee_amount,
                maker_order_event_id,
                common_options,
            } => {
                Self::_process_helper_merge_tokens2(
                    cli_app_context,
                    MergeTokens2CliContext {
                        token_utxo_1,
                        token_utxo_2,
                        fee_utxo,
                        fee_amount,
                        maker_order_event_id,
                    },
                    common_options,
                )
                .await?
            }
            HelperCommands::MergeTokens3 {
                token_utxo_1,
                token_utxo_2,
                token_utxo_3,
                fee_utxo,
                fee_amount,
                maker_order_event_id,
                common_options,
            } => {
                Self::_process_helper_merge_tokens3(
                    cli_app_context,
                    MergeTokens3CliContext {
                        token_utxo_1,
                        token_utxo_2,
                        token_utxo_3,
                        fee_utxo,
                        fee_amount,
                        maker_order_event_id,
                    },
                    common_options,
                )
                .await?
            }
            HelperCommands::MergeTokens4 {
                token_utxo_1,
                token_utxo_2,
                token_utxo_3,
                token_utxo_4,
                fee_utxo,
                fee_amount,
                maker_order_event_id,
                common_options,
            } => {
                Self::_process_helper_merge_tokens4(
                    cli_app_context,
                    MergeTokens4CliContext {
                        token_utxo_1,
                        token_utxo_2,
                        token_utxo_3,
                        token_utxo_4,
                        fee_utxo,
                        fee_amount,
                        maker_order_event_id,
                    },
                    common_options,
                )
                .await?
            }
        })
    }

    async fn _process_helper_faucet(
        fee_utxo_outpoint: OutPoint,
        asset_name: String,
        issue_amount: u64,
        fee_amount: u64,
        CommonOrderOptions {
            account_index,
            is_offline,
        }: CommonOrderOptions,
    ) -> crate::error::Result<String> {
        let tx_id = contract_handlers::faucet::create_asset(
            account_index,
            asset_name,
            fee_utxo_outpoint,
            fee_amount,
            issue_amount,
            is_offline,
        )
        .await?;
        Ok(format!("Finish asset creation, tx_id: {tx_id}"))
    }

    async fn _process_helper_mint_tokens(
        reissue_asset_outpoint: OutPoint,
        fee_utxo_outpoint: OutPoint,
        asset_name: String,
        reissue_amount: u64,
        fee_amount: u64,
        CommonOrderOptions {
            account_index,
            is_offline,
        }: CommonOrderOptions,
    ) -> crate::error::Result<String> {
        let tx_id = contract_handlers::faucet::mint_asset(
            account_index,
            asset_name,
            reissue_asset_outpoint,
            fee_utxo_outpoint,
            reissue_amount,
            fee_amount,
            is_offline,
        )
        .await?;
        Ok(format!("Finish asset minting, tx_id: {tx_id} "))
    }

    async fn _process_helper_split_native_three(
        split_amount: u64,
        fee_utxo: OutPoint,
        fee_amount: u64,
        CommonOrderOptions {
            account_index,
            is_offline,
        }: CommonOrderOptions,
    ) -> crate::error::Result<String> {
        let tx_res =
            contract_handlers::split_utxo::handle(account_index, split_amount, fee_utxo, fee_amount, is_offline)
                .await?;
        Ok(format!("Split utxo result tx_id: {tx_res:?}"))
    }

    fn _process_helper_address(index: u32) -> crate::error::Result<String> {
        let (x_only_pubkey, addr) = contract_handlers::address::handle(index)?;
        Ok(format!("X Only Public Key: '{x_only_pubkey}', P2PK Address: '{addr}'"))
    }

    fn _process_helper_oracle_signature(
        price_at_current_block_height: u64,
        settlement_height: u32,
        oracle_account_index: u32,
    ) -> crate::error::Result<String> {
        let (pubkey, msg, signature) = contract_handlers::oracle_signature::handle(
            oracle_account_index,
            price_at_current_block_height,
            settlement_height,
        )?;
        Ok(format!(
            "Oracle signature for msg: '{}', signature: '{}', pubkey used: '{}'",
            msg.to_hex(),
            hex::encode(signature.serialize()),
            pubkey.x_only_public_key().0.to_hex()
        ))
    }

    async fn _process_helper_merge_tokens2(
        CliAppContext {
            agg_config,
            relay_processor,
        }: &CliAppContext,
        MergeTokens2CliContext {
            token_utxo_1,
            token_utxo_2,
            fee_utxo,
            fee_amount,
            maker_order_event_id,
        }: MergeTokens2CliContext,
        CommonOrderOptions {
            account_index,
            is_offline,
        }: CommonOrderOptions,
    ) -> crate::error::Result<String> {
        use contract_handlers::merge_tokens::{
            merge2::{Utxos2, handle},
            process_args, save_args_to_cache,
        };

        agg_config.check_nostr_keypair_existence()?;
        let processed_args = process_args(account_index, maker_order_event_id, relay_processor).await?;
        let (tx_id, args_to_save) = handle(
            processed_args,
            Utxos2 {
                utxo_1: token_utxo_1,
                utxo_2: token_utxo_2,
                fee: fee_utxo,
            },
            fee_amount,
            is_offline,
        )
        .await?;
        save_args_to_cache(&args_to_save)?;
        let reply_event_id = relay_processor
            .reply_order(
                maker_order_event_id,
                ReplyOption::Merge2 {
                    tx_id,
                    token_utxo_1,
                    token_utxo_2,
                },
            )
            .await?;
        Ok(format!(
            "[Taker] Final merge 2 tx result: {tx_id:?}, reply event id: {reply_event_id}"
        ))
    }

    async fn _process_helper_merge_tokens3(
        CliAppContext {
            agg_config,
            relay_processor,
        }: &CliAppContext,
        MergeTokens3CliContext {
            token_utxo_1,
            token_utxo_2,
            token_utxo_3,
            fee_utxo,
            fee_amount,
            maker_order_event_id,
        }: MergeTokens3CliContext,
        CommonOrderOptions {
            account_index,
            is_offline,
        }: CommonOrderOptions,
    ) -> crate::error::Result<String> {
        use contract_handlers::merge_tokens::{
            merge3::{Utxos3, handle},
            process_args, save_args_to_cache,
        };

        agg_config.check_nostr_keypair_existence()?;
        let processed_args = process_args(account_index, maker_order_event_id, relay_processor).await?;
        let (tx_id, args_to_save) = handle(
            processed_args,
            Utxos3 {
                utxo_1: token_utxo_1,
                utxo_2: token_utxo_2,
                utxo_3: token_utxo_3,
                fee: fee_utxo,
            },
            fee_amount,
            is_offline,
        )
        .await?;
        save_args_to_cache(&args_to_save)?;
        let reply_event_id = relay_processor
            .reply_order(
                maker_order_event_id,
                ReplyOption::Merge3 {
                    tx_id,
                    token_utxo_1,
                    token_utxo_2,
                    token_utxo_3,
                },
            )
            .await?;
        Ok(format!(
            "[Taker] Final merge 3 tx result: {tx_id:?}, reply event id: {reply_event_id}"
        ))
    }

    async fn _process_helper_merge_tokens4(
        CliAppContext {
            agg_config,
            relay_processor,
        }: &CliAppContext,
        MergeTokens4CliContext {
            token_utxo_1,
            token_utxo_2,
            token_utxo_3,
            token_utxo_4,
            fee_utxo,
            fee_amount,
            maker_order_event_id,
        }: MergeTokens4CliContext,
        CommonOrderOptions {
            account_index,
            is_offline,
        }: CommonOrderOptions,
    ) -> crate::error::Result<String> {
        use contract_handlers::merge_tokens::{
            merge4::{Utxos4, handle},
            process_args, save_args_to_cache,
        };

        agg_config.check_nostr_keypair_existence()?;
        let processed_args = process_args(account_index, maker_order_event_id, relay_processor).await?;
        let (tx_id, args_to_save) = handle(
            processed_args,
            Utxos4 {
                utxo_1: token_utxo_1,
                utxo_2: token_utxo_2,
                utxo_3: token_utxo_3,
                utxo_4: token_utxo_4,
                fee: fee_utxo,
            },
            fee_amount,
            is_offline,
        )
        .await?;
        save_args_to_cache(&args_to_save)?;
        let reply_event_id = relay_processor
            .reply_order(
                maker_order_event_id,
                ReplyOption::Merge4 {
                    tx_id,
                    token_utxo_1,
                    token_utxo_2,
                    token_utxo_3,
                    token_utxo_4,
                },
            )
            .await?;
        Ok(format!(
            "[Taker] Final merge 4 tx result: {tx_id:?}, reply event id: {reply_event_id}"
        ))
    }

    async fn process_dex_commands(
        CliAppContext { relay_processor, .. }: &CliAppContext,
        action: DexCommands,
    ) -> crate::error::Result<String> {
        Ok(match action {
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
                crate::common::store::utils::save_dcd_args(&res.dcd_taproot_pubkey_gen, &res.dcd_arguments)?;
                format!("Order {event_id}: {res}")
            }
        })
    }
}

fn format_items<T, F>(items: &[T], map: F) -> String
where
    F: Fn(&T) -> String,
{
    items.iter().map(map).collect::<Vec<_>>().join("\n")
}
