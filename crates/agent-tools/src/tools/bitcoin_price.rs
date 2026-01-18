//! Bitcoin price tool using mempool.space API.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::debug;

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// Response from mempool.space /api/v1/prices endpoint.
#[derive(Debug, Deserialize)]
struct MempoolPrices {
    #[serde(rename = "USD")]
    usd: f64,
    #[serde(rename = "EUR")]
    eur: f64,
    #[serde(rename = "GBP")]
    gbp: f64,
    #[serde(rename = "CAD")]
    cad: f64,
    #[serde(rename = "CHF")]
    chf: f64,
    #[serde(rename = "AUD")]
    aud: f64,
    #[serde(rename = "JPY")]
    jpy: f64,
}

/// Bitcoin price tool that fetches the current BTC price from mempool.space.
///
/// Uses the mempool.space API which is free, requires no API key, and is
/// privacy-friendly (no tracking, open source).
///
/// # Parameters
///
/// - `currency` (optional, default: "USD"): Currency to show price in.
///   Supported: USD, EUR, GBP, CAD, CHF, AUD, JPY
///
/// # Examples
///
/// ```json
/// {}
/// {"currency": "EUR"}
/// {"currency": "GBP"}
/// ```
pub struct BitcoinPrice {
    client: reqwest::Client,
}

impl BitcoinPrice {
    /// Create a new Bitcoin price tool.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("AmanBot/1.0")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Fetch Bitcoin prices from mempool.space.
    async fn fetch_prices(&self) -> Result<MempoolPrices, ToolError> {
        let url = "https://mempool.space/api/v1/prices";
        debug!("Fetching Bitcoin price from: {}", url);

        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "mempool.space API returned status {}",
                response.status()
            )));
        }

        let prices: MempoolPrices = response.json().await?;
        Ok(prices)
    }

    /// Get price for a specific currency.
    fn get_price(prices: &MempoolPrices, currency: &str) -> Option<(f64, &'static str)> {
        match currency.to_uppercase().as_str() {
            "USD" => Some((prices.usd, "$")),
            "EUR" => Some((prices.eur, "€")),
            "GBP" => Some((prices.gbp, "£")),
            "CAD" => Some((prices.cad, "C$")),
            "CHF" => Some((prices.chf, "CHF ")),
            "AUD" => Some((prices.aud, "A$")),
            "JPY" => Some((prices.jpy, "¥")),
            _ => None,
        }
    }
}

impl Default for BitcoinPrice {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BitcoinPrice {
    fn name(&self) -> &str {
        "bitcoin_price"
    }

    fn description(&self) -> &str {
        "Fetches the current Bitcoin price from mempool.space. \
         Supports USD, EUR, GBP, CAD, CHF, AUD, JPY."
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let currency = args
            .get_string_opt("currency")
            .unwrap_or_else(|| "USD".to_string());

        debug!("Getting Bitcoin price in {}", currency);

        let prices = self.fetch_prices().await?;

        match Self::get_price(&prices, &currency) {
            Some((price, symbol)) => {
                // Format with thousands separators
                let formatted = if currency.to_uppercase() == "JPY" {
                    format!("{}{:.0}", symbol, price)
                } else {
                    format!("{}{:.2}", symbol, price)
                };

                Ok(ToolOutput::success(format!(
                    "Bitcoin (BTC): {} {}",
                    formatted,
                    currency.to_uppercase()
                )))
            }
            None => Err(ToolError::InvalidParameter {
                name: "currency".to_string(),
                reason: format!(
                    "Unsupported currency '{}'. Supported: USD, EUR, GBP, CAD, CHF, AUD, JPY",
                    currency
                ),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_get_price() {
        let prices = MempoolPrices {
            usd: 50000.0,
            eur: 45000.0,
            gbp: 40000.0,
            cad: 65000.0,
            chf: 44000.0,
            aud: 75000.0,
            jpy: 7500000.0,
        };

        assert_eq!(BitcoinPrice::get_price(&prices, "USD"), Some((50000.0, "$")));
        assert_eq!(BitcoinPrice::get_price(&prices, "eur"), Some((45000.0, "€")));
        assert_eq!(BitcoinPrice::get_price(&prices, "GBP"), Some((40000.0, "£")));
        assert_eq!(BitcoinPrice::get_price(&prices, "INVALID"), None);
    }

    #[tokio::test]
    #[ignore] // Requires network
    async fn test_fetch_bitcoin_price() {
        let tool = BitcoinPrice::new();
        let args = ToolArgs::new(HashMap::new());

        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("Bitcoin"));
        assert!(result.content.contains("$"));
    }
}
