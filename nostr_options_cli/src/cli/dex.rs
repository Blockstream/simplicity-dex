use clap::Subcommand;
use nostr::EventId;

#[derive(Debug, Subcommand)]
pub enum DexCommands {
    #[command(about = "Get replies for a specific order by its ID [no authentication required]")]
    GetOrderReplies {
        #[arg(short = 'i', long)]
        event_id: EventId,
    },
    #[command(about = "List available orders from relays [no authentication required]")]
    ListOrders,
    #[command(about = "Get events by its ID [no authentication required]")]
    GetEventsById {
        #[arg(short = 'i', long)]
        event_id: EventId,
    },
    #[command(about = "Get order by its ID [no authentication required]")]
    GetOrderById {
        #[arg(short = 'i', long)]
        event_id: EventId,
    },
}
