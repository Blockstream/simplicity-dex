use crate::common::DCDCliArguments;
use clap::Subcommand;
use nostr::EventId;
use simplicity::elements::OutPoint;

#[derive(Debug, Subcommand)]
pub enum TakerCommands {
    #[command(
        about = "Fund an existing DCD order as Taker and lock collateral into the contract [authentication required]"
    )]
    FundOrder {
        /// UTXOs providing filler tokens, collateral, settlement asset, and fees (exactly 5 expected)
        #[arg(long = "fee-utxos", value_delimiter = ',')]
        fee_utxos: Vec<OutPoint>,
        /// Miner fee in satoshis (LBTC) for the Taker funding transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Amount of collateral (in satoshis) that the Taker will lock into the DCD contract
        #[arg(long = "coll-amount-deposit")]
        collateral_amount_to_deposit: u64,
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
        /// EventId of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
    },
    #[command(
        about = "Exit the DCD contract early as Taker by returning filler tokens in exchange for your collateral"
    )]
    TerminationEarly {
        /// UTXOs providing filler tokens, collateral, settlement asset, and fees (exactly 5 expected)
        #[arg(long = "fee-utxos", value_delimiter = ',')]
        fee_utxos: Vec<OutPoint>,
        /// Miner fee in satoshis (LBTC) for the early-termination transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Amount of filler tokens (in satoshis) that the Taker returns to exit early
        #[arg(long = "filler-to-return")]
        filler_token_amount_to_return: u64,
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
        /// EventId of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
    },
    #[command(
        about = "Settle the Taker side of the DCD at maturity using an oracle price to choose collateral or settlement asset"
    )]
    Settlement {
        /// UTXOs providing filler tokens, collateral, settlement asset, and fees (exactly 5 expected)
        #[arg(long = "fee-utxos", value_delimiter = ',')]
        fee_utxos: Vec<OutPoint>,
        /// Miner fee in satoshis (LBTC) for the final Taker settlement transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Amount of filler tokens (in satoshis) that the Taker burns during settlement
        #[arg(long = "filler-to-burn")]
        filler_amount_to_burn: u64,
        /// Oracle price at current block height used for settlement decision
        #[arg(long = "price-now")]
        price_at_current_block_height: u64,
        /// Schnorr/ecdsa signature produced by the oracle over the published price
        #[arg(long = "oracle-sign")]
        oracle_signature: String,
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
        /// EventId of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
    },
}
