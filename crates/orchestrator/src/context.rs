//! Context builder for accumulating search results and other context.

use brain_core::InboundMessage;

/// Context accumulated during action execution.
///
/// This is used to augment the user's message with search results
/// and other gathered information before passing to the brain.
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Search results collected during execution.
    search_results: Vec<SearchResult>,
}

/// A single search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The original query.
    pub query: String,
    /// The search result content.
    pub content: String,
}

impl Context {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a search result to the context.
    pub fn add_search_result(&mut self, query: &str, content: &str) {
        self.search_results.push(SearchResult {
            query: query.to_string(),
            content: content.to_string(),
        });
    }

    /// Check if the context has any search results.
    pub fn has_search_results(&self) -> bool {
        !self.search_results.is_empty()
    }

    /// Get the number of search results.
    pub fn search_result_count(&self) -> usize {
        self.search_results.len()
    }

    /// Create an augmented message with the context prepended.
    ///
    /// If there are search results, they are formatted and prepended
    /// to the original message text as a system context.
    pub fn augment_message(&self, original: &InboundMessage) -> InboundMessage {
        if !self.has_search_results() {
            return original.clone();
        }

        // Build the context prefix
        let mut context_text = String::from("[SEARCH CONTEXT]\n");

        for (i, result) in self.search_results.iter().enumerate() {
            context_text.push_str(&format!(
                "--- Search {}: {} ---\n{}\n\n",
                i + 1,
                result.query,
                result.content
            ));
        }

        context_text.push_str("[USER MESSAGE]\n");
        context_text.push_str(&original.text);

        // Clone the message and update the text
        InboundMessage {
            sender: original.sender.clone(),
            text: context_text,
            timestamp: original.timestamp,
            group_id: original.group_id.clone(),
            attachments: original.attachments.clone(),
        }
    }

    /// Format the context as a string for logging/debugging.
    pub fn format_summary(&self) -> String {
        if self.search_results.is_empty() {
            return "No context gathered".to_string();
        }

        let mut summary = format!("{} search result(s):\n", self.search_results.len());
        for result in &self.search_results {
            summary.push_str(&format!("  - Query: {}\n", result.query));
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_context() {
        let context = Context::new();
        assert!(!context.has_search_results());
        assert_eq!(context.search_result_count(), 0);
    }

    #[test]
    fn test_add_search_result() {
        let mut context = Context::new();
        context.add_search_result("bitcoin price", "Bitcoin is at $50,000");

        assert!(context.has_search_results());
        assert_eq!(context.search_result_count(), 1);
    }

    #[test]
    fn test_augment_message_no_context() {
        let context = Context::new();
        let original = InboundMessage::direct("+1234567890", "Hello", 123);

        let augmented = context.augment_message(&original);
        assert_eq!(augmented.text, "Hello");
    }

    #[test]
    fn test_augment_message_with_context() {
        let mut context = Context::new();
        context.add_search_result("test query", "Test result content");

        let original = InboundMessage::direct("+1234567890", "What is the result?", 123);
        let augmented = context.augment_message(&original);

        assert!(augmented.text.contains("[SEARCH CONTEXT]"));
        assert!(augmented.text.contains("test query"));
        assert!(augmented.text.contains("Test result content"));
        assert!(augmented.text.contains("[USER MESSAGE]"));
        assert!(augmented.text.contains("What is the result?"));
    }

    #[test]
    fn test_multiple_search_results() {
        let mut context = Context::new();
        context.add_search_result("query1", "result1");
        context.add_search_result("query2", "result2");

        assert_eq!(context.search_result_count(), 2);

        let original = InboundMessage::direct("+1234567890", "test", 123);
        let augmented = context.augment_message(&original);

        assert!(augmented.text.contains("Search 1: query1"));
        assert!(augmented.text.contains("Search 2: query2"));
    }
}
