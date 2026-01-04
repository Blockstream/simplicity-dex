use clap::Subcommand;
use simplicityhl::elements::{Address, AssetId, OutPoint};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Wallet management (init, address, balance, utxos, import, spend)
    Wallet {
        #[command(subcommand)]
        command: WalletCommand,
    },

    /// Basic transactions (transfer, split, merge, issue, reissue)
    Tx {
        #[command(subcommand)]
        command: TxCommand,
    },

    /// Options lifecycle (create, exercise, expire, cancel)
    Option {
        #[command(subcommand)]
        command: OptionCommand,
    },

    /// Swap lifecycle (create, take, cancel, withdraw)
    Swap {
        #[command(subcommand)]
        command: SwapCommand,
    },

    /// Fetch options/swaps from NOSTR, sync to coin-store, display
    Browse,

    /// Show my holdings with expiration warnings
    Positions,

    /// Sync coin-store with blockchain via Esplora and/or NOSTR
    Sync {
        #[command(subcommand)]
        command: SyncCommand,
    },

    /// Show current configuration
    Config,
}

/// Wallet management commands
#[derive(Debug, Subcommand)]
pub enum WalletCommand {
    /// Initialize the wallet database
    Init,

    /// Show wallet details
    Address,

    /// Show wallet balance
    Balance,

    /// List all UTXOs stored in wallet
    Utxos,

    /// Import a UTXO into the wallet
    Import {
        /// Outpoint (txid:vout)
        #[arg(long, short = 'o')]
        outpoint: OutPoint,

        /// Blinding key (hex, optional for confidential outputs)
        #[arg(long, short = 'b')]
        blinding_key: Option<String>,
    },

    /// Mark a specific output as spent
    Spend {
        /// Outpoint to mark as spent (txid:vout)
        #[arg(long, short = 'o')]
        outpoint: OutPoint,
    },
}

/// Basic transaction commands
#[derive(Debug, Subcommand)]
pub enum TxCommand {
    /// Transfer an asset to a recipient
    Transfer {
        /// Asset ID (defaults to native LBTC if not specified)
        #[arg(long)]
        asset_id: Option<AssetId>,
        /// Recipient address
        #[arg(long)]
        to: Address,
        /// Amount to send
        #[arg(long)]
        amount: u64,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Split LBTC into multiple UTXOs
    SplitNative {
        /// Number of parts to split into
        #[arg(long)]
        count: u64,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Merge multiple UTXOs of the same asset into one
    Merge {
        /// Asset ID to merge (defaults to native LBTC if not specified)
        #[arg(long)]
        asset_id: Option<AssetId>,
        /// Number of UTXOs to merge
        #[arg(long)]
        count: usize,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Issue a new asset
    IssueAsset {
        /// Amount to issue
        #[arg(long)]
        amount: u64,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Reissue an existing asset using reissuance token
    ReissueAsset {
        /// Asset ID to reissue
        #[arg(long)]
        asset_id: AssetId,
        /// Amount to reissue
        #[arg(long)]
        amount: u64,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },
}

/// Options lifecycle commands
#[derive(Debug, Subcommand)]
pub enum OptionCommand {
    /// Create a new options contract (fund collateral, get Option + Grantor tokens)
    Create {
        /// Collateral asset ID
        #[arg(long)]
        collateral_asset: AssetId,
        /// Total collateral to lock in the contract
        #[arg(long)]
        total_collateral: u64,
        /// Number of option contracts (tokens) to issue
        #[arg(long)]
        num_contracts: u64,
        /// Settlement asset ID
        #[arg(long)]
        settlement_asset: AssetId,
        /// Total strike price (settlement needed to exercise ALL contracts)
        #[arg(long)]
        total_strike: u64,
        /// Expiry time as Unix timestamp or duration (e.g., +30d)
        #[arg(long)]
        expiry: String,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Exercise an option before expiration (deposit settlement, get collateral, burn option)
    Exercise {
        /// Option token outpoint (interactive selection if not provided)
        #[arg(long)]
        option_token: Option<OutPoint>,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Expire an option after expiration (use Grantor Token to get collateral)
    Expire {
        /// Grantor token outpoint (interactive selection if not provided)
        #[arg(long)]
        grantor_token: Option<OutPoint>,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Claim settlement after options were exercised (use Grantor Token to get settlement asset)
    Settlement {
        /// Grantor token outpoint (interactive selection if not provided)
        #[arg(long)]
        grantor_token: Option<OutPoint>,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Cancel an option (requires both Option + Grantor tokens)
    Cancel {
        /// Option token outpoint (interactive selection if not provided)
        #[arg(long)]
        option_token: Option<OutPoint>,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },
}

/// Swap lifecycle commands
#[derive(Debug, Subcommand)]
pub enum SwapCommand {
    /// Create a swap offer (offer Grantor Token for premium)
    Create {
        /// Grantor token outpoint (interactive selection if not provided)
        #[arg(long)]
        grantor_token: Option<OutPoint>,
        /// Premium asset ID (defaults to native LBTC)
        #[arg(long)]
        premium_asset: Option<AssetId>,
        /// Premium amount
        #[arg(long)]
        premium_amount: u64,
        /// Expiry time (defaults to same as option)
        #[arg(long)]
        expiry: Option<String>,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction and publish to NOSTR
        #[arg(long)]
        broadcast: bool,
    },

    /// Take a swap offer (atomic swap: premium for Grantor Token)
    Take {
        /// Swap event ID from NOSTR (interactive selection if not provided)
        #[arg(long)]
        swap_event: Option<String>,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Cancel a swap offer after expiry (reclaim collateral if no one took it)
    Cancel {
        /// Swap event ID from NOSTR (interactive selection if not provided)
        #[arg(long)]
        swap_event: Option<String>,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Withdraw settlement after swap was taken (claim your payment)
    Withdraw {
        /// Swap event ID from NOSTR (interactive selection if not provided)
        #[arg(long)]
        swap_event: Option<String>,
        /// Fee amount in satoshis (auto-estimated if not specified)
        #[arg(long)]
        fee: Option<u64>,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },
}

/// Sync commands for reconciling coin-store with blockchain
#[derive(Debug, Subcommand)]
pub enum SyncCommand {
    /// Full sync: mark spent UTXOs + discover new UTXOs + sync NOSTR events
    Full,

    /// Only check and mark spent UTXOs as spent via Esplora
    Spent,

    /// Only discover new UTXOs for wallet address and tracked contracts via Esplora
    Utxos,

    /// Only sync options and swaps from NOSTR relay
    Nostr,
}
