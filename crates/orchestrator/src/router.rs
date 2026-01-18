//! Message routing using MapleBrain.

use brain_core::{Brain, InboundAttachment, InboundMessage};
use maple_brain::{MapleBrain, MapleBrainConfig};
use std::env;
use std::path::Path;
use tracing::{debug, info, trace, warn};

use brain_core::{hash_prompt, Sensitivity, TaskHint};
use crate::actions::RoutingPlan;
use crate::error::OrchestratorError;

/// Default path for the router prompt file.
pub const DEFAULT_ROUTER_PROMPT_FILE: &str = "ROUTER_PROMPT.md";

/// Default router system prompt (fallback if file not found).
///
/// This prompt instructs the model to analyze messages and return
/// a JSON routing plan with personalized status messages.
pub const DEFAULT_ROUTER_SYSTEM_PROMPT: &str = r#"You are a message routing assistant. Analyze the user's message and determine what actions are needed.

Output JSON with an "actions" array. Each action has a "type" field.

Available action types:
- "search": Real-time search needed. Include "query" field with privacy-safe search terms. Include "message" field with a short, friendly one-liner status update.
- "clear_context": Clear conversation history. Use this AUTOMATICALLY when the user's new message is about a completely different topic from the recent context. No user notification needed.
- "help": User is asking about bot capabilities or commands.
- "respond": Generate a response to the user (should usually be last).
- "skip": Don't process (e.g., message not meant for bot). Include "reason" field.
- "ignore": Silently ignore accidental messages (typos, "?", ".", stray characters, or messages that seem sent by mistake).

Guidelines:
- Most messages need just: [{"type": "respond"}]
- Current events/news need: [{"type": "search", ...}, {"type": "respond"}]
- Explicit "forget our chat" needs: [{"type": "clear_context"}, {"type": "respond"}]
- "what can you do" needs: [{"type": "help"}]
- Accidental messages like "?", ".", "k", single random characters: [{"type": "ignore"}]
- TOPIC CHANGE: If recent context exists and the new message is about a COMPLETELY DIFFERENT topic, add clear_context BEFORE respond. Example: context is about "Minnesota politics" but user asks about "bitcoin price" → clear first.
- Multiple actions can be combined

For "message" fields on search, write short, natural one-liners (under 50 chars). Be friendly and conversational.

