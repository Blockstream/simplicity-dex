use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::instrument;

use crate::utils::{default_key_path, default_relays_path, write_into_stdout};

#[derive(Parser)]
pub struct Cli {
    #[arg(short = 'k', long)]
    key_path: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Maker {
        #[command(subcommand)]
        action: MakerCommand,
    },

    Taker {
        #[command(subcommand)]
        action: TakerCommand,
    },
}

#[derive(Debug, Subcommand)]
enum MakerCommand {
    CreateOrder {
        #[arg(short = 'm', long)]
        message: String,

        #[arg(short = 'r', long)]
        relays_path: Option<PathBuf>,
    },

    GetOrderReply {
        #[arg(short = 'i', long)]
        id: String,

        #[arg(short = 'r', long)]
        relays_path: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum TakerCommand {
    ListOrders {
        #[arg(short = 'r', long)]
        relays_path: Option<PathBuf>,
    },

    ReplyOrder {
        #[arg(short = 'i', long)]
        id: String,
    },
}

impl Cli {
    #[instrument(skip(self))]
    pub fn process(self) -> crate::error::Result<()> {
        let msg = {
            match self.command {
                Command::Maker { action } => match action {
                    MakerCommand::CreateOrder { message, relays_path } => {
                        let key_path = self.key_path.unwrap_or(default_key_path());
                        let relays_path = relays_path.unwrap_or(default_relays_path());
                        format!(
                            "Maker: Create Order\n  message: {}\n  key_path: {}\n  relays_path: {}",
                            message,
                            key_path.display(),
                            relays_path.display()
                        )

                        // TODO:
                        //processor.create_order(message, key_path, relays_path).await?;
                    }

                    MakerCommand::GetOrderReply { id, relays_path } => {
                        let key_path = self.key_path.unwrap_or(default_key_path());
                        let relays_path = relays_path.unwrap_or(default_relays_path());
                        format!(
                            "Maker: Get Order Reply\n  id: {}\n  key_path: {}\n  relays_path: {}",
                            id,
                            key_path.display(),
                            relays_path.display()
                        )

                        // TODO:
                        //processor.get_order_reply(id, key_path, relays_path).await?;
                    }
                },

                Command::Taker { action } => match action {
                    TakerCommand::ListOrders { relays_path } => {
                        let key_path = self.key_path.unwrap_or(default_key_path());
                        let relays_path = relays_path.unwrap_or(default_relays_path());
                        format!(
                            "Taker: List Orders\n  key_path: {}\n  relays_path: {}",
                            key_path.to_string_lossy(),
                            relays_path.to_string_lossy()
                        )

                        // let key = ...
                        // let relays: Vec<RelayUrly> = ...
                        // TODO:
                        //processor.list_orders(key_path, relays_path).await?;
                    }

                    TakerCommand::ReplyOrder { id } => {
                        let key_path = self.key_path.unwrap_or(default_key_path());
                        format!("Taker: Reply Order\n  id: {}\n  key_path: {}", id, key_path.display())

                        // TODO
                        //processor.reply_order(id, key_path).await?;
                    }
                },
            }
        };
        write_into_stdout(msg)?;
        Ok(())
    }
}
