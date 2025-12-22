use clap::Subcommand;
use nostr::{EventId, Timestamp};

#[derive(Debug, Subcommand)]
pub enum DexCommands {
    #[command(about = "Fetch replies for a specific order event from Nostr relays [no authentication required]")]
    GetOrderReplies {
        #[arg(short = 'i', long)]
        event_id: EventId,
    },
    #[command(about = "List all currently available orders discovered on Nostr relays [no authentication required]")]
    ListOrders {
        /// Comma-separated list of author public keys to filter by (hex or npub)
        #[arg(long = "authors", value_delimiter = ',')]
        authors: Option<Vec<nostr::PublicKey>>,
        #[command(subcommand)]
        time_to_filter: Option<TimeOptionArgs>,
        /// Maximum number of orders to return
        #[arg(long = "limit")]
        limit: Option<usize>,
    },
    #[command(about = "Import order parameters from a Maker order Nostr event [no authentication required]")]
    ImportParams {
        #[arg(short = 'i', long)]
        event_id: EventId,
    },
    #[command(about = "Fetch an arbitrary Nostr event by its ID [no authentication required]")]
    GetEventsById {
        #[arg(short = 'i', long)]
        event_id: EventId,
    },
    #[command(about = "Fetch a single order by its event ID from Nostr relays [no authentication required]")]
    GetOrderById {
        #[arg(short = 'i', long)]
        event_id: EventId,
    },
}

#[derive(Debug, Subcommand)]
pub enum TimeOptionArgs {
    /// Filter events from the last duration (e.g., "1h", "30m", "7d")
    #[command(name = "duration")]
    Duration {
        #[arg(value_name = "DURATION")]
        value: humantime::Duration,
    },
    /// Filter events by timestamp range
    #[command(name = "timestamp")]
    Timestamp {
        /// Filter events since this Unix timestamp
        #[arg(long = "since")]
        since: Option<u64>,
        /// Filter events until this Unix timestamp
        #[arg(long = "until")]
        until: Option<u64>,
    },
}

impl TimeOptionArgs {
    #[must_use]
    pub fn compute_since(&self) -> Option<u64> {
        match self {
            TimeOptionArgs::Duration { value } => {
                let now = Timestamp::now().as_u64();
                Some(now.saturating_sub(value.as_secs()))
            }
            TimeOptionArgs::Timestamp { since, .. } => *since,
        }
    }

    #[must_use]
    pub fn compute_until(&self) -> Option<u64> {
        match self {
            TimeOptionArgs::Duration { .. } => None,
            TimeOptionArgs::Timestamp { until, .. } => *until,
        }
    }
}
