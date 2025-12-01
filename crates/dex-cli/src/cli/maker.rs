use crate::cli::CommonOrderOptions;
use crate::common::InitOrderArgs;
use clap::Subcommand;
use nostr::EventId;
use simplicity::elements::OutPoint;

#[derive(Debug, Subcommand)]
pub enum MakerCommands {
    #[command(
        about = "Mint three DCD token types and create an initial Maker offer for a Taker",
        long_about = "Mint three distinct DCD token types and initialize a Maker offer. \
        These tokens represent the Maker/Taker claims on collateral and settlement assets \
        and are used to manage the contract lifecycle (funding, early termination, settlement).",
        name = "init"
    )]
    InitOrder {
        /// LBTC UTXO used to fund issuance fees and the first DCD token
        #[arg(long = "utxo-1")]
        first_lbtc_utxo: OutPoint,
        /// LBTC UTXO used to fund issuance fees and the second DCD token
        #[arg(long = "utxo-2")]
        second_lbtc_utxo: OutPoint,
        /// LBTC UTXO used to fund issuance fees and the third DCD token
        #[arg(long = "utxo-3")]
        third_lbtc_utxo: OutPoint,
        #[command(flatten)]
        init_order_args: InitOrderArgs,
        /// Miner fee in satoshis (LBTC) for the init order transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
    #[command(
        about = "Fund a DCD offer by locking Maker tokens into the contract and publish the order on relays [authentication required]"
    )]
    Fund {
        /// UTXO containing Maker filler tokens to be locked into the DCD contract
        #[arg(long = "filler-utxo")]
        filler_token_utxo: OutPoint,
        /// UTXO containing Maker grantor collateral tokens to be locked or burned
        #[arg(long = "grant-coll-utxo")]
        grantor_collateral_token_utxo: OutPoint,
        /// UTXO containing Maker grantor settlement tokens to be locked or burned
        #[arg(long = "grant-settl-utxo")]
        grantor_settlement_token_utxo: OutPoint,
        /// UTXO providing the settlement asset (e.g. LBTC) for the DCD contract
        #[arg(long = "settl-asset-utxo")]
        settlement_asset_utxo: OutPoint,
        /// UTXO used to pay miner fees for the Maker funding transaction
        #[arg(long = "fee-utxo")]
        fee_utxo: OutPoint,
        /// Miner fee in satoshis (LBTC) for the Maker funding transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Taproot internal pubkey (hex) used to derive the contract output address
        #[arg(long = "taproot-pubkey-gen")]
        dcd_taproot_pubkey_gen: String,
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
    #[command(
        about = "Withdraw Maker collateral early by burning grantor collateral tokens (DCD early termination leg)"
    )]
    TerminationCollateral {
        /// UTXO containing grantor collateral tokens to be burned for early termination
        #[arg(long = "grantor-collateral-utxo")]
        grantor_collateral_token_utxo: OutPoint,
        /// UTXO containing the collateral asset (e.g. LBTC) to be withdrawn by the Maker
        #[arg(long = "collateral-utxo")]
        collateral_token_utxo: OutPoint,
        /// UTXO used to pay miner fees for the early-termination collateral transaction
        #[arg(long = "fee-utxo")]
        fee_utxo: OutPoint,
        /// Miner fee in satoshis (LBTC) for the early-termination collateral transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Amount of grantor collateral tokens (in satoshis) to burn for early termination
        #[arg(long = "grantor-collateral-burn")]
        grantor_collateral_amount_to_burn: u64,
        /// `EventId` of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
    #[command(
        about = "Withdraw Maker settlement asset early by burning grantor settlement tokens (DCD early termination leg)"
    )]
    TerminationSettlement {
        /// UTXO providing the settlement asset (e.g. LBTC) to be withdrawn by the Maker
        #[arg(long = "settlement-asset-utxo")]
        settlement_asset_utxo: OutPoint,
        /// UTXO containing grantor settlement tokens to be burned for early termination
        #[arg(long = "grantor-settlement-utxo")]
        grantor_settlement_token_utxo: OutPoint,
        /// UTXO used to pay miner fees for the early-termination settlement transaction
        #[arg(long = "fee-utxo")]
        fee_utxo: OutPoint,
        /// Miner fee in satoshis (LBTC) for the early-termination settlement transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Amount of grantor settlement tokens (in satoshis) to burn for early termination
        #[arg(long = "grantor-settlement-amount-burn")]
        grantor_settlement_amount_to_burn: u64,
        /// `EventId` of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
    #[command(
        about = "Settle the Maker side of the DCD at maturity using an oracle price to decide between collateral or settlement asset"
    )]
    Settlement {
        /// UTXO containing grantor collateral tokens used in final settlement
        #[arg(long = "grant-collateral-utxo")]
        grantor_collateral_token_utxo: OutPoint,
        /// UTXO containing grantor settlement tokens used in final settlement
        #[arg(long = "grant-settlement-utxo")]
        grantor_settlement_token_utxo: OutPoint,
        /// UTXO providing the asset (collateral or settlement) paid out to the Maker at maturity
        #[arg(long = "asset-utxo")]
        asset_utxo: OutPoint,
        /// UTXO used to pay miner fees for the final Maker settlement transaction
        #[arg(long = "fee-utxo")]
        fee_utxo: OutPoint,
        /// Miner fee in satoshis (LBTC) for the final settlement transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Amount of grantor (settlement and collateral) tokens (in satoshis) to burn during settlement step
        #[arg(long = "grantor-amount-burn")]
        grantor_amount_to_burn: u64,
        /// Oracle price at current block height used for settlement decision
        #[arg(long = "price-now")]
        price_at_current_block_height: u64,
        /// Schnorr signature produced by the oracle over the published price
        #[arg(long = "oracle-sign")]
        oracle_signature: String,
        /// `EventId` of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
}
