use clap::Subcommand;
use simplicityhl::elements::OutPoint;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Maker commands for creating and managing options
    Maker {
        #[command(subcommand)]
        command: MakerCommand,
    },

    /// Taker commands for participating in options
    Taker {
        #[command(subcommand)]
        command: TakerCommand,
    },

    /// Helper utilities
    Helper {
        #[command(subcommand)]
        command: HelperCommand,
    },

    /// Show current configuration
    Config,

    /// Basic transaction commands (transfer, split, issue)
    Basic {
        #[command(subcommand)]
        command: BasicCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum MakerCommand {
    /// Create a new options contract
    Create,

    /// Fund an options contract with collateral
    Fund,

    /// Exercise an option before expiration
    Exercise,

    /// Cancel an unfilled option and retrieve collateral
    Cancel,

    /// List your created options
    List,
}

#[derive(Debug, Subcommand)]
pub enum TakerCommand {
    /// Browse available options
    Browse,

    /// Take an option by purchasing grantor token
    Take,

    /// Claim settlement after expiration
    Claim,

    /// List your positions
    List,
}

#[derive(Debug, Subcommand)]
pub enum HelperCommand {
    /// Show wallet details
    Address,

    /// Initialize the wallet database
    Init,

    /// Show wallet balance
    Balance,

    /// List UTXOs
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
}

#[derive(Debug, Subcommand)]
pub enum BasicCommand {
    /// Transfer LBTC to a recipient
    TransferNative {
        /// Recipient address
        #[arg(long)]
        to: String,
        /// Amount to send in satoshis
        #[arg(long)]
        amount: u64,
        /// Fee amount in satoshis
        #[arg(long)]
        fee: u64,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Split LBTC into multiple UTXOs
    SplitNative {
        /// Number of parts to split into
        #[arg(long)]
        parts: u64,
        /// Fee amount in satoshis
        #[arg(long)]
        fee: u64,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Transfer an asset to a recipient
    TransferAsset {
        /// Asset ID (hex)
        #[arg(long)]
        asset: String,
        /// Recipient address
        #[arg(long)]
        to: String,
        /// Amount to send
        #[arg(long)]
        amount: u64,
        /// Fee amount in satoshis
        #[arg(long)]
        fee: u64,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Issue a new asset
    IssueAsset {
        /// Asset name (local reference)
        #[arg(long)]
        name: String,
        /// Amount to issue
        #[arg(long)]
        amount: u64,
        /// Fee amount in satoshis
        #[arg(long)]
        fee: u64,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },

    /// Reissue an existing asset
    ReissueAsset {
        /// Asset name (local reference)
        #[arg(long)]
        name: String,
        /// Amount to reissue
        #[arg(long)]
        amount: u64,
        /// Fee amount in satoshis
        #[arg(long)]
        fee: u64,
        /// Broadcast transaction
        #[arg(long)]
        broadcast: bool,
    },
}
