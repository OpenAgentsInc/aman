//! WebFetch tool for fetching and processing web content.

use async_trait::async_trait;
use brain_core::InboundMessage;
use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};
use tracing::{debug, warn};
use url::Url;

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// Maximum content length to fetch (500KB).
const MAX_CONTENT_LENGTH: usize = 500 * 1024;

/// Maximum content length for summarization (10KB).
const MAX_SUMMARIZE_LENGTH: usize = 10 * 1024;

/// Check if an IP address is private/internal (SSRF protection).
fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            // Private ranges per RFC 1918
            ipv4.is_private()
                // Loopback (127.0.0.0/8)
                || ipv4.is_loopback()
                // Link-local (169.254.0.0/16) - includes AWS metadata service
                || ipv4.is_link_local()
                // Broadcast
                || ipv4.is_broadcast()
                // Documentation ranges (192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24)
                || ipv4.is_documentation()
                // Shared address space (100.64.0.0/10) - RFC 6598
                || (ipv4.octets()[0] == 100 && (ipv4.octets()[1] & 0xC0) == 64)
                // AWS metadata service explicitly (169.254.169.254)
                || *ipv4 == Ipv4Addr::new(169, 254, 169, 254)
                // GCP metadata (169.254.169.253)
                || *ipv4 == Ipv4Addr::new(169, 254, 169, 253)
                // Unspecified (0.0.0.0)
                || ipv4.is_unspecified()
        }
        IpAddr::V6(ipv6) => {
            // Loopback (::1)
            ipv6.is_loopback()
                // Unspecified (::)
                || ipv6.is_unspecified()
                // IPv4-mapped addresses - check the embedded IPv4
                || ipv6.to_ipv4_mapped().map(|v4| is_private_ip(&IpAddr::V4(v4))).unwrap_or(false)
                // Unique local addresses (fc00::/7) - RFC 4193
                || (ipv6.segments()[0] & 0xFE00) == 0xFC00
                // Link-local (fe80::/10)
                || (ipv6.segments()[0] & 0xFFC0) == 0xFE80
        }
    }
}

/// Validate that a URL does not point to a private/internal address (SSRF protection).
async fn validate_url_ssrf(url_str: &str) -> Result<(), ToolError> {
    let url = Url::parse(url_str).map_err(|e| ToolError::InvalidParameter {
        name: "url".to_string(),
        reason: format!("Invalid URL: {}", e),
    })?;

    let host = url.host_str().ok_or_else(|| ToolError::InvalidParameter {
        name: "url".to_string(),
        reason: "URL must have a host".to_string(),
    })?;

    // Use default port 80/443 based on scheme
    let port = url.port().unwrap_or(match url.scheme() {
        "https" => 443,
        _ => 80,
    });

    // Try to resolve the hostname to IP addresses
    let addr_str = format!("{}:{}", host, port);
    let addrs = tokio::task::spawn_blocking(move || {
        addr_str.to_socket_addrs().map(|iter| iter.collect::<Vec<_>>())
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("DNS resolution task failed: {}", e)))?
    .map_err(|e| ToolError::ExecutionFailed(format!("Failed to resolve hostname: {}", e)))?;

    // Check each resolved IP
    for addr in addrs {
        if is_private_ip(&addr.ip()) {
            return Err(ToolError::InvalidParameter {
                name: "url".to_string(),
                reason: format!(
                    "URL resolves to private/internal IP address ({}). Access denied for security.",
                    addr.ip()
                ),
            });
        }
    }

    Ok(())
}

fn truncate_utf8(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_string();
    }
    if max_bytes == 0 {
        return String::new();
    }

    let mut idx = max_bytes.min(input.len());
    while idx > 0 && !input.is_char_boundary(idx) {
        idx -= 1;
    }

    input[..idx].to_string()
}

/// WebFetch tool for fetching URL content and optionally summarizing it.
///
/// Fetches web pages, converts HTML to plain text, and can optionally
/// use a brain to summarize the content.
///
/// # Parameters
///
/// - `url` (required): The URL to fetch.
/// - `summarize` (optional, default: false): If true and a brain is available,
///   summarize the fetched content.
/// - `prompt` (optional): Custom summarization prompt. Only used if summarize is true.
///
/// # Examples
///
/// ```json
/// {"url": "https://example.com"}
/// {"url": "https://news.site/article", "summarize": true}
/// {"url": "https://docs.site/page", "summarize": true, "prompt": "Extract the key points"}
/// ```
pub struct WebFetch {
    client: reqwest::Client,
}

