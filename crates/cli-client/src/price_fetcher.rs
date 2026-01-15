use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(thiserror::Error, Debug)]
pub enum PriceFetcherError {
    #[error("Request error: {0}")]
    Request(String),
    #[error("Response status error: {0}")]
    Status(String),
    #[error("Response parse error: {0}")]
    Parse(String),
}

pub type PriceMap = HashMap<String, HashMap<String, Decimal>>;

pub trait PriceFetcher {
    fn fetch_price(&self) -> Result<PriceMap, PriceFetcherError>;
}

#[derive(Deserialize)]
struct PriceResponse(PriceMap);

pub struct CoingeckoPriceFetcher;

impl CoingeckoPriceFetcher {
    const URL: &'static str = "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd&precision=8";
    const TIMEOUT_SECS: u64 = 5;

    pub fn new() -> Self {
        Self
    }
}

impl Default for CoingeckoPriceFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PriceFetcher for CoingeckoPriceFetcher {
    fn fetch_price(&self) -> Result<PriceMap, PriceFetcherError> {
        let resp = minreq::get(Self::URL)
            .with_header("User-Agent", "SimplicityDex")
            .with_timeout(Self::TIMEOUT_SECS)
            .send()
            .map_err(|e| PriceFetcherError::Request(e.to_string()))?;

        if resp.status_code != 200 {
            return Err(PriceFetcherError::Status(format!(
                "HTTP {}: {}",
                resp.status_code, resp.reason_phrase
            )));
        }

        let PriceResponse(json) = resp.json().map_err(|e| PriceFetcherError::Parse(e.to_string()))?;

        Ok(json)
    }
}
