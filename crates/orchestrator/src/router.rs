//! Message routing using MapleBrain.

use brain_core::{Brain, InboundMessage};
use maple_brain::{MapleBrain, MapleBrainConfig};
use tracing::{debug, info, warn};

use crate::actions::RoutingPlan;
#[cfg(test)]
use crate::actions::OrchestratorAction;
use crate::error::OrchestratorError;

/// System prompt for the router.
///
/// This prompt instructs the model to analyze messages and return
/// a JSON routing plan with personalized status messages.
pub const ROUTER_SYSTEM_PROMPT: &str = r#"You are a message routing assistant. Analyze the user's message and determine what actions are needed.

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

/// Router that uses MapleBrain to analyze messages and determine actions.
///
/// The router is stateless - it doesn't maintain conversation history
/// and only makes a single call to determine the routing plan.
pub struct Router {
    brain: MapleBrain,
}

impl Router {
    /// Create a new router with the given MapleBrain configuration.
    ///
    /// The router uses its own system prompt and disables history.
    pub async fn new(mut config: MapleBrainConfig) -> Result<Self, OrchestratorError> {
        // Override config for routing
        config.system_prompt = Some(ROUTER_SYSTEM_PROMPT.to_string());
        config.max_history_turns = 0; // Stateless
        config.temperature = Some(0.0); // Deterministic
        config.max_tokens = Some(256); // Routing plans are small

        let brain = MapleBrain::new(config)
            .await
            .map_err(|e| OrchestratorError::RoutingFailed(format!("Failed to initialize router brain: {}", e)))?;

        Ok(Self { brain })
    }

    /// Create a router from environment variables.
    pub async fn from_env() -> Result<Self, OrchestratorError> {
        let config = MapleBrainConfig::from_env()
            .map_err(|e| OrchestratorError::RoutingFailed(format!("Config error: {}", e)))?;
        Self::new(config).await
    }

    /// Route a message and return the routing plan.
    ///
    /// Returns a default plan (respond only) if routing fails or produces
    /// invalid output.
    pub async fn route(&self, message_text: &str, context: Option<&str>) -> RoutingPlan {
        // Format the input with context if available
        let formatted_input = Self::format_router_input(message_text, context);

        // Create a minimal inbound message for the brain
        let inbound = InboundMessage::direct("router", &formatted_input, 0);

        match self.brain.process(inbound).await {
            Ok(response) => {
                debug!("Router response: {}", response.text);
                self.parse_plan(&response.text)
            }
            Err(e) => {
                warn!("Router brain error: {}, using default plan", e);
                RoutingPlan::respond_only()
            }
        }
    }

    /// Format the input for the router with optional context.
    fn format_router_input(message: &str, context: Option<&str>) -> String {
        match context {
            Some(ctx) if !ctx.is_empty() => {
                format!("[CONTEXT: {}]\n[MESSAGE: {}]", ctx, message)
            }
            _ => {
                format!("[MESSAGE: {}]", message)
            }
        }
    }

    /// Parse the routing plan from the brain's response.
    fn parse_plan(&self, response: &str) -> RoutingPlan {
        // Try to extract JSON from the response
        let json_str = self.extract_json(response);

        match serde_json::from_str::<RoutingPlan>(json_str) {
            Ok(plan) => {
                if plan.is_empty() {
                    info!("Empty routing plan, using default");
                    RoutingPlan::respond_only()
                } else {
                    info!("Parsed routing plan with {} actions", plan.actions.len());
                    for action in &plan.actions {
                        debug!("  - {}", action.description());
                    }
                    plan
                }
            }
            Err(e) => {
                warn!("Failed to parse routing plan: {}, response was: {}", e, response);
                RoutingPlan::respond_only()
            }
        }
    }

    /// Extract JSON from a response that may contain markdown or other text.
    fn extract_json<'a>(&self, response: &'a str) -> &'a str {
        let trimmed = response.trim();

        // If it starts with {, assume it's JSON
        if trimmed.starts_with('{') {
            return trimmed;
        }

        // Try to find JSON in markdown code block
        if let Some(start) = trimmed.find("```json") {
            let json_start = start + 7;
            if let Some(end) = trimmed[json_start..].find("```") {
                return trimmed[json_start..json_start + end].trim();
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
                return after_backticks[json_start..json_start + end].trim();
            }
        }

        // Try to find a JSON object in the text
        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                return &trimmed[start..=end];
            }
        }

        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to extract JSON, matching Router's logic.
    fn extract_json(response: &str) -> &str {
        let trimmed = response.trim();

        // If it starts with {, assume it's JSON
        if trimmed.starts_with('{') {
            return trimmed;
        }

        // Try to find JSON in markdown code block
        if let Some(start) = trimmed.find("```json") {
            let json_start = start + 7;
            if let Some(end) = trimmed[json_start..].find("```") {
                return trimmed[json_start..json_start + end].trim();
            }
        }

        // Try to find JSON in generic code block
        if let Some(start) = trimmed.find("```") {
            let after_backticks = &trimmed[start + 3..];
            let json_start = after_backticks.find('\n').map(|i| i + 1).unwrap_or(0);
            if let Some(end) = after_backticks[json_start..].find("```") {
                return after_backticks[json_start..json_start + end].trim();
            }
        }

        // Try to find a JSON object in the text
        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                return &trimmed[start..=end];
            }
        }

        trimmed
    }

    #[test]
    fn test_extract_json_plain() {
        let result = extract_json(r#"{"actions": [{"type": "respond"}]}"#);
        assert!(result.starts_with('{'));
    }

    #[test]
    fn test_extract_json_code_block() {
        let input = "Here's the plan:\n```json\n{\"actions\": [{\"type\": \"respond\"}]}\n```";
        let result = extract_json(input);
        assert!(result.starts_with('{'));
        assert!(result.contains("respond"));
    }

    #[test]
    fn test_extract_json_with_text() {
        let input = "Let me analyze this. {\"actions\": [{\"type\": \"respond\"}]} That's my plan.";
        let result = extract_json(input);
        assert!(result.starts_with('{'));
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
        assert!(matches!(plan.actions[3], OrchestratorAction::Respond));
        assert!(matches!(plan.actions[4], OrchestratorAction::Skip { .. }));
        assert!(matches!(plan.actions[5], OrchestratorAction::Ignore));
    }

    #[test]
    fn test_format_router_input_no_context() {
        let input = Router::format_router_input("hello world", None);
        assert_eq!(input, "[MESSAGE: hello world]");
    }

    #[test]
    fn test_format_router_input_empty_context() {
        let input = Router::format_router_input("hello world", Some(""));
        assert_eq!(input, "[MESSAGE: hello world]");
    }

    #[test]
    fn test_format_router_input_with_context() {
        let input = Router::format_router_input("what's bitcoin price?", Some("discussing Minnesota politics"));
        assert_eq!(input, "[CONTEXT: discussing Minnesota politics]\n[MESSAGE: what's bitcoin price?]");
    }
}
