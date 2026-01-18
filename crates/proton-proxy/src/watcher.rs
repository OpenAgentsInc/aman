use std::collections::HashSet;
use std::future::Future;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::{ImapClient, InboxMessage, ProtonConfig, ProtonError};

/// Watches an IMAP folder for new messages and triggers callbacks.
pub struct InboxWatcher {
    config: ProtonConfig,
    poll_interval: Duration,
}

impl InboxWatcher {
    /// Create a new inbox watcher with default 30 second poll interval.
    pub fn new(config: ProtonConfig) -> Self {
        Self {
            config,
            poll_interval: Duration::from_secs(30),
        }
    }

    /// Set the poll interval.
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Watch a folder and call the handler for each new message.
    ///
    /// This function runs indefinitely, polling the folder at the configured interval.
    /// When a new message is detected, the handler is called with the message.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use proton_proxy::{InboxWatcher, ProtonConfig, InboxMessage, ProtonError};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), ProtonError> {
    ///     let config = ProtonConfig::from_env()?;
    ///     let watcher = InboxWatcher::new(config);
    ///     
    ///     watcher.watch("INBOX", |msg| async move {
    ///         println!("New message from {:?}: {}", msg.from, msg.subject);
    ///         Ok(())
    ///     }).await?;
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn watch<F, Fut>(&self, folder: &str, handler: F) -> Result<(), ProtonError>
    where
        F: Fn(InboxMessage) -> Fut + Send + Sync,
        Fut: Future<Output = Result<(), ProtonError>> + Send,
    {
        let mut seen_uids: HashSet<u32> = HashSet::new();
        let mut interval = interval(self.poll_interval);
        let mut first_run = true;

        info!(folder = folder, poll_interval = ?self.poll_interval, "Starting inbox watcher");

        loop {
            interval.tick().await;

            match self.poll_folder(folder, &mut seen_uids, first_run, &handler).await {
                Ok(_) => {
                    first_run = false;
                }
                Err(e) => {
                    error!("Error polling folder: {}", e);
                    // Continue polling despite errors
                }
            }
        }
    }

    /// Watch a folder and send new messages to a channel.
    ///
    /// Returns a receiver that will receive new messages as they arrive.
    pub async fn watch_channel(
        &self,
        folder: &str,
    ) -> Result<mpsc::Receiver<InboxMessage>, ProtonError> {
        let (tx, rx) = mpsc::channel(100);
        let config = self.config.clone();
        let poll_interval = self.poll_interval;
        let folder = folder.to_string();

        tokio::spawn(async move {
            let watcher = InboxWatcher {
                config,
                poll_interval,
            };

            let _ = watcher
                .watch(&folder, |msg| {
                    let tx = tx.clone();
                    async move {
                        if tx.send(msg).await.is_err() {
                            warn!("Channel closed, stopping watcher");
                        }
                        Ok(())
                    }
                })
                .await;
        });

        Ok(rx)
    }

    /// Poll folder once and process new messages.
    async fn poll_folder<F, Fut>(
        &self,
        folder: &str,
        seen_uids: &mut HashSet<u32>,
        first_run: bool,
        handler: &F,
    ) -> Result<(), ProtonError>
    where
        F: Fn(InboxMessage) -> Fut + Send + Sync,
        Fut: Future<Output = Result<(), ProtonError>> + Send,
    {
        debug!("Polling folder: {}", folder);

        let mut client = ImapClient::connect(&self.config).await?;
        client.select_folder(folder).await?;

        let current_uids: HashSet<u32> = client.fetch_uids().await?.into_iter().collect();

        // Find new UIDs (not seen before)
        let new_uids: Vec<u32> = current_uids
            .iter()
            .filter(|uid| !seen_uids.contains(uid))
            .copied()
            .collect();

        if !new_uids.is_empty() {
            if first_run {
                // On first run, just record existing UIDs without processing
                info!("Initial scan found {} existing messages", new_uids.len());
            } else {
                info!("Found {} new messages", new_uids.len());

                // Fetch and process new messages
                let messages = client.fetch_messages(&new_uids).await?;
                for msg in messages {
                    debug!(
                        uid = msg.uid,
                        subject = %msg.subject,
                        from = ?msg.from,
                        "Processing new message"
                    );

                    if let Err(e) = handler(msg).await {
                        error!("Handler error: {}", e);
                    }
                }
            }
        }

        // Update seen UIDs
        *seen_uids = current_uids;

        client.logout().await?;
        Ok(())
    }

    /// Poll folder once and return new messages since last poll.
    ///
    /// Unlike `watch`, this only polls once and returns.
    pub async fn poll_once(
        &self,
        folder: &str,
        seen_uids: &mut HashSet<u32>,
    ) -> Result<Vec<InboxMessage>, ProtonError> {
        debug!("Polling folder once: {}", folder);

        let mut client = ImapClient::connect(&self.config).await?;
        client.select_folder(folder).await?;

        let current_uids: HashSet<u32> = client.fetch_uids().await?.into_iter().collect();

        // Find new UIDs
        let new_uids: Vec<u32> = current_uids
            .iter()
            .filter(|uid| !seen_uids.contains(uid))
            .copied()
            .collect();

        let messages = if !new_uids.is_empty() {
            client.fetch_messages(&new_uids).await?
        } else {
            Vec::new()
        };

        // Update seen UIDs
        *seen_uids = current_uids;

        client.logout().await?;
        Ok(messages)
    }
}
