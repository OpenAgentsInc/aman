//! Cryptocurrency price tool using CoinGecko API.

use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::debug;

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// Response from CoinGecko simple/price endpoint.
#[derive(Debug, Deserialize)]
struct CoinGeckoPrice {
    #[serde(flatten)]
    prices: HashMap<String, PriceData>,
}

#[derive(Debug, Deserialize)]
struct PriceData {
    usd: Option<f64>,
    eur: Option<f64>,
    gbp: Option<f64>,
    usd_24h_change: Option<f64>,
    usd_market_cap: Option<f64>,
}

/// Cryptocurrency price tool using CoinGecko API.
///
/// Fetches current prices for any cryptocurrency supported by CoinGecko.
/// Free tier, no API key required.
///
/// # Parameters
///
/// - `coin` (required): Coin ID (e.g., "bitcoin", "ethereum", "solana", "dogecoin")
/// - `currency` (optional, default: "USD"): Currency for price (USD, EUR, GBP)
///
/// # Examples
///
/// ```json
/// {"coin": "ethereum"}
/// {"coin": "solana", "currency": "EUR"}
/// {"coin": "dogecoin"}
/// ```
///
/// # Common Coin IDs
///
/// - bitcoin, ethereum, solana, cardano, dogecoin, polkadot
/// - ripple (XRP), litecoin, chainlink, uniswap, avalanche-2
/// - For full list: https://api.coingecko.com/api/v3/coins/list
pub struct CryptoPrice {
    client: reqwest::Client,
}

impl CryptoPrice {
    /// Create a new crypto price tool.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("AmanBot/1.0")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Fetch price data from CoinGecko.
    async fn fetch_price(&self, coin: &str) -> Result<PriceData, ToolError> {
        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd,eur,gbp&include_24hr_change=true&include_market_cap=true",
            coin.to_lowercase()
        );

        debug!("Fetching crypto price from: {}", url);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "CoinGecko API returned status {}",
                response.status()
            )));
        }

        let data: HashMap<String, PriceData> = response.json().await?;

        data.into_iter()
            .next()
            .map(|(_, v)| v)
            .ok_or_else(|| ToolError::ExecutionFailed(format!(
                "Coin '{}' not found. Use coin ID like 'bitcoin', 'ethereum', 'solana'",
                coin
            )))
    }

    /// Format a large number with K/M/B suffixes.
    fn format_market_cap(value: f64) -> String {
        if value >= 1_000_000_000_000.0 {
            format!("${:.2}T", value / 1_000_000_000_000.0)
        } else if value >= 1_000_000_000.0 {
            format!("${:.2}B", value / 1_000_000_000.0)
        } else if value >= 1_000_000.0 {
            format!("${:.2}M", value / 1_000_000.0)
        } else {
            format!("${:.0}", value)
        }
    }

    /// Format price change with arrow indicator.
    fn format_change(change: f64) -> String {
        let arrow = if change >= 0.0 { "↑" } else { "↓" };
        format!("{}{:.2}%", arrow, change.abs())
    }
}

impl Default for CryptoPrice {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CryptoPrice {
    fn name(&self) -> &str {
        "crypto_price"
    }

    fn description(&self) -> &str {
        "Fetches cryptocurrency prices from CoinGecko. \
         Supports any coin: bitcoin, ethereum, solana, dogecoin, etc."
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let coin = args.get_string("coin")?;
        let currency = args
            .get_string_opt("currency")
            .unwrap_or_else(|| "USD".to_string())
            .to_uppercase();

        debug!("Getting {} price in {}", coin, currency);

        let data = self.fetch_price(&coin).await?;

        // Get price in requested currency
        let (price, symbol) = match currency.as_str() {
            "USD" => (data.usd, "$"),
            "EUR" => (data.eur, "€"),
            "GBP" => (data.gbp, "£"),
            _ => {
                return Err(ToolError::InvalidParameter {
                    name: "currency".to_string(),
                    reason: format!("Unsupported currency '{}'. Use USD, EUR, or GBP", currency),
                });
            }
        };

        let price = price.ok_or_else(|| {
            ToolError::ExecutionFailed("Price data unavailable".to_string())
        })?;

        // Build response
        let coin_upper = coin.to_uppercase();
        let mut response = format!("{}: {}{:.2} {}", coin_upper, symbol, price, currency);

        // Add 24h change if available
        if let Some(change) = data.usd_24h_change {
            response.push_str(&format!(" ({})", Self::format_change(change)));
        }

        // Add market cap if available
        if let Some(mcap) = data.usd_market_cap {
            response.push_str(&format!(" | Market Cap: {}", Self::format_market_cap(mcap)));
        }

        Ok(ToolOutput::success(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_market_cap() {
        assert_eq!(CryptoPrice::format_market_cap(1_500_000_000_000.0), "$1.50T");
        assert_eq!(CryptoPrice::format_market_cap(500_000_000_000.0), "$500.00B");
        assert_eq!(CryptoPrice::format_market_cap(1_500_000_000.0), "$1.50B");
        assert_eq!(CryptoPrice::format_market_cap(500_000_000.0), "$500.00M");
        assert_eq!(CryptoPrice::format_market_cap(1_500_000.0), "$1.50M");
        assert_eq!(CryptoPrice::format_market_cap(500_000.0), "$500000");
    }

    #[test]
    fn test_format_change() {
        assert_eq!(CryptoPrice::format_change(5.25), "↑5.25%");
        assert_eq!(CryptoPrice::format_change(-3.14), "↓3.14%");
        assert_eq!(CryptoPrice::format_change(0.0), "↑0.00%");
    }

    #[tokio::test]
    #[ignore] // Requires network
    async fn test_fetch_ethereum_price() {
        let tool = CryptoPrice::new();
        let mut params = HashMap::new();
        params.insert(
            "coin".to_string(),
            serde_json::Value::String("ethereum".to_string()),
        );

        let result = tool.execute(ToolArgs::new(params)).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("ETHEREUM"));
        assert!(result.content.contains("$"));
    }
}
