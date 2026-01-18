//! Currency converter tool using exchangerate.host API.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::debug;

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// Response from exchangerate.host API.
#[derive(Debug, Deserialize)]
struct ExchangeRateResponse {
    success: bool,
    result: Option<f64>,
    error: Option<ExchangeRateError>,
}

#[derive(Debug, Deserialize)]
struct ExchangeRateError {
    info: Option<String>,
}

/// Currency converter tool using exchangerate.host API.
///
/// Converts between fiat currencies. Free API, no key required.
///
/// # Parameters
///
/// - `amount` (required): Amount to convert (number)
/// - `from` (required): Source currency code (e.g., "USD", "EUR")
/// - `to` (required): Target currency code (e.g., "GBP", "JPY")
///
/// # Examples
///
/// ```json
/// {"amount": 100, "from": "USD", "to": "EUR"}
/// {"amount": 50, "from": "GBP", "to": "JPY"}
/// {"amount": 1000, "from": "EUR", "to": "USD"}
/// ```
///
/// # Supported Currencies
///
/// USD, EUR, GBP, JPY, CAD, AUD, CHF, CNY, INR, MXN, BRL, KRW, and many more.
pub struct CurrencyConverter {
    client: reqwest::Client,
}

impl CurrencyConverter {
    /// Create a new currency converter tool.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("AmanBot/1.0")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Fetch exchange rate and convert.
    async fn convert(&self, amount: f64, from: &str, to: &str) -> Result<f64, ToolError> {
        let url = format!(
            "https://api.exchangerate.host/convert?from={}&to={}&amount={}",
            from.to_uppercase(),
            to.to_uppercase(),
            amount
        );

        debug!("Fetching exchange rate from: {}", url);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "Exchange rate API returned status {}",
                response.status()
            )));
        }

        let data: ExchangeRateResponse = response.json().await?;

        if !data.success {
            let error_msg = data
                .error
                .and_then(|e| e.info)
                .unwrap_or_else(|| "Unknown error".to_string());
            return Err(ToolError::ExecutionFailed(format!(
                "Currency conversion failed: {}",
                error_msg
            )));
        }

        data.result.ok_or_else(|| {
            ToolError::ExecutionFailed("No conversion result returned".to_string())
        })
    }

    /// Get currency symbol for common currencies.
    fn get_symbol(currency: &str) -> &'static str {
        match currency.to_uppercase().as_str() {
            "USD" => "$",
            "EUR" => "€",
            "GBP" => "£",
            "JPY" => "¥",
            "CNY" => "¥",
            "KRW" => "₩",
            "INR" => "₹",
            "BRL" => "R$",
            "CAD" => "C$",
            "AUD" => "A$",
            "CHF" => "CHF",
            "MXN" => "MX$",
            _ => "",
        }
    }
}

impl Default for CurrencyConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CurrencyConverter {
    fn name(&self) -> &str {
        "currency_converter"
    }

    fn description(&self) -> &str {
        "Converts between fiat currencies. \
         Supports USD, EUR, GBP, JPY, CAD, AUD, CHF, CNY, INR, and many more."
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let amount = args.get_f64("amount")?;
        let from = args.get_string("from")?.to_uppercase();
        let to = args.get_string("to")?.to_uppercase();

        if amount <= 0.0 {
            return Err(ToolError::InvalidParameter {
                name: "amount".to_string(),
                reason: "Amount must be positive".to_string(),
            });
        }

        debug!("Converting {} {} to {}", amount, from, to);

        let result = self.convert(amount, &from, &to).await?;

        let from_symbol = Self::get_symbol(&from);
        let to_symbol = Self::get_symbol(&to);

        // Format based on currency (JPY and KRW don't use decimals)
        let result_formatted = if matches!(to.as_str(), "JPY" | "KRW") {
            format!("{:.0}", result)
        } else {
            format!("{:.2}", result)
        };

        let amount_formatted = if matches!(from.as_str(), "JPY" | "KRW") {
            format!("{:.0}", amount)
        } else {
            format!("{:.2}", amount)
        };

        Ok(ToolOutput::success(format!(
            "{}{} {} = {}{} {}",
            from_symbol, amount_formatted, from, to_symbol, result_formatted, to
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_symbol() {
        assert_eq!(CurrencyConverter::get_symbol("USD"), "$");
        assert_eq!(CurrencyConverter::get_symbol("eur"), "€");
        assert_eq!(CurrencyConverter::get_symbol("GBP"), "£");
        assert_eq!(CurrencyConverter::get_symbol("JPY"), "¥");
        assert_eq!(CurrencyConverter::get_symbol("UNKNOWN"), "");
    }

    #[tokio::test]
    #[ignore] // Requires network
    async fn test_convert_usd_to_eur() {
        let tool = CurrencyConverter::new();
        let mut params = std::collections::HashMap::new();
        params.insert(
            "amount".to_string(),
            serde_json::Value::Number(serde_json::Number::from_f64(100.0).unwrap()),
        );
        params.insert(
            "from".to_string(),
            serde_json::Value::String("USD".to_string()),
        );
        params.insert(
            "to".to_string(),
            serde_json::Value::String("EUR".to_string()),
        );

        let result = tool.execute(ToolArgs::new(params)).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("USD"));
        assert!(result.content.contains("EUR"));
    }
}
