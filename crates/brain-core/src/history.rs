//! Conversation history management.
//!
//! This module provides per-sender conversation history tracking with
//! automatic turn-based trimming and LRU eviction to prevent memory exhaustion.

use indexmap::IndexMap;
use tokio::sync::RwLock;

/// Default maximum number of senders to track before LRU eviction.
const DEFAULT_MAX_SENDERS: usize = 10000;

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

/// Per-sender conversation history with LRU eviction.
///
/// Maintains separate conversation histories for each sender (or group),
/// with automatic trimming to a configurable maximum number of turns.
///
/// To prevent memory exhaustion from attackers sending messages from many
/// unique senders, this struct also limits the total number of tracked
/// senders and evicts the least recently used senders when the limit is reached.
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
#[derive(Debug)]
pub struct ConversationHistory {
    /// Map from sender ID to their message history.
    /// Uses IndexMap to maintain insertion order for LRU eviction.
    histories: RwLock<IndexMap<String, Vec<HistoryMessage>>>,
    /// Map from sender ID to their system message (memory prompt).
    system_messages: RwLock<IndexMap<String, String>>,
    /// Maximum number of turns (user + assistant pairs) to keep per sender.
    max_turns: usize,
    /// Maximum number of senders to track before LRU eviction.
    max_senders: usize,
}

impl Default for ConversationHistory {
    fn default() -> Self {
        Self::new(10)
    }
}

impl ConversationHistory {
    /// Create a new conversation history with the given max turns.
    ///
    /// Uses the default max senders limit (10,000).
    pub fn new(max_turns: usize) -> Self {
        Self::with_limits(max_turns, DEFAULT_MAX_SENDERS)
    }

    /// Create a new conversation history with custom limits.
    ///
    /// # Arguments
    ///
    /// * `max_turns` - Maximum number of turns (user + assistant pairs) per sender
    /// * `max_senders` - Maximum number of senders to track before LRU eviction
    pub fn with_limits(max_turns: usize, max_senders: usize) -> Self {
        Self {
            histories: RwLock::new(IndexMap::new()),
            system_messages: RwLock::new(IndexMap::new()),
            max_turns,
            max_senders,
        }
    }

    /// Get the conversation history for a sender.
    ///
    /// This marks the sender as recently used for LRU purposes.
    pub async fn get(&self, sender: &str) -> Vec<HistoryMessage> {
        let mut histories = self.histories.write().await;

        // Move to end to mark as recently used (LRU behavior)
        if let Some(entry) = histories.shift_remove(sender) {
            let result = entry.clone();
            histories.insert(sender.to_string(), entry);
            result
        } else {
            Vec::new()
        }
    }

    /// Add a user message and assistant response to the history.
    ///
    /// This also performs LRU eviction if the sender limit is exceeded.
    pub async fn add_exchange(&self, sender: &str, user_msg: &str, assistant_msg: &str) {
        let mut histories = self.histories.write().await;

        // Remove and re-insert to move to end (mark as recently used)
        let history = histories.shift_remove(sender).unwrap_or_default();
        let mut history = history;

        history.push(HistoryMessage::user(user_msg));
        history.push(HistoryMessage::assistant(assistant_msg));

        // Trim to max turns (each turn is 2 messages)
        let max_messages = self.max_turns * 2;
        if history.len() > max_messages {
            let to_remove = history.len() - max_messages;
            history.drain(0..to_remove);
        }

        histories.insert(sender.to_string(), history);

        // LRU eviction: remove oldest entries if we exceed max_senders
        while histories.len() > self.max_senders {
            // shift_remove removes the first (oldest) entry
            histories.shift_remove_index(0);
        }
    }

    /// Clear history for a specific sender.
    pub async fn clear(&self, sender: &str) {
        let mut histories = self.histories.write().await;
        histories.shift_remove(sender);
        let mut system_messages = self.system_messages.write().await;
        system_messages.shift_remove(sender);
    }

    /// Clear all conversation histories.
    pub async fn clear_all(&self) {
        let mut histories = self.histories.write().await;
        histories.clear();
        let mut system_messages = self.system_messages.write().await;
        system_messages.clear();
    }

    /// Get the current number of tracked senders.
    pub async fn sender_count(&self) -> usize {
        let histories = self.histories.read().await;
        histories.len()
    }

    /// Set a system message (memory prompt) for a sender.
    ///
    /// This message will be prepended to the conversation when building messages.
    pub async fn set_system_message(&self, sender: &str, message: impl Into<String>) {
        let mut system_messages = self.system_messages.write().await;
        system_messages.insert(sender.to_string(), message.into());
    }

    /// Get the system message for a sender, if any.
    pub async fn get_system_message(&self, sender: &str) -> Option<String> {
        let system_messages = self.system_messages.read().await;
        system_messages.get(sender).cloned()
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
    async fn test_lru_eviction() {
        // Create history with max 3 senders
        let history = ConversationHistory::with_limits(5, 3);

        // Add 4 senders
        history.add_exchange("+1111", "Hello", "Hi!").await;
        history.add_exchange("+2222", "Hello", "Hi!").await;
        history.add_exchange("+3333", "Hello", "Hi!").await;
        history.add_exchange("+4444", "Hello", "Hi!").await;

        // Should have evicted +1111 (oldest)
        assert_eq!(history.sender_count().await, 3);
        let oldest = history.get("+1111").await;
        assert!(oldest.is_empty(), "Oldest sender should have been evicted");

        // +2222, +3333, +4444 should still exist
        assert!(!history.get("+2222").await.is_empty());
        assert!(!history.get("+3333").await.is_empty());
        assert!(!history.get("+4444").await.is_empty());
    }

    #[tokio::test]
    async fn test_lru_access_order() {
        // Create history with max 3 senders
        let history = ConversationHistory::with_limits(5, 3);

        // Add 3 senders
        history.add_exchange("+1111", "Hello", "Hi!").await;
        history.add_exchange("+2222", "Hello", "Hi!").await;
        history.add_exchange("+3333", "Hello", "Hi!").await;

        // Access +1111 to make it recently used
        let _ = history.get("+1111").await;

        // Add a 4th sender - should evict +2222 (now oldest)
        history.add_exchange("+4444", "Hello", "Hi!").await;

        // +2222 should be evicted
        assert!(history.get("+2222").await.is_empty());

        // +1111, +3333, +4444 should still exist
        assert!(!history.get("+1111").await.is_empty());
        assert!(!history.get("+3333").await.is_empty());
        assert!(!history.get("+4444").await.is_empty());
    }
}
