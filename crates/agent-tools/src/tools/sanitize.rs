//! Sanitize tool for PII detection and anonymization.

use async_trait::async_trait;
use brain_core::InboundMessage;
use tracing::{debug, info};

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// System prompt for the sanitization brain.
const SANITIZE_SYSTEM_PROMPT: &str = r#"You are a PII (Personally Identifiable Information) sanitizer. Your job is to detect and replace sensitive personal information with generic placeholders.

Replace the following types of PII with placeholders:
- Names → [NAME], [NAME_2], etc.
- Phone numbers → [PHONE]
- Email addresses → [EMAIL]
- Social Security Numbers → [SSN]
- Credit card numbers → [CARD]
- Bank account numbers → [ACCOUNT]
- Addresses → [ADDRESS]
- Dates of birth → [DOB]
- Medical conditions/diagnoses → [MEDICAL]
- Salaries/income amounts → [INCOME]
- Specific monetary amounts in personal context → [AMOUNT]
- Passwords/secrets → [SECRET]
- IP addresses → [IP]
- License plate numbers → [LICENSE]
- Passport/ID numbers → [ID]

Rules:
1. ONLY replace actual PII - don't replace general references like "my friend" or "someone"
2. Keep the rest of the text exactly as-is
3. Use numbered placeholders when there are multiple of the same type (e.g., [NAME], [NAME_2])
4. Preserve the grammatical structure of the sentence
5. Output ONLY the sanitized text - no explanations, no JSON, no markdown

Examples:

Input: "My name is John Smith and my SSN is 123-45-6789"
Output: My name is [NAME] and my SSN is [SSN]

Input: "Call me at 555-123-4567 or email john@example.com"
Output: Call me at [PHONE] or email [EMAIL]

Input: "I make $150,000 a year and live at 123 Main St, NYC"
Output: I make [INCOME] a year and live at [ADDRESS]

Input: "What's the weather like today?"
Output: What's the weather like today?

Input: "My doctor said I have diabetes and prescribed metformin"
Output: My doctor said I have [MEDICAL] and prescribed metformin
"#;

/// Sanitize tool that uses a Brain to detect and replace PII.
///
/// This tool requires a Brain to be provided in the ToolArgs.
/// It will use the brain to analyze text and replace personal
/// information with generic placeholders.
///
/// # Parameters
///
/// - `text` (required): The text to sanitize.
///
/// # Returns
///
/// The sanitized text with PII replaced by placeholders.
///
/// # Example
///
/// ```json
/// {"text": "My name is John and my SSN is 123-45-6789"}
/// ```
///
/// Returns: "My name is [NAME] and my SSN is [SSN]"
pub struct Sanitize;

impl Sanitize {
    /// Create a new sanitize tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for Sanitize {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for Sanitize {
    fn name(&self) -> &str {
        "sanitize"
    }

    fn description(&self) -> &str {
        "Detects and replaces personally identifiable information (PII) with generic placeholders. \
         Handles names, phone numbers, emails, SSNs, addresses, financial info, and more."
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let text = args.get_string("text")?;

        debug!("Sanitizing text ({} chars)", text.len());

        // We need a brain to do the sanitization
        let brain = args.brain.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed(
                "Sanitize tool requires a brain for PII detection".to_string(),
            )
        })?;

        // Create a message with our sanitization prompt
        let prompt = format!(
            "Sanitize the following text by replacing PII with placeholders. Output ONLY the sanitized text:\n\n{}",
            text
        );

        // Create a temporary inbound message for the brain
        // We use a special sender ID to avoid polluting real conversation history
        let message = InboundMessage::direct("__sanitize_tool__", &prompt, 0);

        // Process through the brain
        let response = brain.process(message).await.map_err(|e| {
            ToolError::BrainError(format!("Failed to process sanitization: {}", e))
        })?;

        let sanitized = response.text.trim().to_string();

        info!(
            "Sanitized text: {} chars -> {} chars",
            text.len(),
            sanitized.len()
        );

        debug!("Sanitized result: {}", sanitized);

        Ok(ToolOutput::success(sanitized))
    }
}

/// Get the system prompt for a sanitization brain.
///
/// Use this when configuring a MapleBrain specifically for sanitization.
pub fn sanitize_system_prompt() -> &'static str {
    SANITIZE_SYSTEM_PROMPT
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::Value;

    fn make_args(text: &str) -> ToolArgs {
        let mut params = HashMap::new();
        params.insert("text".to_string(), Value::String(text.to_string()));
        ToolArgs::new(params)
    }

    #[tokio::test]
    async fn test_missing_text() {
        let sanitize = Sanitize::new();
        let args = ToolArgs::new(HashMap::new());

        let result = sanitize.execute(args).await;
        assert!(matches!(result, Err(ToolError::MissingParameter(_))));
    }

    #[tokio::test]
    async fn test_missing_brain() {
        let sanitize = Sanitize::new();
        let args = make_args("My name is John");

        let result = sanitize.execute(args).await;
        assert!(matches!(result, Err(ToolError::ExecutionFailed(_))));
    }

    #[test]
    fn test_system_prompt_not_empty() {
        let prompt = sanitize_system_prompt();
        assert!(!prompt.is_empty());
        assert!(prompt.contains("PII"));
        assert!(prompt.contains("[NAME]"));
        assert!(prompt.contains("[SSN]"));
    }
}
