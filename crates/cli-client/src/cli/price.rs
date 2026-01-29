use crate::error::Error;
use crate::price_fetcher::{CoingeckoPriceFetcher, PriceFetcher};
use rust_decimal::Decimal;

pub async fn fetch_btc_usd_price<T: PriceFetcher>(fetcher: &T) -> Result<Decimal, Error> {
    Ok(fetcher.fetch_price().await?)
}

pub async fn run_price_feed() -> Result<(), Error> {
    let fetcher = CoingeckoPriceFetcher::new()?;
    let price = fetch_btc_usd_price(&fetcher).await?;

    println!("1 BTC = ${}", price.round_dp(2));

    Ok(())
}
