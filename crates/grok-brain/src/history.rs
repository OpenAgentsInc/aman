//! Conversation history management.

use std::collections::HashMap;
use tokio::sync::RwLock;

/// A single message in the conversation history.
#[derive(Debug, Clone)]
pub struct HistoryMessage {
    /// Role: "user" or "assistant"
    pub role: String,
    /// Message content
    pub content: String,
}

impl HistoryMessage {
    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

/// Per-sender conversation history.
#[derive(Debug, Default)]
pub struct ConversationHistory {
    /// Map from sender ID to their message history.
    histories: RwLock<HashMap<String, Vec<HistoryMessage>>>,
    /// Maximum number of turns (user + assistant pairs) to keep.
    max_turns: usize,
}

impl ConversationHistory {
    /// Create a new conversation history with the given max turns.
    pub fn new(max_turns: usize) -> Self {
        Self {
            histories: RwLock::new(HashMap::new()),
            max_turns,
        }
    }

    /// Get the conversation history for a sender.
    pub async fn get(&self, sender: &str) -> Vec<HistoryMessage> {
        let histories = self.histories.read().await;
        histories.get(sender).cloned().unwrap_or_default()
    }

    /// Add a user message and assistant response to the history.
    pub async fn add_exchange(&self, sender: &str, user_msg: &str, assistant_msg: &str) {
        let mut histories = self.histories.write().await;
        let history = histories.entry(sender.to_string()).or_default();

        history.push(HistoryMessage::user(user_msg));
        history.push(HistoryMessage::assistant(assistant_msg));

        // Trim to max turns (each turn is 2 messages)
        let max_messages = self.max_turns * 2;
        if history.len() > max_messages {
            let to_remove = history.len() - max_messages;
            history.drain(0..to_remove);
        }
    }

    /// Clear history for a specific sender.
    pub async fn clear(&self, sender: &str) {
        let mut histories = self.histories.write().await;
        histories.remove(sender);
    }

    /// Clear all conversation histories.
    pub async fn clear_all(&self) {
        let mut histories = self.histories.write().await;
        histories.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_and_get_history() {
        let history = ConversationHistory::new(5);

        history.add_exchange("+1234", "Hello", "Hi there!").await;
        history
            .add_exchange("+1234", "How are you?", "I'm doing well!")
            .await;

        let messages = history.get("+1234").await;
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "Hi there!");
    }

    #[tokio::test]
    async fn test_history_trimming() {
        let history = ConversationHistory::new(2); // Keep only 2 turns

        history.add_exchange("+1234", "First", "Response 1").await;
        history.add_exchange("+1234", "Second", "Response 2").await;
        history.add_exchange("+1234", "Third", "Response 3").await;

        let messages = history.get("+1234").await;
        assert_eq!(messages.len(), 4); // 2 turns = 4 messages
        assert_eq!(messages[0].content, "Second");
        assert_eq!(messages[1].content, "Response 2");
    }

    #[tokio::test]
    async fn test_clear_history() {
        let history = ConversationHistory::new(5);

        history.add_exchange("+1234", "Hello", "Hi!").await;
        history.add_exchange("+5678", "Hey", "Hello!").await;

        history.clear("+1234").await;

        let messages1 = history.get("+1234").await;
        let messages2 = history.get("+5678").await;

        assert!(messages1.is_empty());
        assert_eq!(messages2.len(), 2);
    }

    #[tokio::test]
    async fn test_clear_all_history() {
        let history = ConversationHistory::new(5);

        history.add_exchange("+1234", "Hello", "Hi!").await;
        history.add_exchange("+5678", "Hey", "Hello!").await;

        history.clear_all().await;

        let messages1 = history.get("+1234").await;
        let messages2 = history.get("+5678").await;

        assert!(messages1.is_empty());
        assert!(messages2.is_empty());
    }
}
