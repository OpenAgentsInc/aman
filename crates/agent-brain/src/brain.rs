//! AgentBrain implementation.

#[cfg(feature = "nostr")]
use std::sync::Arc;

use brain_core::{async_trait, Brain, BrainError, InboundMessage, OutboundMessage};
use database::{notification, topic, user, Database, DatabaseError, User};
#[cfg(feature = "nostr")]
use nostr_persistence::{NostrIndexer, NostrPublisher};

use crate::config::AgentBrainConfig;
use crate::events::RegionEvent;
use crate::regions::{canonicalize_region, normalize_region_input};

/// Core brain implementation for Aman.
#[derive(Clone)]
pub struct AgentBrain {
    db: Database,
    config: AgentBrainConfig,
    #[cfg(feature = "nostr")]
    pub nostr_publisher: Option<Arc<dyn NostrPublisher>>,
    #[cfg(feature = "nostr")]
    pub nostr_indexer: Option<Arc<dyn NostrIndexer>>,
}

impl AgentBrain {
    /// Create a new AgentBrain with the given configuration.
    pub async fn new(config: AgentBrainConfig) -> Result<Self, BrainError> {
        let db = Database::connect(&config.sqlite_url)
            .await
            .map_err(|e| BrainError::Configuration(format!("db connect failed: {}", e)))?;
        db.migrate()
            .await
            .map_err(|e| BrainError::Configuration(format!("db migrate failed: {}", e)))?;

        Ok(Self {
            db,
            config,
            #[cfg(feature = "nostr")]
            nostr_publisher: None,
            #[cfg(feature = "nostr")]
            nostr_indexer: None,
        })
    }

    /// Build from environment variables.
    pub async fn from_env() -> Result<Self, BrainError> {
        let config = AgentBrainConfig::from_env()?;
        Self::new(config).await
    }

    /// Create a brain with optional Nostr helpers.
    #[cfg(feature = "nostr")]
    pub fn with_nostr(
        db: Database,
        config: AgentBrainConfig,
        publisher: Option<Arc<dyn NostrPublisher>>,
        indexer: Option<Arc<dyn NostrIndexer>>,
    ) -> Self {
        Self {
            db,
            config,
            nostr_publisher: publisher,
            nostr_indexer: indexer,
        }
    }

    /// Get a reference to the database.
    pub fn db(&self) -> &Database {
        &self.db
    }

    async fn ensure_user(&self, sender: &str) -> Result<(), BrainError> {
        match user::get_user(self.db.pool(), sender).await {
            Ok(_) => Ok(()),
            Err(DatabaseError::NotFound { .. }) => {
                let new_user = User {
                    id: sender.to_string(),
                    name: sender.to_string(),
                    language: self.config.default_language.clone(),
                };
                user::create_user(self.db.pool(), &new_user)
                    .await
                    .map_err(map_db_error)
            }
            Err(e) => Err(map_db_error(e)),
        }
    }

    async fn list_topics(&self) -> Result<Vec<String>, BrainError> {
        let topics = topic::list_topics(self.db.pool())
            .await
            .map_err(map_db_error)?;
        Ok(topics.into_iter().map(|t| t.slug).collect())
    }

    async fn get_subscriptions(&self, sender: &str) -> Result<Vec<String>, BrainError> {
        let subs = notification::get_user_subscriptions(self.db.pool(), sender)
            .await
            .map_err(map_db_error)?;
        Ok(subs.into_iter().map(|t| t.slug).collect())
    }

    async fn unsubscribe_all(&self, sender: &str) -> Result<usize, BrainError> {
        let subs = notification::get_user_subscriptions(self.db.pool(), sender)
            .await
            .map_err(map_db_error)?;
        let mut removed = 0usize;
        for sub in subs {
            if notification::unsubscribe(self.db.pool(), sender, &sub.slug)
                .await
                .is_ok()
            {
                removed += 1;
            }
        }
        Ok(removed)
    }

    async fn resolve_region_slug(&self, raw: &str) -> Result<Option<String>, BrainError> {
        let cleaned = normalize_region_input(raw);
        if cleaned.is_empty() {
            return Ok(None);
        }

        let canonical = canonicalize_region(&cleaned);
        if topic::get_topic(self.db.pool(), &canonical).await.is_ok() {
            return Ok(Some(canonical));
        }

        let dashed = canonical.replace(' ', "-");
        if topic::get_topic(self.db.pool(), &dashed).await.is_ok() {
            return Ok(Some(dashed));
        }

        let plus = canonical.replace(' ', "+");
        if topic::get_topic(self.db.pool(), &plus).await.is_ok() {
            return Ok(Some(plus));
        }

        Ok(None)
    }

    async fn set_region_subscription(&self, sender: &str, region_slug: &str) -> Result<(), BrainError> {
        self.unsubscribe_all(sender).await?;
        notification::subscribe(self.db.pool(), sender, region_slug)
            .await
            .map_err(map_db_error)
    }

