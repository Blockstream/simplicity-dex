use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PriceFetcherError {
    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Rate limit exceeded (429)")]
    RateLimit,
    #[error("Response status error: {0}")]
    Status(u16),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Client build error: {0}")]
    Build(String),
}

#[derive(Deserialize)]
struct BitcoinResponse {
    bitcoin: BitcoinPrice,
}

#[derive(Deserialize)]
struct BitcoinPrice {
    usd: Decimal,
}

#[async_trait]
pub trait PriceFetcher {
    async fn fetch_price(&self) -> Result<Decimal, PriceFetcherError>;
}

pub struct CoingeckoPriceFetcher {
    client: reqwest::Client,
}

impl CoingeckoPriceFetcher {
    const URL: &'static str = "https://api.coingecko.com/api/v3";
    const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

    pub fn new() -> Result<Self, PriceFetcherError> {
        let client = reqwest::Client::builder()
            .user_agent("simplicity-dex/1.0")
            .timeout(Self::REQUEST_TIMEOUT)
            .build()
            .map_err(|e| PriceFetcherError::Build(e.to_string()))?;

        Ok(Self { client })
    }
}

#[async_trait]
impl PriceFetcher for CoingeckoPriceFetcher {
    async fn fetch_price(&self) -> Result<Decimal, PriceFetcherError> {
        let url = format!("{}/simple/price", Self::URL);

        let resp = self
            .client
            .get(&url)
            .query(&[("ids", "bitcoin"), ("vs_currencies", "usd"), ("precision", "8")])
            .send()
            .await?;

        match resp.status() {
            reqwest::StatusCode::OK => {
                let data: BitcoinResponse = resp.json().await.map_err(|e| PriceFetcherError::Parse(e.to_string()))?;
                Ok(data.bitcoin.usd)
            }
            reqwest::StatusCode::TOO_MANY_REQUESTS => Err(PriceFetcherError::RateLimit),
            status => Err(PriceFetcherError::Status(status.as_u16())),
        }
    }
}
