//! Delayed brain implementation - wraps another brain with artificial delay.

use std::time::Duration;

use brain_core::{async_trait, Brain, BrainError, InboundMessage, OutboundMessage};
use tokio::time::sleep;

/// A brain that wraps another brain and adds artificial delay.
///
/// Useful for testing timeout handling and simulating AI processing latency.
pub struct DelayedBrain<B: Brain> {
    inner: B,
    delay: Duration,
}

impl<B: Brain> DelayedBrain<B> {
    /// Create a new DelayedBrain wrapping the given brain with the specified delay.
    pub fn new(inner: B, delay: Duration) -> Self {
        Self { inner, delay }
    }

    /// Create a brain with a delay in milliseconds.
    pub fn with_millis(inner: B, millis: u64) -> Self {
        Self::new(inner, Duration::from_millis(millis))
    }

    /// Create a brain with a delay in seconds.
    pub fn with_secs(inner: B, secs: u64) -> Self {
        Self::new(inner, Duration::from_secs(secs))
    }
}

#[async_trait]
impl<B: Brain> Brain for DelayedBrain<B> {
    async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, BrainError> {
        sleep(self.delay).await;
        self.inner.process(message).await
    }

    fn name(&self) -> &str {
        "DelayedBrain"
    }

    async fn is_ready(&self) -> bool {
        self.inner.is_ready().await
    }

    async fn shutdown(&self) -> Result<(), BrainError> {
        self.inner.shutdown().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EchoBrain;
    use std::time::Instant;

    #[tokio::test]
    async fn test_delayed_brain() {
        let inner = EchoBrain::new();
        let brain = DelayedBrain::with_millis(inner, 100);

        let msg = InboundMessage::direct("+15551234567", "test", 1234567890);

        let start = Instant::now();
        let response = brain.process(msg).await.unwrap();
        let elapsed = start.elapsed();

        assert_eq!(response.text, "test");
        assert!(elapsed >= Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_brain_name() {
        let brain = DelayedBrain::with_millis(EchoBrain::new(), 0);
        assert_eq!(brain.name(), "DelayedBrain");
    }
}
