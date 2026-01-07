use nostr::Kind;

pub const OPTION_CREATED: Kind = Kind::Custom(9910);
pub const SWAP_CREATED: Kind = Kind::Custom(9911);
pub const ACTION_COMPLETED: Kind = Kind::Custom(9912);

pub const TAG_OPTIONS_ARGS: &str = "options_args";
pub const TAG_OPTIONS_UTXO: &str = "options_utxo";
pub const TAG_SWAP_ARGS: &str = "swap_args";
pub const TAG_SWAP_UTXO: &str = "swap_utxo";
pub const TAG_TAPROOT_GEN: &str = "t";
pub const TAG_ACTION: &str = "action";
pub const TAG_OUTPOINT: &str = "outpoint";
pub const TAG_EXPIRY: &str = "expiry";

pub const ACTION_SWAP_EXERCISED: &str = "swap_exercised";
pub const ACTION_SWAP_CANCELLED: &str = "swap_cancelled";
pub const ACTION_OPTION_EXERCISED: &str = "option_exercised";
pub const ACTION_OPTION_CANCELLED: &str = "option_cancelled";
pub const ACTION_SETTLEMENT_CLAIMED: &str = "settlement_claimed";
pub const ACTION_OPTION_EXPIRED: &str = "option_expired";
