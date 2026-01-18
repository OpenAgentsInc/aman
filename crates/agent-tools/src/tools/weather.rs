//! Weather tool using wttr.in API.

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// Weather tool that fetches weather information from wttr.in.
///
/// The wttr.in service is free and requires no API key. It provides
/// weather information in various formats.
///
/// # Parameters
///
/// - `location` (required): City name, airport code, or coordinates.
/// - `format` (optional): Output format. Options:
///   - "short" (default): One-line summary with temperature and conditions.
///   - "full": Multi-line detailed forecast.
///   - "json": Raw JSON data.
///
/// # Examples
///
/// ```json
/// {"location": "New York"}
/// {"location": "London", "format": "full"}
/// {"location": "LAX", "format": "short"}
/// ```
pub struct Weather {
    client: reqwest::Client,
}

impl Weather {
    /// Create a new weather tool.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("curl/8.0.0") // wttr.in serves different content based on user agent
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Fetch weather for a location with the given format.
    async fn fetch_weather(&self, location: &str, format: &str) -> Result<String, ToolError> {
        // URL-encode the location
        let encoded_location = urlencoding::encode(location);

        let url = match format {
            "full" => format!("https://wttr.in/{}?T", encoded_location),
            "json" => format!("https://wttr.in/{}?format=j1", encoded_location),
            _ => {
                // Short format: one-liner with emoji, condition, temp
                format!("https://wttr.in/{}?format=%c+%C+%t", encoded_location)
            }
        };

        debug!("Fetching weather from: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "Weather API returned status {}",
                response.status()
            )));
        }

        let body = response.text().await?;

        // Check for error responses from wttr.in
        if body.contains("Unknown location") || body.contains("Sorry") {
            return Err(ToolError::ExecutionFailed(format!(
                "Location not found: {}",
                location
            )));
        }

        Ok(body.trim().to_string())
    }
}

impl Default for Weather {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for Weather {
    fn name(&self) -> &str {
        "weather"
    }

    fn description(&self) -> &str {
        "Fetches current weather for a location using wttr.in. \
         Supports city names, airport codes, and coordinates."
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let location = args.get_string("location")?;
        let format = args.get_string_opt("format").unwrap_or_else(|| "short".to_string());

        debug!("Getting weather for '{}' (format: {})", location, format);

        match self.fetch_weather(&location, &format).await {
            Ok(weather) => {
                if format == "short" {
                    Ok(ToolOutput::success(format!(
                        "Weather in {}: {}",
                        location, weather
                    )))
                } else {
                    Ok(ToolOutput::success(weather))
                }
            }
            Err(e) => {
                warn!("Weather fetch failed: {}", e);
                Err(e)
            }
        }
    }
}

// Inline urlencoding to avoid adding another dependency
mod urlencoding {
    pub fn encode(input: &str) -> String {
        let mut result = String::new();
        for c in input.chars() {
            match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                    result.push(c);
                }
                ' ' => result.push('+'),
                _ => {
                    for byte in c.to_string().as_bytes() {
                        result.push_str(&format!("%{:02X}", byte));
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::Value;

    fn make_args(location: &str) -> ToolArgs {
        let mut params = HashMap::new();
        params.insert("location".to_string(), Value::String(location.to_string()));
        ToolArgs::new(params)
    }

    fn make_args_with_format(location: &str, format: &str) -> ToolArgs {
        let mut params = HashMap::new();
        params.insert("location".to_string(), Value::String(location.to_string()));
        params.insert("format".to_string(), Value::String(format.to_string()));
        ToolArgs::new(params)
    }

    #[test]
    fn test_url_encoding() {
        assert_eq!(urlencoding::encode("New York"), "New+York");
        assert_eq!(urlencoding::encode("London"), "London");
        assert_eq!(urlencoding::encode("São Paulo"), "S%C3%A3o+Paulo");
    }

    #[tokio::test]
    async fn test_missing_location() {
        let weather = Weather::new();
        let args = ToolArgs::new(HashMap::new());

        let result = weather.execute(args).await;
        assert!(matches!(result, Err(ToolError::MissingParameter(_))));
    }

    // Integration tests that require network access
    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_weather_fetch_short() {
        let weather = Weather::new();
        let result = weather.execute(make_args("London")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("London"));
    }

    #[tokio::test]
    #[ignore]
    async fn test_weather_fetch_full() {
        let weather = Weather::new();
        let result = weather
            .execute(make_args_with_format("Tokyo", "full"))
            .await
            .unwrap();
        assert!(result.success);
        // Full format contains multiple lines
        assert!(result.content.lines().count() > 1);
    }
}
