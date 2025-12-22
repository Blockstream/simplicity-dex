use crate::cli::CommonOrderOptions;
use clap::Subcommand;
use nostr::EventId;
use simplicity::elements::OutPoint;

#[derive(Debug, Subcommand)]
pub enum HelperCommands {
    #[command(about = "Display a test P2PK address for the given account index [testing only]")]
    Address {
        /// Account index to use for change address
        #[arg(long = "account-index", default_value_t = 0)]
        account_index: u32,
    },
    #[command(about = "Issue new test tokens backed by LBTC for settlement testing [testing only]")]
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
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
    #[command(about = "Reissue additional units of an already created test asset [testing only]")]
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
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
    #[command(about = "Split a single LBTC UTXO into three outputs of equal value [testing only]")]
    SplitNativeThree {
        #[arg(long = "split-amount")]
        split_amount: u64,
        /// Fee utxo
        #[arg(long = "fee-utxo")]
        fee_utxo: OutPoint,
        #[arg(long = "fee-amount", default_value_t = 150)]
        fee_amount: u64,
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
    #[command(about = "Sign oracle message with keypair [testing only]")]
    OracleSignature {
        /// Price at current block height
        #[arg(long = "price-at-current-block-height")]
        price_at_current_block_height: u64,
        /// Settlement height
        #[arg(long = "settlement-height")]
        settlement_height: u32,
        /// Oracle account index to derive key from `SEED_HEX`
        #[arg(long = "oracle-account-index")]
        oracle_account_index: u32,
    },
    #[command(about = "Merge 2 token UTXOs into 1")]
    MergeTokens2 {
        /// First token UTXO
        #[arg(long = "token-utxo-1")]
        token_utxo_1: OutPoint,
        /// Second token UTXO
        #[arg(long = "token-utxo-2")]
        token_utxo_2: OutPoint,
        /// Fee UTXO
        #[arg(long = "fee-utxo")]
        fee_utxo: OutPoint,
        /// Miner fee in satoshis (LBTC) for the final settlement transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// `EventId` of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
    #[command(about = "Merge 3 token UTXOs into 1")]
    MergeTokens3 {
        /// First token UTXO
        #[arg(long = "token-utxo-1")]
        token_utxo_1: OutPoint,
        /// Second token UTXO
        #[arg(long = "token-utxo-2")]
        token_utxo_2: OutPoint,
        /// Third token UTXO
        #[arg(long = "token-utxo-3")]
        token_utxo_3: OutPoint,
        /// Fee UTXO
        #[arg(long = "fee-utxo")]
        fee_utxo: OutPoint,
        /// Miner fee in satoshis (LBTC) for the final settlement transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// `EventId` of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
    #[command(about = "Merge 4 token UTXOs into 1")]
    MergeTokens4 {
        /// First token UTXO
        #[arg(long = "token-utxo-1")]
        token_utxo_1: OutPoint,
        /// Second token UTXO
        #[arg(long = "token-utxo-2")]
        token_utxo_2: OutPoint,
        /// Third token UTXO
        #[arg(long = "token-utxo-3")]
        token_utxo_3: OutPoint,
        /// Fourth token UTXO
        #[arg(long = "token-utxo-4")]
        token_utxo_4: OutPoint,
        /// Fee UTXO
        #[arg(long = "fee-utxo")]
        fee_utxo: OutPoint,
        /// Miner fee in satoshis (LBTC) for the final settlement transaction
        #[arg(long = "fee-amount", default_value_t = 1500)]
        fee_amount: u64,
        /// `EventId` of the Maker\'s original order event on Nostr
        #[arg(short = 'i', long)]
        maker_order_event_id: EventId,
        #[command(flatten)]
        common_options: CommonOrderOptions,
    },
}