impl WebFetch {
    /// Create a new WebFetch tool.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (compatible; AmanBot/1.0)")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Fetch content from a URL.
    async fn fetch_url(&self, url: &str) -> Result<String, ToolError> {
        debug!("Fetching URL: {}", url);

        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "HTTP {} for URL: {}",
                response.status(),
                url
            )));
        }

        // Check content type
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("text/html");

        let is_html = content_type.contains("text/html");

        // Check content length
        let content_length = response.content_length().unwrap_or(0) as usize;
        if content_length > MAX_CONTENT_LENGTH {
            return Err(ToolError::ExecutionFailed(format!(
                "Content too large: {} bytes (max: {})",
                content_length, MAX_CONTENT_LENGTH
            )));
        }

        let body = response.text().await?;

        // Truncate if necessary
        let body = if body.len() > MAX_CONTENT_LENGTH {
            truncate_utf8(&body, MAX_CONTENT_LENGTH)
        } else {
            body
        };

        // Convert HTML to text if needed
        let text = if is_html {
            html2text::from_read(body.as_bytes(), 80)
                .map_err(|e| ToolError::ExecutionFailed(format!("HTML parsing error: {}", e)))?
        } else {
            body
        };

        Ok(text)
    }

    /// Summarize content using a brain.
    async fn summarize_with_brain(
        &self,
        content: &str,
        prompt: Option<&str>,
        args: &ToolArgs,
    ) -> Result<String, ToolError> {
        let brain = args
            .brain
            .as_ref()
            .ok_or_else(|| ToolError::BrainError("No brain available for summarization".to_string()))?;

        // Truncate content for summarization
        let truncated = if content.len() > MAX_SUMMARIZE_LENGTH {
            let preview = truncate_utf8(content, MAX_SUMMARIZE_LENGTH);
            format!(
                "{}...\n\n[Content truncated, showing first {} bytes]",
                preview, MAX_SUMMARIZE_LENGTH
            )
        } else {
            content.to_string()
        };

        let summarize_prompt = prompt.unwrap_or("Please summarize the following content concisely:");
        let message_text = format!("{}\n\n---\n\n{}", summarize_prompt, truncated);

        // Create an inbound message for the brain
        let message = InboundMessage::direct("tool:web_fetch", &message_text, 0);

        match brain.process(message).await {
            Ok(response) => Ok(response.text),
            Err(e) => Err(ToolError::BrainError(format!("Summarization failed: {}", e))),
        }
    }
}

impl Default for WebFetch {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebFetch {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetches content from a URL and converts HTML to plain text. \
         Can optionally summarize the content using AI."
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let url = args.get_string("url")?;
        let summarize = args.get_bool_or("summarize", false);
        let prompt = args.get_string_opt("prompt");

        debug!("WebFetch: url={}, summarize={}", url, summarize);

        // Validate URL scheme
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(ToolError::InvalidParameter {
                name: "url".to_string(),
                reason: "URL must start with http:// or https://".to_string(),
            });
        }

        // SSRF protection: validate URL does not point to internal addresses
        validate_url_ssrf(&url).await?;

        // Fetch the content
        let content = match self.fetch_url(&url).await {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to fetch URL {}: {}", url, e);
                return Err(e);
            }
        };

        // Summarize if requested and brain is available
        if summarize {
            if args.brain.is_some() {
                match self
                    .summarize_with_brain(&content, prompt.as_deref(), &args)
                    .await
                {
                    Ok(summary) => {
                        return Ok(ToolOutput::success(format!(
                            "Summary of {}:\n\n{}",
                            url, summary
                        )));
                    }
                    Err(e) => {
                        warn!("Summarization failed, returning raw content: {}", e);
                        // Fall through to return raw content
                    }
                }
            } else {
                debug!("Summarization requested but no brain available");
            }
        }

        // Return raw content
        let preview = if content.len() > 2000 {
            let snippet = truncate_utf8(&content, 2000);
            format!(
                "{}...\n\n[Showing first {} of {} bytes]",
                snippet,
                snippet.len(),
                content.len()
            )
        } else {
            content
        };

        Ok(ToolOutput::success(format!("Content from {}:\n\n{}", url, preview)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::Value;

    fn make_args(url: &str) -> ToolArgs {
        let mut params = HashMap::new();
        params.insert("url".to_string(), Value::String(url.to_string()));
        ToolArgs::new(params)
    }

    #[tokio::test]
    async fn test_missing_url() {
        let fetch = WebFetch::new();
        let args = ToolArgs::new(HashMap::new());

        let result = fetch.execute(args).await;
        assert!(matches!(result, Err(ToolError::MissingParameter(_))));
    }

    #[tokio::test]
    async fn test_invalid_url_scheme() {
        let fetch = WebFetch::new();
        let args = make_args("ftp://example.com");

        let result = fetch.execute(args).await;
        assert!(matches!(result, Err(ToolError::InvalidParameter { .. })));
    }

    // Integration tests
    #[tokio::test]
    #[ignore]
    async fn test_fetch_example_com() {
        let fetch = WebFetch::new();
        let result = fetch.execute(make_args("https://example.com")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("Example Domain"));
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_nonexistent() {
        let fetch = WebFetch::new();
        let result = fetch
            .execute(make_args("https://this-domain-does-not-exist-12345.com"))
            .await;
        assert!(matches!(result, Err(ToolError::HttpError(_))));
    }

    // SSRF protection tests
    #[test]
    fn test_is_private_ip_v4() {
        use std::net::Ipv4Addr;

        // Private ranges
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));

        // Loopback
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));

        // Link-local / AWS metadata
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))));

        // Public IPs should NOT be private
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)))); // example.com
    }

    #[test]
    fn test_is_private_ip_v6() {
        use std::net::Ipv6Addr;

        // Loopback
        assert!(is_private_ip(&std::net::IpAddr::V6(Ipv6Addr::LOCALHOST)));

        // Unspecified
        assert!(is_private_ip(&std::net::IpAddr::V6(Ipv6Addr::UNSPECIFIED)));

        // Public IPv6 should NOT be private
        assert!(!is_private_ip(&std::net::IpAddr::V6(Ipv6Addr::new(
            0x2606, 0x2800, 0x220, 0x1, 0x248, 0x1893, 0x25c8, 0x1946
        ))));
    }

    #[tokio::test]
    async fn test_ssrf_localhost_blocked() {
        let result = validate_url_ssrf("http://localhost/admin").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ssrf_127_blocked() {
        let result = validate_url_ssrf("http://127.0.0.1/admin").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ssrf_private_ip_blocked() {
        let result = validate_url_ssrf("http://192.168.1.1/router").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ssrf_metadata_service_blocked() {
        let result = validate_url_ssrf("http://169.254.169.254/latest/meta-data/").await;
        assert!(result.is_err());
    }
}
