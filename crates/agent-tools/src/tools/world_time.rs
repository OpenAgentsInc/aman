//! World time tool using WorldTimeAPI.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::debug;

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// Response from WorldTimeAPI.
#[derive(Debug, Deserialize)]
struct WorldTimeResponse {
    datetime: String,
    timezone: String,
    utc_offset: String,
    day_of_week: i32,
    abbreviation: String,
}

/// Common timezone aliases for user convenience.
const TIMEZONE_ALIASES: &[(&str, &str)] = &[
    // US
    ("new york", "America/New_York"),
    ("nyc", "America/New_York"),
    ("los angeles", "America/Los_Angeles"),
    ("la", "America/Los_Angeles"),
    ("chicago", "America/Chicago"),
    ("denver", "America/Denver"),
    ("phoenix", "America/Phoenix"),
    ("seattle", "America/Los_Angeles"),
    ("san francisco", "America/Los_Angeles"),
    ("sf", "America/Los_Angeles"),
    ("boston", "America/New_York"),
    ("miami", "America/New_York"),
    ("est", "America/New_York"),
    ("pst", "America/Los_Angeles"),
    ("cst", "America/Chicago"),
    ("mst", "America/Denver"),
    // Europe
    ("london", "Europe/London"),
    ("paris", "Europe/Paris"),
    ("berlin", "Europe/Berlin"),
    ("amsterdam", "Europe/Amsterdam"),
    ("rome", "Europe/Rome"),
    ("madrid", "Europe/Madrid"),
    ("zurich", "Europe/Zurich"),
    ("gmt", "Europe/London"),
    ("utc", "UTC"),
    // Asia
    ("tokyo", "Asia/Tokyo"),
    ("japan", "Asia/Tokyo"),
    ("jst", "Asia/Tokyo"),
    ("beijing", "Asia/Shanghai"),
    ("shanghai", "Asia/Shanghai"),
    ("hong kong", "Asia/Hong_Kong"),
    ("singapore", "Asia/Singapore"),
    ("seoul", "Asia/Seoul"),
    ("mumbai", "Asia/Kolkata"),
    ("delhi", "Asia/Kolkata"),
    ("india", "Asia/Kolkata"),
    ("ist", "Asia/Kolkata"),
    ("dubai", "Asia/Dubai"),
    ("bangkok", "Asia/Bangkok"),
    // Australia
    ("sydney", "Australia/Sydney"),
    ("melbourne", "Australia/Melbourne"),
    ("perth", "Australia/Perth"),
    ("aest", "Australia/Sydney"),
    // Others
    ("moscow", "Europe/Moscow"),
    ("sao paulo", "America/Sao_Paulo"),
    ("brazil", "America/Sao_Paulo"),
    ("cairo", "Africa/Cairo"),
    ("johannesburg", "Africa/Johannesburg"),
    ("auckland", "Pacific/Auckland"),
    ("honolulu", "Pacific/Honolulu"),
    ("hawaii", "Pacific/Honolulu"),
];

/// World time tool using WorldTimeAPI.
///
/// Gets the current time in any timezone or city.
/// Free API, no key required.
///
/// # Parameters
///
/// - `location` (required): City name or timezone (e.g., "Tokyo", "America/New_York", "PST")
///
/// # Examples
///
/// ```json
/// {"location": "Tokyo"}
/// {"location": "New York"}
/// {"location": "Europe/London"}
/// {"location": "PST"}
/// ```
pub struct WorldTime {
    client: reqwest::Client,
}

impl WorldTime {
    /// Create a new world time tool.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("AmanBot/1.0")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Resolve a location to a timezone identifier.
    fn resolve_timezone(location: &str) -> String {
        let lower = location.to_lowercase();

        // Check aliases first
        for (alias, tz) in TIMEZONE_ALIASES {
            if lower == *alias {
                return tz.to_string();
            }
        }

        // If it looks like a timezone (contains /), use as-is
        if location.contains('/') {
            return location.to_string();
        }

        // Otherwise, try to guess the timezone format
        location.to_string()
    }