    fn help_text(&self, topics: &[String]) -> String {
        let mut lines = vec![
            "Commands: help, status, subscribe <region>, region <region>, stop".to_string(),
            "Example: subscribe Iran".to_string(),
        ];
        if !topics.is_empty() {
            lines.push(format!("Available regions: {}", topics.join(", ")));
        }
        lines.join("\n")
    }

    fn onboarding_text(&self) -> String {
        "Want regional alerts? Reply with a region (e.g., Iran) or send 'stop'.".to_string()
    }

    fn unknown_region_text(&self, topics: &[String]) -> String {
        if topics.is_empty() {
            "I didn't recognize that region. Try again or send 'help'.".to_string()
        } else {
            format!(
                "I didn't recognize that region. Try one of: {}",
                topics.join(", ")
            )
        }
    }

    /// Fan out a regional event to all subscribers.
    pub async fn fanout_event(&self, event: &RegionEvent) -> Result<Vec<OutboundMessage>, BrainError> {
        let Some(region_slug) = self.resolve_region_slug(&event.region).await? else {
            return Ok(Vec::new());
        };
        let subscribers = notification::get_topic_subscribers(self.db.pool(), &region_slug)
            .await
            .map_err(map_db_error)?;
        let body = event.render_alert();

        Ok(subscribers
            .into_iter()
            .map(|user| OutboundMessage::direct(user.id, body.clone()))
            .collect())
    }
}

#[async_trait]
impl Brain for AgentBrain {
    async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, BrainError> {
        self.ensure_user(&message.sender).await?;

        let text = message.text.trim();
        if text.is_empty() {
            if message.has_attachments() {
                return Ok(OutboundMessage::reply_to(
                    &message,
                    "Thanks for the attachment. I can only process text for now.",
                ));
            }
            return Ok(OutboundMessage::reply_to(&message, self.onboarding_text()));
        }

        let (command, rest) = split_command(text);
        let command_lower = command.to_lowercase();

        let topics = self.list_topics().await?;
        let subscriptions = self.get_subscriptions(&message.sender).await?;

        match command_lower.as_str() {
            "help" | "?" => Ok(OutboundMessage::reply_to(&message, self.help_text(&topics))),
            "status" => {
                if subscriptions.is_empty() {
                    Ok(OutboundMessage::reply_to(
                        &message,
                        "You're not subscribed to any alerts. Reply with a region to opt in.",
                    ))
                } else {
                    Ok(OutboundMessage::reply_to(
                        &message,
                        format!("You're subscribed to: {}", subscriptions.join(", ")),
                    ))
                }
            }
            "stop" | "unsubscribe" => {
                let _ = self.unsubscribe_all(&message.sender).await?;
                Ok(OutboundMessage::reply_to(
                    &message,
                    "You're unsubscribed. Send 'subscribe <region>' to opt back in.",
                ))
            }
            "subscribe" | "region" => {
                if rest.is_empty() {
                    return Ok(OutboundMessage::reply_to(
                        &message,
                        "Please provide a region. Example: subscribe Iran",
                    ));
                }

                if let Some(region_slug) = self.resolve_region_slug(rest).await? {
                    self.set_region_subscription(&message.sender, &region_slug)
                        .await?;
                    Ok(OutboundMessage::reply_to(
                        &message,
                        format!("Subscribed to {} alerts.", region_slug),
                    ))
                } else {
                    Ok(OutboundMessage::reply_to(
                        &message,
                        self.unknown_region_text(&topics),
                    ))
                }
            }
            "yes" => Ok(OutboundMessage::reply_to(
                &message,
                "Which region should I watch? Example: Iran",
            )),
            "no" => {
                let _ = self.unsubscribe_all(&message.sender).await?;
                Ok(OutboundMessage::reply_to(
                    &message,
                    "Okay. You won't receive regional alerts.",
                ))
            }
            _ => {
                if let Some(region_slug) = self.resolve_region_slug(text).await? {
                    self.set_region_subscription(&message.sender, &region_slug)
                        .await?;
                    Ok(OutboundMessage::reply_to(
                        &message,
                        format!("Subscribed to {} alerts.", region_slug),
                    ))
                } else if subscriptions.is_empty() {
                    Ok(OutboundMessage::reply_to(&message, self.onboarding_text()))
                } else {
                    Ok(OutboundMessage::reply_to(
                        &message,
                        "Send 'help' for commands or 'status' to view subscriptions.",
                    ))
                }
            }
        }
    }

    fn name(&self) -> &str {
        "AgentBrain"
    }
}

fn map_db_error(error: DatabaseError) -> BrainError {
    BrainError::ProcessingFailed(format!("database error: {}", error))
}

fn split_command(text: &str) -> (&str, &str) {
    let trimmed = text.trim();
    let mut parts = trimmed.splitn(2, |c: char| c.is_whitespace());
    let command = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();
    (command, rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_command() {
        let (cmd, rest) = split_command("subscribe iran");
        assert_eq!(cmd, "subscribe");
        assert_eq!(rest, "iran");

        let (cmd, rest) = split_command("status");
        assert_eq!(cmd, "status");
        assert_eq!(rest, "");
    }
}
