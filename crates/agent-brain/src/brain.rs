//! AgentBrain implementation.

#[cfg(feature = "nostr")]
use std::sync::Arc;

use brain_core::{async_trait, Brain, BrainError, InboundMessage, OutboundMessage};
use database::{user, Database, DatabaseError, User};
#[cfg(feature = "nostr")]
use nostr_persistence::{NostrIndexer, NostrPublisher};

use crate::config::AgentBrainConfig;

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

    fn help_text(&self) -> String {
        "Commands: help, status\nSend a message to chat.".to_string()
    }

    fn welcome_text(&self) -> String {
        "Welcome! Send a message to get started, or 'help' for commands.".to_string()
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
            return Ok(OutboundMessage::reply_to(&message, self.welcome_text()));
        }

        let (command, _rest) = split_command(text);
        let command_lower = command.to_lowercase();

        match command_lower.as_str() {
            "help" | "?" => Ok(OutboundMessage::reply_to(&message, self.help_text())),
            "status" => Ok(OutboundMessage::reply_to(
                &message,
                "AgentBrain is running.",
            )),
            _ => Ok(OutboundMessage::reply_to(
                &message,
                format!("You said: {}", text),
            )),
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
        let (cmd, rest) = split_command("help me");
        assert_eq!(cmd, "help");
        assert_eq!(rest, "me");

        let (cmd, rest) = split_command("status");
        assert_eq!(cmd, "status");
        assert_eq!(rest, "");
    }
}
