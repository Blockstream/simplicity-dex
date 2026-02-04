use serde::Deserialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PriceFetcherError {
    #[error("Request error: {0}")]
    Request(#[from] minreq::Error),
    #[error("Rate limit exceeded (429)")]
    RateLimit,
    #[error("Response status error: {0}")]
    Status(i32),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Internal runtime error: {0}")]
    Internal(String),
}

#[derive(Deserialize)]
struct BitcoinResponse {
    bitcoin: BitcoinPrice,
}

#[derive(Deserialize)]
struct BitcoinPrice {
    usd: f64,
}

pub trait PriceFetcher {
    fn fetch_price(&self) -> Result<f64, PriceFetcherError>;
}

#[derive(Default)]
pub struct CoingeckoPriceFetcher;

impl CoingeckoPriceFetcher {
    const URL: &'static str = "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd&precision=8";
    const TIMEOUT_SECS: u64 = 5;
}

impl PriceFetcher for CoingeckoPriceFetcher {
    fn fetch_price(&self) -> Result<f64, PriceFetcherError> {
        let resp = minreq::get(Self::URL)
            .with_header("User-Agent", "simplicity-dex/1.0")
            .with_timeout(Self::TIMEOUT_SECS)
            .send()
            .map_err(PriceFetcherError::from)?;

        match resp.status_code {
            200 => resp
                .json::<BitcoinResponse>()
                .map(|data| data.bitcoin.usd)
                .map_err(|e| PriceFetcherError::Parse(e.to_string())),
            429 => Err(PriceFetcherError::RateLimit),
            status => Err(PriceFetcherError::Status(status)),
        }
    }
}

pub fn fetch_btc_usd_price<T: PriceFetcher>(fetcher: &T) -> Result<f64, PriceFetcherError> {
    fetcher.fetch_price()
}
