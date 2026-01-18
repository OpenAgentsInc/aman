//! Server-Sent Events (SSE) client for receiving messages.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::stream::Stream;
use reqwest_eventsource::{Event, EventSource, RequestBuilderExt};
use tokio::time::{sleep, Sleep};
use tracing::{debug, error, info, warn};

use crate::config::DaemonConfig;
use crate::error::DaemonError;
use crate::types::{Envelope, ReceiveEvent};
use crate::SignalClient;

/// Default SSE read timeout in seconds.
/// SSE connections should receive keep-alive pings; if no data arrives within
/// this window, the connection is considered stale.
const DEFAULT_SSE_READ_TIMEOUT_SECS: u64 = 120;

/// Default maximum reconnection attempts before giving up.
/// Provides a reasonable circuit breaker to prevent infinite retries.
const DEFAULT_MAX_RETRIES: u32 = 100;

/// Configuration for automatic reconnection.
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Maximum number of retries (None = infinite, but defaults to 100 for safety).
    pub max_retries: Option<u32>,
    /// Initial delay before first retry.
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
    /// Backoff multiplier for each retry.
    pub backoff_multiplier: f64,
    /// Read timeout for SSE connections. If no data is received within this
    /// duration, the connection is considered dead and will be reconnected.
    pub read_timeout: Duration,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            // Default to 100 retries as a circuit breaker (about 50 minutes with max backoff)
            max_retries: Some(DEFAULT_MAX_RETRIES),
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            read_timeout: Duration::from_secs(DEFAULT_SSE_READ_TIMEOUT_SECS),
        }
    }
}

impl ReconnectConfig {
    /// Create a reconnect config with finite retries.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = Some(max_retries);
        self
    }

    /// Disable reconnection (fail immediately on disconnect).
    pub fn no_reconnect() -> Self {
        Self {
            max_retries: Some(0),
            ..Default::default()
        }
    }

    /// Set the read timeout for SSE connections.
    pub fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    /// Create a config with infinite retries (use with caution).
    pub fn with_infinite_retries(mut self) -> Self {
        self.max_retries = None;
        self
    }

    /// Calculate delay for a given attempt number.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms = self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powi(attempt as i32);
        let delay = Duration::from_millis(delay_ms as u64);
        delay.min(self.max_delay)
    }

    /// Check if we should retry after the given number of attempts.
    pub fn should_retry(&self, attempts: u32) -> bool {
        self.max_retries.map_or(true, |max| attempts < max)
    }
}

/// Internal state of the message stream.
enum StreamState {
    /// Connected and receiving events.
    Connected,
    /// Waiting to reconnect after a delay.
    Reconnecting,
    /// Stream has ended (no more reconnection attempts).
    Ended,
}

/// A stream of incoming Signal message envelopes with automatic reconnection.
pub struct MessageStream {
    event_source: EventSource,
    config: DaemonConfig,
    reconnect_config: ReconnectConfig,
    reconnect_attempts: u32,
    state: StreamState,
    reconnect_delay: Option<Pin<Box<Sleep>>>,
}

impl MessageStream {
    /// Create a new message stream from a SignalClient.
    pub fn new(client: &SignalClient) -> Result<Self, DaemonError> {
        let config = client.config().clone();
        Self::with_config(config, ReconnectConfig::default())
    }

    /// Create a new message stream with custom reconnection config.
    pub fn with_reconnect(
        client: &SignalClient,
        reconnect_config: ReconnectConfig,
    ) -> Result<Self, DaemonError> {
        let config = client.config().clone();
        Self::with_config(config, reconnect_config)
    }

    fn with_config(
        config: DaemonConfig,
        reconnect_config: ReconnectConfig,
    ) -> Result<Self, DaemonError> {
        let event_source = Self::create_event_source(&config, reconnect_config.read_timeout)?;

        Ok(Self {
            event_source,
            config,
            reconnect_config,
            reconnect_attempts: 0,
            state: StreamState::Connected,
            reconnect_delay: None,
        })
    }

