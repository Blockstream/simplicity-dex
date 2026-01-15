use crate::error::Error;
use crate::price_fetcher::{CoingeckoPriceFetcher, PriceFetcher};
pub async fn run_price_feed() -> Result<(), Error> {
    let fetcher = CoingeckoPriceFetcher::new();
    let prices = tokio::task::spawn_blocking(move || fetcher.fetch_price())
        .await
        .map_err(|e| Error::Config(format!("Price task error: {e}")))?
        .map_err(|e| Error::Config(format!("Price fetch error: {e}")))?;

    let price = prices
        .get("bitcoin")
        .and_then(|currencies| currencies.get("usd"))
        .ok_or_else(|| Error::Config("Price response missing bitcoin/usd".to_string()))?;

    println!("1 BTC = ${price:.2}");

    Ok(())
}
