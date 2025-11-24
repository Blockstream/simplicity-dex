use crate::common::{DCDCliArguments, DCDCliMakerFundArguments, InitOrderArgs};
use clap::Subcommand;
use nostr::EventId;
use simplicity::elements::OutPoint;

#[derive(Debug, Subcommand)]
pub enum MakerCommands {
    #[command(
        about = "Mint three DCD token types and create an initial Maker offer for a Taker",
        long_about = "Mint three distinct DCD token types and initialize a Maker offer. \
        These tokens represent the Maker/Taker claims on collateral and settlement assets \
        and are used to manage the contract lifecycle (funding, early termination, settlement)."
    )]
    InitOrder {
        /// UTXOs that will fund fees and asset issuance for the DCD tokens
        #[arg(long = "fee-utxos", value_delimiter = ',')]
        fee_utxos: Vec<OutPoint>,
        #[command(flatten)]
        init_order_args: InitOrderArgs,
        /// Miner fee in satoshis (LBTC) for the init order transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Account index used to derive internal/change addresses from the wallet
        #[arg(long = "account-index", default_value_t = 0)]
        account_index: u32,
        /// When true, broadcast the built transaction via Esplora; otherwise only print it
        #[arg(long = "broadcast", default_value_t = true)]
        broadcast: bool,
    },
    #[command(
        about = "Fund a DCD offer by locking Maker tokens into the contract and publish the order on relays [authentication required]",
        alias = "fund"
    )]
    Fund {
        /// UTXOs providing the DCD tokens, settlement asset, and fees (exactly 5 expected)
        #[arg(long = "fee-utxos", value_delimiter = ',')]
        fee_utxos: Vec<OutPoint>,
        /// Miner fee in satoshis (LBTC) for the Maker funding transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Taproot internal pubkey (hex) used to derive the contract output address
        #[arg(long = "taproot-pubkey-gen")]
        dcd_taproot_pubkey_gen: String,
        #[command(flatten)]
        dcd_arguments: Option<DCDCliMakerFundArguments>,
        /// Account index used to derive internal/change addresses from the wallet
        #[arg(long = "account-index", default_value_t = 0)]
        account_index: u32,
        /// When true, broadcast the built transaction via Esplora; otherwise only print it
        #[arg(long = "broadcast", default_value_t = true)]
        broadcast: bool,
    },
    #[command(
        about = "Withdraw Maker collateral early by burning grantor collateral tokens (DCD early termination leg)"
    )]
    TerminationCollateral {
        /// UTXOs providing grantor collateral tokens, settlement asset, and fees (exactly 5 expected)
        #[arg(long = "fee-utxos", value_delimiter = ',')]
        fee_utxos: Vec<OutPoint>,
        /// Miner fee in satoshis (LBTC) for the early-termination collateral transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Taproot internal pubkey (hex) used to derive the contract output address
        #[arg(long = "taproot-pubkey-gen")]
        dcd_taproot_pubkey_gen: String,
        /// Amount of grantor collateral tokens (in satoshis) to burn for early termination
        #[arg(long = "grantor-coll-burn")]
        grantor_collateral_amount_to_burn: u64,
        #[command(flatten)]
        dcd_arguments: Option<DCDCliArguments>,
        /// Account index used to derive internal/change addresses from the wallet
        #[arg(long = "account-index", default_value_t = 0)]
        account_index: u32,
        /// When true, broadcast the built transaction via Esplora; otherwise only print it
        #[arg(long = "broadcast", default_value_t = true)]
        broadcast: bool,
        /// `EventId` of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
    },
    #[command(
        about = "Withdraw Maker settlement asset early by burning grantor settlement tokens (DCD early termination leg)"
    )]
    TerminationSettlement {
        /// UTXOs providing grantor settlement tokens, settlement asset, and fees (exactly 5 expected)
        #[arg(long = "fee-utxos", value_delimiter = ',')]
        fee_utxos: Vec<OutPoint>,
        /// Miner fee in satoshis (LBTC) for the early-termination settlement transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Amount of grantor settlement tokens (in satoshis) to burn for early termination
        #[arg(long = "grantor-settl-burn")]
        grantor_settlement_amount_to_burn: u64,
        /// Taproot internal pubkey (hex) used to derive the contract output address
        #[arg(long = "taproot-pubkey-gen")]
        dcd_taproot_pubkey_gen: String,
        #[command(flatten)]
        dcd_arguments: Option<DCDCliArguments>,
        /// Account index used to derive internal/change addresses from the wallet
        #[arg(long = "account-index", default_value_t = 0)]
        account_index: u32,
        /// When true, broadcast the built transaction via Esplora; otherwise only print it
        #[arg(long = "broadcast", default_value_t = true)]
        broadcast: bool,
        /// `EventId` of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
    },
    #[command(
        about = "Settle the Maker side of the DCD at maturity using an oracle price to decide between collateral or settlement asset"
    )]
    Settlement {
        /// UTXOs providing grantor tokens, settlement asset, and fees (exactly 5 expected)
        #[arg(long = "fee-utxos", value_delimiter = ',')]
        fee_utxos: Vec<OutPoint>,
        /// Miner fee in satoshis (LBTC) for the final settlement transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Oracle price at current block height used for settlement decision
        #[arg(long = "grantor-settl-burn")]
        price_at_current_block_height: u64,
        /// Schnorr signature produced by the oracle over the published price
        #[arg(long = "oracle-sign")]
        oracle_signature: String,
        /// Amount of grantor tokens (in satoshis) to burn during settlement
        #[arg(long = "grantor-amount-burn")]
        grantor_amount_to_burn: u64,
        /// Taproot internal pubkey (hex) used to derive the contract output address
        #[arg(long = "taproot-pubkey-gen")]
        dcd_taproot_pubkey_gen: String,
        #[command(flatten)]
        dcd_arguments: Option<DCDCliArguments>,
        /// Account index used to derive internal/change addresses from the wallet
        #[arg(long = "account-index", default_value_t = 0)]
        account_index: u32,
        /// When true, broadcast the built transaction via Esplora; otherwise only print it
        #[arg(long = "broadcast", default_value_t = true)]
        broadcast: bool,
        /// `EventId` of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
    },
}