    /// Create a new EventSource connection to the SSE endpoint.
    fn create_event_source(
        config: &DaemonConfig,
        read_timeout: Duration,
    ) -> Result<EventSource, DaemonError> {
        let url = config.events_url();
        info!(
            "Creating SSE connection to {} (read timeout: {:?})",
            url, read_timeout
        );

        // Create a separate HTTP client for SSE with read timeout.
        // This prevents thread starvation if the daemon becomes unresponsive.
        // The read_timeout should be longer than the keep-alive interval.
        let sse_client = reqwest::Client::builder()
            .read_timeout(read_timeout)
            .build()
            .map_err(|e| DaemonError::Connection(format!("Failed to build SSE client: {}", e)))?;

        let request = sse_client.get(&url);
        let event_source = request
            .eventsource()
            .map_err(|e| DaemonError::Connection(format!("Failed to create EventSource: {}", e)))?;

        Ok(event_source)
    }
}

impl Stream for MessageStream {
    type Item = Result<Envelope, DaemonError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.state {
                StreamState::Ended => {
                    return Poll::Ready(None);
                }

                StreamState::Reconnecting => {
                    // Wait for the reconnect delay to complete
                    if let Some(delay) = self.reconnect_delay.as_mut() {
                        match delay.as_mut().poll(cx) {
                            Poll::Ready(()) => {
                                // Delay complete, attempt reconnection
                                match Self::create_event_source(
                                    &self.config,
                                    self.reconnect_config.read_timeout,
                                ) {
                                    Ok(new_source) => {
                                        self.reconnect_attempts += 1;
                                        info!(
                                            "Reconnection attempt {} succeeded",
                                            self.reconnect_attempts
                                        );
                                        self.event_source = new_source;
                                        self.reconnect_delay = None;
                                        self.state = StreamState::Connected;
                                        continue;
                                    }
                                    Err(e) => {
                                        warn!("Reconnection failed: {}", e);
                                        self.reconnect_attempts += 1;

                                        // Check if we should retry again
                                        if self.reconnect_config.should_retry(self.reconnect_attempts)
                                        {
                                            let delay_duration = self
                                                .reconnect_config
                                                .delay_for_attempt(self.reconnect_attempts);
                                            info!(
                                                "Scheduling reconnection attempt {} in {:?}",
                                                self.reconnect_attempts + 1,
                                                delay_duration
                                            );
                                            self.reconnect_delay = Some(Box::pin(sleep(delay_duration)));
                                            continue;
                                        } else {
                                            error!(
                                                "Max reconnection attempts ({}) reached, giving up",
                                                self.reconnect_attempts
                                            );
                                            self.state = StreamState::Ended;
                                            return Poll::Ready(Some(Err(
                                                DaemonError::Connection(
                                                    "Max reconnection attempts reached".to_string(),
                                                ),
                                            )));
                                        }
                                    }
                                }
                            }
                            Poll::Pending => {
                                return Poll::Pending;
                            }
                        }
                    } else {
                        // No delay set, shouldn't happen but handle gracefully
                        self.state = StreamState::Connected;
                        continue;
                    }
                }

                StreamState::Connected => {
                    match Pin::new(&mut self.event_source).poll_next(cx) {
                        Poll::Ready(Some(Ok(event))) => {
                            match event {
                                Event::Open => {
                                    debug!("SSE connection opened");
                                    self.reconnect_attempts = 0; // Reset on successful connection
                                    continue;
                                }
                                Event::Message(msg) => {
                                    // The "receive" event type contains message data
                                    if msg.event == "receive" {
                                        debug!("Received SSE event: {}", msg.event);
                                        match serde_json::from_str::<ReceiveEvent>(&msg.data) {
                                            Ok(event) => {
                                                return Poll::Ready(Some(Ok(event.envelope)));
                                            }
                                            Err(e) => {
                                                warn!("Failed to parse SSE event data: {}", e);
                                                debug!("Raw data: {}", msg.data);
                                                continue;
                                            }
                                        }
                                    } else {
                                        debug!("Ignoring SSE event type: {}", msg.event);
                                        continue;
                                    }
                                }
                            }
                        }
                        Poll::Ready(Some(Err(e))) => {
                            error!("SSE error: {}", e);

                            // Check if we should attempt reconnection
                            if self.reconnect_config.should_retry(self.reconnect_attempts) {
                                let delay_duration = self
                                    .reconnect_config
                                    .delay_for_attempt(self.reconnect_attempts);
                                info!(
                                    "SSE error, scheduling reconnection attempt {} in {:?}",
                                    self.reconnect_attempts + 1,
                                    delay_duration
                                );
                                self.reconnect_delay = Some(Box::pin(sleep(delay_duration)));
                                self.state = StreamState::Reconnecting;
                                continue;
                            } else {
                                self.state = StreamState::Ended;
                                return Poll::Ready(Some(Err(DaemonError::Sse(e.to_string()))));
                            }
                        }
                        Poll::Ready(None) => {
                            // Stream ended
                            info!("SSE stream ended");

                            // Check if we should attempt reconnection
                            if self.reconnect_config.should_retry(self.reconnect_attempts) {
                                let delay_duration = self
                                    .reconnect_config
                                    .delay_for_attempt(self.reconnect_attempts);
                                info!(
                                    "Stream ended, scheduling reconnection attempt {} in {:?}",
                                    self.reconnect_attempts + 1,
                                    delay_duration
                                );
                                self.reconnect_delay = Some(Box::pin(sleep(delay_duration)));
                                self.state = StreamState::Reconnecting;
                                continue;
                            } else {
                                self.state = StreamState::Ended;
                                return Poll::Ready(None);
                            }
                        }
                        Poll::Pending => {
                            return Poll::Pending;
                        }
                    }
                }
            }
        }
    }
}

