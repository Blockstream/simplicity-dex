use crate::cli::processor::CommonOrderOptions;
use clap::Subcommand;
use nostr::EventId;
use simplicity::elements::OutPoint;

#[derive(Debug, Subcommand)]
pub enum TakerCommands {
    #[command(
        about = "Fund an existing DCD order as Taker and lock collateral into the contract [authentication required]",
        name = "fund"
    )]
    FundOrder {
        /// UTXO containing filler tokens provided by the Taker to fund the contract
        #[arg(long = "filler-utxo")]
        filler_token_utxo: OutPoint,
        /// UTXO containing collateral asset that the Taker locks into the DCD contract
        #[arg(long = "collateral-utxo")]
        collateral_token_utxo: OutPoint,
        /// Miner fee in satoshis (LBTC) for the Taker funding transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Amount of collateral (in satoshis) that the Taker will lock into the DCD contract
        #[arg(long = "collateral-amount-deposit")]
        collateral_amount_to_deposit: u64,
        /// `EventId` of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
    #[command(
        about = "Exit the DCD contract early as Taker by returning filler tokens in exchange for your collateral"
    )]
    TerminationEarly {
        /// UTXO containing filler tokens that the Taker returns to exit the contract early
        #[arg(long = "filler-utxo")]
        filler_token_utxo: OutPoint,
        /// UTXO containing the collateral asset that the Taker will withdraw back
        #[arg(long = "collateral-utxo")]
        collateral_token_utxo: OutPoint,
        /// UTXO used to pay miner fees for the early-termination transaction
        #[arg(long = "fee-utxo")]
        fee_utxo: OutPoint,
        /// Miner fee in satoshis (LBTC) for the early-termination transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// Amount of filler tokens (in satoshis) that the Taker returns to exit early
        #[arg(long = "filler-to-return")]
        filler_token_amount_to_return: u64,
        /// `EventId` of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
    #[command(
        about = "Settle the Taker side of the DCD at maturity using an oracle price to choose collateral or settlement asset"
    )]
    Settlement {
        /// UTXO containing filler tokens that the Taker burns during settlement
        #[arg(long = "filler-utxo")]
        filler_token_utxo: OutPoint,
        /// UTXO providing the asset (collateral or settlement) that the Taker receives at maturity
        #[arg(long = "asset-utxo")]
        asset_utxo: OutPoint,
        /// UTXO used to pay miner fees for the final Taker settlement transaction
        #[arg(long = "fee-utxo")]
        fee_utxo: OutPoint,
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
        /// `EventId` of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
}
