//! Server-Sent Events (SSE) client for receiving messages.

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::stream::Stream;
use reqwest_eventsource::{Event, EventSource, RequestBuilderExt};
use tracing::{debug, error, info, warn};

use crate::config::DaemonConfig;
use crate::error::DaemonError;
use crate::types::{Envelope, ReceiveEvent};
use crate::SignalClient;

/// Configuration for automatic reconnection.
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Maximum number of retries (None = infinite).
    pub max_retries: Option<u32>,
    /// Initial delay before first retry.
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
    /// Backoff multiplier for each retry.
    pub backoff_multiplier: f64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            max_retries: None,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }
}

impl ReconnectConfig {
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

/// A stream of incoming Signal message envelopes.
pub struct MessageStream {
    event_source: EventSource,
    #[allow(dead_code)] // For future reconnection support
    config: DaemonConfig,
    #[allow(dead_code)] // For future reconnection support
    reconnect_config: ReconnectConfig,
    reconnect_attempts: u32,
}

impl MessageStream {
    /// Create a new message stream from a SignalClient.
    pub fn new(client: &SignalClient) -> Self {
        let config = client.config().clone();
        Self::with_config(client.http_client().clone(), config, ReconnectConfig::default())
    }

    /// Create a new message stream with custom reconnection config.
    pub fn with_reconnect(client: &SignalClient, reconnect_config: ReconnectConfig) -> Self {
        let config = client.config().clone();
        Self::with_config(client.http_client().clone(), config, reconnect_config)
    }

    fn with_config(
        _http: reqwest::Client,
        config: DaemonConfig,
        reconnect_config: ReconnectConfig,
    ) -> Self {
        let url = config.events_url();
        info!("Creating SSE connection to {}", url);

        // Create a separate HTTP client for SSE without timeout
        // SSE connections are long-lived and should not timeout
        let sse_client = reqwest::Client::builder()
            .build()
            .expect("Failed to build SSE client");

        let request = sse_client.get(&url);
        let event_source = request.eventsource().unwrap();

        Self {
            event_source,
            config,
            reconnect_config,
            reconnect_attempts: 0,
        }
    }

    /// Reconnect to the SSE endpoint.
    #[allow(dead_code)] // For future reconnection support
    fn reconnect(&mut self, http: &reqwest::Client) {
        let url = self.config.events_url();
        info!("Reconnecting to SSE endpoint: {}", url);

        let request = http.get(&url);
        self.event_source = request.eventsource().unwrap();
        self.reconnect_attempts += 1;
    }
}

impl Stream for MessageStream {
    type Item = Result<Envelope, DaemonError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match Pin::new(&mut self.event_source).poll_next(cx) {
                Poll::Ready(Some(Ok(event))) => {
                    match event {
                        Event::Open => {
                            debug!("SSE connection opened");
                            self.reconnect_attempts = 0;
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
                    return Poll::Ready(Some(Err(DaemonError::Sse(e.to_string()))));
                }
                Poll::Ready(None) => {
                    // Stream ended
                    info!("SSE stream ended");
                    return Poll::Ready(None);
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }
    }
}

/// Create a message stream from a SignalClient.
pub fn subscribe(client: &SignalClient) -> MessageStream {
    MessageStream::new(client)
}

/// Create a message stream with custom reconnection configuration.
pub fn subscribe_with_reconnect(
    client: &SignalClient,
    reconnect_config: ReconnectConfig,
) -> MessageStream {
    MessageStream::with_reconnect(client, reconnect_config)
}
