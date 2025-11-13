use clap::Subcommand;
use nostr::{EventId, PublicKey};

#[derive(Debug, Subcommand)]
pub enum TakerCommands {
    #[command(
        about = "Allows a Taker to exit the Dual Currency Deposit (DCD) contract before its expiry \
            by returning their filler tokens in exchange for their original collateral."
    )]
    TerminationEarly,
    #[command(about = "Allows the Taker to settle their position at the contract's maturity, \
        receiving either the collateral or the settlement asset based on an oracle-provided price")]
    Settlement,
    #[command(about = "Replies order as Taker on Relays specified [authentication required]")]
    ReplyOrder {
        #[arg(short = 'i', long)]
        maker_event_id: EventId,
        #[arg(short = 'p', long, help = " Pubkey in bech32 or hex format")]
        maker_pubkey: PublicKey,
        #[arg(short = 't', long, help = "Txid from funding transaction step", required = false)]
        tx_id: String,
    },
    #[command(about = "Funds order with settlement tokens [authentication required]")]
    FundOrder {
        #[arg(short = 'i', long)]
        maker_event_id: EventId,
        #[arg(short = 'p', long, help = " Pubkey in bech32 or hex format")]
        maker_pubkey: PublicKey,
        #[arg(short = 't', long, help = "Txid from funding transaction step", required = false)]
        tx_id: String,
    },
}
