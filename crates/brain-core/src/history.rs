//! Conversation history management.
//!
//! This module provides per-sender conversation history tracking with
//! automatic turn-based trimming.

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

    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
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
///
/// Maintains separate conversation histories for each sender (or group),
/// with automatic trimming to a configurable maximum number of turns.
///
/// # Example
///
/// ```rust
/// use brain_core::ConversationHistory;
///
/// #[tokio::main(flavor = "current_thread")]
/// async fn main() {
///     let history = ConversationHistory::new(5); // Keep 5 turns
///
///     history.add_exchange("+1234", "Hello", "Hi there!").await;
///     history.add_exchange("+1234", "How are you?", "I'm doing well!").await;
///
///     let messages = history.get("+1234").await;
///     assert_eq!(messages.len(), 4); // 2 turns = 4 messages
/// }
/// ```
#[derive(Debug, Default)]
pub struct ConversationHistory {
    /// Map from sender ID to their message history.
    histories: RwLock<HashMap<String, Vec<HistoryMessage>>>,
    /// Optional system messages per sender (not counted toward max turns).
    system_messages: RwLock<HashMap<String, HistoryMessage>>,
    /// Maximum number of turns (user + assistant pairs) to keep.
    max_turns: usize,
}

impl ConversationHistory {
    /// Create a new conversation history with the given max turns.
    pub fn new(max_turns: usize) -> Self {
        Self {
            histories: RwLock::new(HashMap::new()),
            system_messages: RwLock::new(HashMap::new()),
            max_turns,
        }
    }

    /// Get the conversation history for a sender.
    pub async fn get(&self, sender: &str) -> Vec<HistoryMessage> {
        let histories = self.histories.read().await;
        let system_messages = self.system_messages.read().await;

        let mut messages = Vec::new();
        if let Some(system_message) = system_messages.get(sender) {
            messages.push(system_message.clone());
        }
        if let Some(history) = histories.get(sender) {
            messages.extend(history.clone());
        }
        messages
    }

    /// Add a user message and assistant response to the history.
    pub async fn add_exchange(&self, sender: &str, user_msg: &str, assistant_msg: &str) {
        let mut histories = self.histories.write().await;
        let history = histories.entry(sender.to_string()).or_default();

        history.push(HistoryMessage::user(user_msg));
        history.push(HistoryMessage::assistant(assistant_msg));

        // Trim to max turns (each turn is 2 messages)
        trim_history(history, self.max_turns);
    }

    /// Set or replace the system message for a sender (not counted toward max turns).
    pub async fn set_system_message(&self, sender: &str, content: impl Into<String>) {
        let mut system_messages = self.system_messages.write().await;
        system_messages.insert(sender.to_string(), HistoryMessage::system(content));
    }

    /// Clear history for a specific sender.
    pub async fn clear(&self, sender: &str) {
        let mut histories = self.histories.write().await;
        histories.remove(sender);
        let mut system_messages = self.system_messages.write().await;
        system_messages.remove(sender);
    }

    /// Clear all conversation histories.
    pub async fn clear_all(&self) {
        let mut histories = self.histories.write().await;
        histories.clear();
        let mut system_messages = self.system_messages.write().await;
        system_messages.clear();
    }
}

fn trim_history(history: &mut Vec<HistoryMessage>, max_turns: usize) {
    let max_messages = max_turns * 2;
    if history.len() > max_messages {
        let to_remove = history.len() - max_messages;
        history.drain(0..to_remove);
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
    async fn test_separate_sender_histories() {
        let history = ConversationHistory::new(5);

        history.add_exchange("+1111", "Hello A", "Hi A!").await;
        history.add_exchange("+2222", "Hello B", "Hi B!").await;

        let a_messages = history.get("+1111").await;
        let b_messages = history.get("+2222").await;

        assert_eq!(a_messages.len(), 2);
        assert_eq!(b_messages.len(), 2);
        assert_eq!(a_messages[0].content, "Hello A");
        assert_eq!(b_messages[0].content, "Hello B");
    }

    #[tokio::test]
    async fn test_clear_sender() {
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

    #[tokio::test]
    async fn test_system_message_in_history() {
        let history = ConversationHistory::new(2);

        history
            .set_system_message("+1234", "Memory prompt")
            .await;
        history.add_exchange("+1234", "Hello", "Hi").await;

        let messages = history.get("+1234").await;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content, "Memory prompt");
    }
}
