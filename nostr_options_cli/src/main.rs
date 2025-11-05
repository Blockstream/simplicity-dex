use global_utils::logger::init_logger;
use nostr::prelude::*;
use clap::{Parser, Subcommand};
use std::path::PathBuf;


#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Maker
    CreateOrder {
        #[arg(short = 'm', long)]
        message: String,

        #[arg(short = 'k', long)]
        key_path: Option<PathBuf>,

        #[arg(short = 'r', long)]
        relays_path: Option<PathBuf>,
    },

    GetOrderReply {
        #[arg(short = 'i', long)]
        id: String,

        #[arg(short = 'k', long)]
        key_path: Option<PathBuf>,

        #[arg(short = 'r', long)]
        relays_path: Option<PathBuf>,
    },

    /// Taker
    ListOrders {
        #[arg(short = 'k', long)]
        key_path: Option<PathBuf>,

        #[arg(short = 'r', long)]
        relays_path: Option<PathBuf>,
    },

    ReplyOrder {
        #[arg(short = 'i', long)]
        id: String,

        #[arg(short = 'k', long)]
        key_path: Option<PathBuf>,
    },
}


#[tokio::main]
async fn main() -> Result<()> {
    let _logger_guard = init_logger();

    let cli = Cli::parse();

    match cli.command {
        Command::CreateOrder { message, key_path, relays_path } => {
            let key_path = key_path.unwrap_or(default_key_path());
            let relays_path = relays_path.unwrap_or(default_relays_path());
            println!("🛠 Create order:");
            println!("  message: {}", message);
            println!("  key_path: {}", key_path.display());
            println!("  relays_path: {}", relays_path.display());
        }

        Command::GetOrderReply { id, key_path, relays_path } => {
            let key_path = key_path.unwrap_or(default_key_path());
            let relays_path = relays_path.unwrap_or(default_relays_path());
            println!("📦 Get order reply:");
            println!("  id: {}", id);
            println!("  key_path: {}", key_path.display());
            println!("  relays_path: {}", relays_path.display());
        }

        Command::ListOrders { key_path, relays_path } => {
            let key_path = key_path.unwrap_or(default_key_path());
            let relays_path = relays_path.unwrap_or(default_relays_path());
            println!("📋 List orders:");
            println!("  key_path: {}", key_path.display());
            println!("  relays_path: {}", relays_path.display());
        }

        Command::ReplyOrder { id, key_path } => {
            let key_path = key_path.unwrap_or(default_key_path());
            println!("💬 Reply order:");
            println!("  id: {}", id);
            println!("  key_path: {}", key_path.display());
        }
    }

    Ok(())
}


fn default_key_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".nostr/keypair.txt")
}

fn default_relays_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".nostr/relays.txt")
}