The input format is:
[CONTEXT: recent conversation topics, if any]
[MESSAGE: the user's new message]

Examples:

[MESSAGE: tell me a joke]
→ {"actions": [{"type": "respond"}]}

[MESSAGE: ?]
→ {"actions": [{"type": "ignore"}]}

[CONTEXT: discussing weather in NYC | planning weekend trip]
[MESSAGE: what about Saturday?]
→ {"actions": [{"type": "respond"}]}

[CONTEXT: discussing Minnesota election results | governor race]
[MESSAGE: what's the bitcoin price?]
→ {"actions": [{"type": "clear_context"}, {"type": "search", "query": "bitcoin price USD", "message": "Let me check..."}, {"type": "respond"}]}

[CONTEXT: chatting about cooking recipes]
[MESSAGE: who won the Super Bowl?]
→ {"actions": [{"type": "clear_context"}, {"type": "search", "query": "Super Bowl winner 2024", "message": "Looking that up..."}, {"type": "respond"}]}

[MESSAGE: what's the weather in NYC?]
→ {"actions": [{"type": "search", "query": "weather New York City", "message": "Checking the forecast..."}, {"type": "respond"}]}

[MESSAGE: forget our conversation]
→ {"actions": [{"type": "clear_context"}, {"type": "respond"}]}

Respond with JSON only. No explanation."#;

/// Load the router system prompt.
///
/// Priority:
/// 1. `ROUTER_SYSTEM_PROMPT` env var (if set)
/// 2. Contents of prompt file (`ROUTER_PROMPT_FILE` or default `ROUTER_PROMPT.md`)
/// 3. Embedded default prompt
pub fn load_router_prompt() -> String {
    // 1. Check for inline env var
    if let Ok(prompt) = env::var("ROUTER_SYSTEM_PROMPT") {
        info!("Using router prompt from ROUTER_SYSTEM_PROMPT env var");
        return prompt;
    }

    // 2. Try to load from file
    let prompt_file = env::var("ROUTER_PROMPT_FILE")
        .unwrap_or_else(|_| DEFAULT_ROUTER_PROMPT_FILE.to_string());

    if let Some(prompt) = load_prompt_file(&prompt_file) {
        info!("Loaded router prompt from {}", prompt_file);
        return prompt;
    }

    // 3. Fall back to embedded default
    info!("Using embedded default router prompt");
    DEFAULT_ROUTER_SYSTEM_PROMPT.to_string()
}

/// Load a prompt from a file path.
///
/// Returns `Some(content)` if the file exists and is readable, `None` otherwise.
fn load_prompt_file(path: impl AsRef<Path>) -> Option<String> {
    let path = path.as_ref();

    match std::fs::read_to_string(path) {
        Ok(content) => {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Err(_) => None,
    }
}

/// Router that uses MapleBrain to analyze messages and determine actions.
///
/// The router is stateless - it doesn't maintain conversation history
/// and only makes a single call to determine the routing plan.
pub struct Router {
    brain: MapleBrain,
    prompt_hash: String,
}

impl Router {
    /// Create a new router with the given MapleBrain configuration.
    ///
    /// The router uses its own system prompt and disables history.
    /// Prompt is loaded from file or env var (see `load_router_prompt`).
    pub async fn new(mut config: MapleBrainConfig) -> Result<Self, OrchestratorError> {
        // Override config for routing
        let prompt = load_router_prompt();
        let prompt_hash = hash_prompt(&prompt);
        config.system_prompt = Some(prompt);
        config.max_history_turns = 0; // Stateless
        config.temperature = Some(0.0); // Deterministic
        config.max_tokens = Some(256); // Routing plans are small

        let brain = MapleBrain::new(config)
            .await
            .map_err(|e| OrchestratorError::RoutingFailed(format!("Failed to initialize router brain: {}", e)))?;

        info!("Router prompt fingerprint: {}", prompt_hash);

        Ok(Self { brain, prompt_hash })
    }

    /// Get the router prompt fingerprint.
    pub fn prompt_hash(&self) -> &str {
        &self.prompt_hash
    }

    /// Create a router from environment variables.
    pub async fn from_env() -> Result<Self, OrchestratorError> {
        let config = MapleBrainConfig::from_env()
            .map_err(|e| OrchestratorError::RoutingFailed(format!("Config error: {}", e)))?;
        Self::new(config).await
    }

    /// Route a message and return the routing plan.
    ///
    /// Returns a safe fallback plan (respond-only, Maple, vision-safe) if routing
    /// fails or produces invalid output.
    pub async fn route(&self, message_text: &str, context: Option<&str>) -> RoutingPlan {
        self.route_with_attachments(message_text, context, &[]).await
    }

    /// Route a message with attachments and return the routing plan.
    ///
    /// Returns a safe fallback plan (respond-only, Maple, vision-safe) if routing
    /// fails or produces invalid output.
    pub async fn route_with_attachments(
        &self,
        message_text: &str,
        context: Option<&str>,
        attachments: &[InboundAttachment],
    ) -> RoutingPlan {
        // Format the input with context and attachments if available
        let formatted_input = Self::format_router_input(message_text, context, attachments);

        // Log the router input for debugging
        trace!(
            message_text = %message_text,
            context = ?context,
            attachments_count = attachments.len(),
            formatted_input = %formatted_input,
            "ROUTER_INPUT"
        );

        // Create a minimal inbound message for the brain
        let inbound = InboundMessage::direct("router", &formatted_input, 0);

        let fallback = Self::fallback_plan(attachments);

        match self.brain.process(inbound).await {
            Ok(response) => {
                // Log full router response for debugging
                trace!(
                    raw_response = %response.text,
                    response_len = response.text.len(),
                    "ROUTER_RAW_RESPONSE"
                );
                debug!("Router response: {}", response.text);
                match self.parse_plan(&response.text) {
                    Ok(plan) => {
                        trace!(
                            actions_count = plan.actions.len(),
                            parsed_plan = ?plan,
                            "ROUTER_PARSED_PLAN"
                        );
                        plan
                    }
                    Err(e) => {
                        warn!(
                            error = %e,
                            raw_response = %response.text,
                            "ROUTER_PARSE_FAILED"
                        );
                        fallback
                    }
                }
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "ROUTER_BRAIN_ERROR"
                );
                fallback
            }
        }
    }

    /// Format the input for the router with optional context and attachments.
    pub fn format_router_input(
        message: &str,
        context: Option<&str>,
        attachments: &[InboundAttachment],
    ) -> String {
        let mut parts = Vec::new();

        // Add context if available
        if let Some(ctx) = context {
            if !ctx.is_empty() {
                parts.push(format!("[CONTEXT: {}]", ctx));
            }
        }

        // Add message
        parts.push(format!("[MESSAGE: {}]", message));

        // Add attachments description
        let attachments_desc = Self::format_attachments(attachments);
        parts.push(format!("[ATTACHMENTS: {}]", attachments_desc));

        parts.join("\n")
    }

    /// Format attachments into a human-readable description.
    pub fn format_attachments(attachments: &[InboundAttachment]) -> String {
        if attachments.is_empty() {
            return "none".to_string();
        }

        let image_count = attachments.iter().filter(|a| a.is_image()).count();
        let video_count = attachments.iter().filter(|a| a.is_video()).count();
        let audio_count = attachments.iter().filter(|a| a.is_audio()).count();
        let other_count = attachments.len() - image_count - video_count - audio_count;

        let mut descriptions = Vec::new();

        // Describe images with details
        if image_count > 0 {
            let image_details: Vec<String> = attachments
                .iter()
                .filter(|a| a.is_image())
                .map(|a| {
                    let ext = a
                        .content_type
                        .strip_prefix("image/")
                        .unwrap_or("unknown");
                    let size = match (a.width, a.height) {
                        (Some(w), Some(h)) => format!(", {}x{}", w, h),
                        _ => String::new(),
                    };
                    format!("({}{})", ext, size)
                })
                .collect();

            if image_count == 1 {
                descriptions.push(format!("1 image {}", image_details[0]));
            } else {
                descriptions.push(format!("{} images {}", image_count, image_details.join(", ")));
            }
        }

        // Describe videos
        if video_count > 0 {
            descriptions.push(format!(
                "{} video{}",
                video_count,
                if video_count > 1 { "s" } else { "" }
            ));
        }

        // Describe audio
        if audio_count > 0 {
            descriptions.push(format!(
                "{} audio file{}",
                audio_count,
                if audio_count > 1 { "s" } else { "" }
            ));
        }

        // Describe other files
        if other_count > 0 {
            descriptions.push(format!(
                "{} other file{}",
                other_count,
                if other_count > 1 { "s" } else { "" }
            ));
        }

        descriptions.join(", ")
    }

    /// Parse the routing plan from the brain's response.
    fn parse_plan(&self, response: &str) -> Result<RoutingPlan, OrchestratorError> {
        // Try to extract JSON from the response
        let json_str = self.extract_json(response);

        let plan = serde_json::from_str::<RoutingPlan>(json_str).map_err(|e| {
            OrchestratorError::InvalidPlan(format!(
                "parse error: {}, response was: {}",
                e, response
            ))
        })?;

        if plan.is_empty() {
            return Err(OrchestratorError::InvalidPlan(
                "empty routing plan".to_string(),
            ));
        }

        info!("Parsed routing plan with {} actions", plan.actions.len());
        for action in &plan.actions {
            debug!("  - {}", action.description());
        }

        Ok(plan)
    }

    /// Build a safe fallback plan when routing fails.
    fn fallback_plan(attachments: &[InboundAttachment]) -> RoutingPlan {
        let task_hint = if attachments.iter().any(|a| a.is_image()) {
            TaskHint::Vision
        } else {
            TaskHint::General
        };

        // Fail closed to Maple by marking sensitivity as sensitive.
        RoutingPlan::respond_with_hint(Sensitivity::Sensitive, task_hint)
    }

    /// Extract JSON from a response that may contain markdown or other text.
    fn extract_json<'a>(&self, response: &'a str) -> &'a str {
        let trimmed = response.trim();

        // If it starts with {, extract balanced JSON object
        if trimmed.starts_with('{') {
            return Self::extract_balanced_json(trimmed);
        }

        // Try to find JSON in markdown code block
        if let Some(start) = trimmed.find("```json") {
            let json_start = start + 7;
            if let Some(end) = trimmed[json_start..].find("```") {
                let extracted = trimmed[json_start..json_start + end].trim();
                return Self::extract_balanced_json(extracted);
            }
        }

        // Try to find JSON in generic code block
        if let Some(start) = trimmed.find("```") {
            let after_backticks = &trimmed[start + 3..];
            // Skip optional language identifier
            let json_start = after_backticks
                .find('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            if let Some(end) = after_backticks[json_start..].find("```") {
                let extracted = after_backticks[json_start..json_start + end].trim();
                return Self::extract_balanced_json(extracted);
            }
        }

        // Try to find a JSON object in the text
        if let Some(start) = trimmed.find('{') {
            return Self::extract_balanced_json(&trimmed[start..]);
        }

        trimmed
    }

    /// Extract a balanced JSON object from a string that starts with '{'.
    ///
    /// This handles cases where the LLM adds trailing characters like extra braces.
    /// For example: `{"actions": [...]}}}` -> `{"actions": [...]}`
    fn extract_balanced_json(s: &str) -> &str {
        if !s.starts_with('{') {
            return s;
        }

        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for (i, ch) in s.char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match ch {
                '\\' if in_string => {
                    escape_next = true;
                }
                '"' => {
                    in_string = !in_string;
                }
                '{' if !in_string => {
                    depth += 1;
                }
                '}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        // Found the matching closing brace
                        return &s[..=i];
                    }
                }
                _ => {}
            }
        }

        // If we didn't find balanced braces, return the original
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::OrchestratorAction;
    use brain_core::InboundAttachment;

    #[test]
    fn test_extract_balanced_json_clean() {
        let input = r#"{"actions": [{"type": "respond"}]}"#;
        let result = Router::extract_balanced_json(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_extract_balanced_json_trailing_braces() {
        // This is the actual bug we observed in production
        let input = r#"{"actions": [{"type": "use_tool", "name": "bitcoin_price"}]}}"#;
        let result = Router::extract_balanced_json(input);
        assert_eq!(result, r#"{"actions": [{"type": "use_tool", "name": "bitcoin_price"}]}"#);
    }

    #[test]
    fn test_extract_balanced_json_multiple_trailing() {
        let input = r#"{"actions": [{"type": "respond"}]}}}}}"#;
        let result = Router::extract_balanced_json(input);
        assert_eq!(result, r#"{"actions": [{"type": "respond"}]}"#);
    }

    #[test]
    fn test_extract_balanced_json_with_strings() {
        // Ensure braces inside strings don't confuse the parser
        let input = r#"{"message": "Hello { world }", "nested": {"key": "value"}}"#;
        let result = Router::extract_balanced_json(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_extract_balanced_json_with_escaped_quotes() {
        let input = r#"{"message": "He said \"hello\"", "done": true}"#;
        let result = Router::extract_balanced_json(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_extract_json_plain() {
        let input = r#"{"actions": [{"type": "respond"}]}"#;
        let result = Router::extract_balanced_json(input);
        assert!(result.starts_with('{'));
        assert!(result.ends_with('}'));
    }

    #[test]
    fn test_extract_json_with_trailing_text() {
        let input = r#"{"actions": [{"type": "respond"}]} some trailing text"#;
        let result = Router::extract_balanced_json(input);
        assert_eq!(result, r#"{"actions": [{"type": "respond"}]}"#);
    }

    #[test]
    fn test_parse_plan_valid() {
        let json = r#"{"actions": [{"type": "search", "query": "test"}, {"type": "respond"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();

        assert_eq!(plan.actions.len(), 2);
        assert!(plan.has_search());
    }

    #[test]
    fn test_parse_plan_all_actions() {
        let json = r#"{
            "actions": [
                {"type": "clear_context", "message": "Fresh start!"},
                {"type": "search", "query": "news", "message": "On it!"},
                {"type": "help"},
                {"type": "respond"},
                {"type": "skip", "reason": "test"},
                {"type": "ignore"}
            ]
        }"#;

        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert_eq!(plan.actions.len(), 6);

        assert!(matches!(plan.actions[0], OrchestratorAction::ClearContext { .. }));
        assert!(matches!(plan.actions[1], OrchestratorAction::Search { .. }));
        assert!(matches!(plan.actions[2], OrchestratorAction::Help));
        assert!(matches!(plan.actions[3], OrchestratorAction::Respond { .. }));
        assert!(matches!(plan.actions[4], OrchestratorAction::Skip { .. }));
        assert!(matches!(plan.actions[5], OrchestratorAction::Ignore));
    }

    #[test]
    fn test_format_router_input_no_context() {
        let input = Router::format_router_input("hello world", None, &[]);
        assert_eq!(input, "[MESSAGE: hello world]\n[ATTACHMENTS: none]");
    }

    #[test]
    fn test_format_router_input_empty_context() {
        let input = Router::format_router_input("hello world", Some(""), &[]);
        assert_eq!(input, "[MESSAGE: hello world]\n[ATTACHMENTS: none]");
    }

    #[test]
    fn test_format_router_input_with_context() {
        let input = Router::format_router_input("what's bitcoin price?", Some("discussing Minnesota politics"), &[]);
        assert_eq!(input, "[CONTEXT: discussing Minnesota politics]\n[MESSAGE: what's bitcoin price?]\n[ATTACHMENTS: none]");
    }

    #[test]
    fn test_format_attachments_none() {
        let desc = Router::format_attachments(&[]);
        assert_eq!(desc, "none");
    }

    #[test]
    fn test_format_attachments_single_image() {
        let attachments = vec![InboundAttachment {
            content_type: "image/jpeg".to_string(),
            file_path: Some("/tmp/test.jpg".to_string()),
            width: Some(1024),
            height: Some(768),
            ..Default::default()
        }];
        let desc = Router::format_attachments(&attachments);
        assert_eq!(desc, "1 image (jpeg, 1024x768)");
    }

    #[test]
    fn test_format_attachments_multiple_images() {
        let attachments = vec![
            InboundAttachment {
                content_type: "image/jpeg".to_string(),
                file_path: Some("/tmp/test1.jpg".to_string()),
                width: Some(1024),
                height: Some(768),
                ..Default::default()
            },
            InboundAttachment {
                content_type: "image/png".to_string(),
                file_path: Some("/tmp/test2.png".to_string()),
                width: Some(800),
                height: Some(600),
                ..Default::default()
            },
        ];
        let desc = Router::format_attachments(&attachments);
        assert_eq!(desc, "2 images (jpeg, 1024x768), (png, 800x600)");
    }

    #[test]
    fn test_format_router_input_with_image() {
        let attachments = vec![InboundAttachment {
            content_type: "image/jpeg".to_string(),
            file_path: Some("/tmp/test.jpg".to_string()),
            width: Some(1024),
            height: Some(768),
            ..Default::default()
        }];
        let input = Router::format_router_input("what is this?", None, &attachments);
        assert_eq!(input, "[MESSAGE: what is this?]\n[ATTACHMENTS: 1 image (jpeg, 1024x768)]");
    }

    #[test]
    fn test_fallback_plan_no_attachments() {
        let plan = Router::fallback_plan(&[]);
        assert_eq!(plan.actions.len(), 1);

        if let OrchestratorAction::Respond {
            sensitivity,
            task_hint,
            ..
        } = &plan.actions[0]
        {
            assert_eq!(*sensitivity, Sensitivity::Sensitive);
            assert_eq!(*task_hint, TaskHint::General);
        } else {
            panic!("Expected Respond action");
        }
    }

    #[test]
    fn test_fallback_plan_with_image() {
        let attachments = vec![InboundAttachment {
            content_type: "image/png".to_string(),
            ..Default::default()
        }];
        let plan = Router::fallback_plan(&attachments);
        assert_eq!(plan.actions.len(), 1);

        if let OrchestratorAction::Respond {
            sensitivity,
            task_hint,
            ..
        } = &plan.actions[0]
        {
            assert_eq!(*sensitivity, Sensitivity::Sensitive);
            assert_eq!(*task_hint, TaskHint::Vision);
        } else {
            panic!("Expected Respond action");
        }
    }
}