/// Create a message stream from a SignalClient.
///
/// This uses the default reconnection configuration which retries up to 100 times
/// with exponential backoff. For infinite retries, use `subscribe_with_reconnect`
/// with `ReconnectConfig::default().with_infinite_retries()`.
pub fn subscribe(client: &SignalClient) -> Result<MessageStream, DaemonError> {
    MessageStream::new(client)
}

/// Create a message stream with custom reconnection configuration.
pub fn subscribe_with_reconnect(
    client: &SignalClient,
    reconnect_config: ReconnectConfig,
) -> Result<MessageStream, DaemonError> {
    MessageStream::with_reconnect(client, reconnect_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconnect_config_default() {
        let config = ReconnectConfig::default();
        // Default now has circuit breaker at 100 retries
        assert_eq!(config.max_retries, Some(DEFAULT_MAX_RETRIES));
        assert_eq!(config.initial_delay, Duration::from_millis(500));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert_eq!(config.backoff_multiplier, 2.0);
        assert_eq!(
            config.read_timeout,
            Duration::from_secs(DEFAULT_SSE_READ_TIMEOUT_SECS)
        );
    }

    #[test]
    fn test_reconnect_config_delay_calculation() {
        let config = ReconnectConfig::default();

        // First attempt: 500ms
        assert_eq!(config.delay_for_attempt(0), Duration::from_millis(500));
        // Second attempt: 1000ms
        assert_eq!(config.delay_for_attempt(1), Duration::from_millis(1000));
        // Third attempt: 2000ms
        assert_eq!(config.delay_for_attempt(2), Duration::from_millis(2000));
        // Sixth attempt: would be 16000ms but capped at 30000ms
        assert_eq!(config.delay_for_attempt(6), Duration::from_secs(30));
    }

    #[test]
    fn test_reconnect_config_should_retry() {
        // Default config has circuit breaker at 100 retries
        let default_config = ReconnectConfig::default();
        assert!(default_config.should_retry(0));
        assert!(default_config.should_retry(99));
        assert!(!default_config.should_retry(100));

        // Infinite retries for when needed
        let infinite_config = ReconnectConfig::default().with_infinite_retries();
        assert!(infinite_config.should_retry(0));
        assert!(infinite_config.should_retry(100));
        assert!(infinite_config.should_retry(1000));

        let limited_config = ReconnectConfig::default().with_max_retries(5);
        assert!(limited_config.should_retry(0));
        assert!(limited_config.should_retry(4));
        assert!(!limited_config.should_retry(5));
        assert!(!limited_config.should_retry(6));
    }

    #[test]
    fn test_reconnect_config_no_reconnect() {
        let config = ReconnectConfig::no_reconnect();
        assert_eq!(config.max_retries, Some(0));
        assert!(!config.should_retry(0));
    }
}
