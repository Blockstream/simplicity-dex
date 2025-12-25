use crate::error::Error;
use simplicityhl::elements::Transaction;
use simplicityhl::elements::pset::serialize::Serialize;
use simplicityhl::simplicity::hex::DisplayHex;

#[derive(Debug, Clone, Copy)]
pub enum Broadcaster {
    Offline,
    Online,
}

impl From<bool> for Broadcaster {
    fn from(b: bool) -> Self {
        if b { Broadcaster::Online } else { Broadcaster::Offline }
    }
}

impl Broadcaster {
    pub async fn broadcast_tx(&self, tx: &Transaction) -> Result<(), Error> {
        match self {
            Broadcaster::Offline => {
                println!("{}", tx.serialize().to_lower_hex_string());
            }
            Broadcaster::Online => {
                cli_helper::explorer::broadcast_tx(tx).await?;
                let txid = tx.txid();
                println!("Broadcasted: {txid}");
            }
        }
        Ok(())
    }
}