    /// Fetch time from WorldTimeAPI.
    async fn fetch_time(&self, timezone: &str) -> Result<WorldTimeResponse, ToolError> {
        let url = format!(
            "http://worldtimeapi.org/api/timezone/{}",
            timezone.replace(' ', "_")
        );

        debug!("Fetching time from: {}", url);

        let response = self.client.get(&url).send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ToolError::ExecutionFailed(format!(
                "Timezone '{}' not found. Try a city name like 'Tokyo' or 'New York', \
                 or a timezone like 'America/New_York'",
                timezone
            )));
        }

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "WorldTimeAPI returned status {}",
                response.status()
            )));
        }

        let data: WorldTimeResponse = response.json().await?;
        Ok(data)
    }

    /// Format the time response for display.
    fn format_response(data: &WorldTimeResponse, original_location: &str) -> String {
        // Parse the datetime to extract just the time
        // Format: "2024-01-15T14:30:45.123456+09:00"
        let time_str = if let Some(t_pos) = data.datetime.find('T') {
            let time_part = &data.datetime[t_pos + 1..];
            // Take just HH:MM:SS
            if time_part.len() >= 8 {
                &time_part[..8]
            } else {
                time_part
            }
        } else {
            &data.datetime
        };

        // Parse date
        let date_str = if let Some(t_pos) = data.datetime.find('T') {
            &data.datetime[..t_pos]
        } else {
            ""
        };

        // Day of week
        let day_name = match data.day_of_week {
            1 => "Monday",
            2 => "Tuesday",
            3 => "Wednesday",
            4 => "Thursday",
            5 => "Friday",
            6 => "Saturday",
            7 | 0 => "Sunday",
            _ => "",
        };

        format!(
            "**{}**\nTime: {} ({})\nDate: {}, {}\nTimezone: {} (UTC{})",
            original_location,
            time_str,
            data.abbreviation,
            day_name,
            date_str,
            data.timezone,
            data.utc_offset
        )
    }
}

impl Default for WorldTime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WorldTime {
    fn name(&self) -> &str {
        "world_time"
    }

    fn description(&self) -> &str {
        "Gets the current time in any city or timezone. \
         Supports city names (Tokyo, London) or timezone IDs (America/New_York)."
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let location = args.get_string("location")?;

        if location.trim().is_empty() {
            return Err(ToolError::InvalidParameter {
                name: "location".to_string(),
                reason: "Location cannot be empty".to_string(),
            });
        }

        debug!("Getting time for: {}", location);

        let timezone = Self::resolve_timezone(&location);
        let data = self.fetch_time(&timezone).await?;
        let formatted = Self::format_response(&data, &location);

        Ok(ToolOutput::success(formatted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_resolve_timezone_aliases() {
        assert_eq!(WorldTime::resolve_timezone("Tokyo"), "Asia/Tokyo");
        assert_eq!(WorldTime::resolve_timezone("new york"), "America/New_York");
        assert_eq!(WorldTime::resolve_timezone("NYC"), "America/New_York");
        assert_eq!(WorldTime::resolve_timezone("PST"), "America/Los_Angeles");
        assert_eq!(WorldTime::resolve_timezone("london"), "Europe/London");
    }

    #[test]
    fn test_resolve_timezone_direct() {
        assert_eq!(
            WorldTime::resolve_timezone("America/Chicago"),
            "America/Chicago"
        );
        assert_eq!(
            WorldTime::resolve_timezone("Europe/Paris"),
            "Europe/Paris"
        );
    }

    #[tokio::test]
    #[ignore] // Requires network
    async fn test_get_tokyo_time() {
        let tool = WorldTime::new();
        let mut params = HashMap::new();
        params.insert(
            "location".to_string(),
            serde_json::Value::String("Tokyo".to_string()),
        );

        let result = tool.execute(ToolArgs::new(params)).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("Tokyo"));
        assert!(result.content.contains("Asia/Tokyo"));
    }

    #[tokio::test]
    #[ignore] // Requires network
    async fn test_get_nyc_time() {
        let tool = WorldTime::new();
        let mut params = HashMap::new();
        params.insert(
            "location".to_string(),
            serde_json::Value::String("New York".to_string()),
        );

        let result = tool.execute(ToolArgs::new(params)).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("New York"));
    }
}
