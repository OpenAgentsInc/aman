//! Main orchestrator that coordinates message processing.

use std::sync::Arc;

use brain_core::{InboundMessage, OutboundMessage, ToolExecutor, ToolRequest};
use grok_brain::GrokToolExecutor;
use maple_brain::{MapleBrain, MapleBrainConfig};
use tracing::{debug, info, warn};

use crate::actions::{OrchestratorAction, RoutingPlan};
use crate::context::Context;
use crate::error::OrchestratorError;
use crate::router::Router;
use crate::sender::MessageSender;

/// Help text shown when user asks for help.
pub const HELP_TEXT: &str = r#"I'm an AI assistant. Here's what I can do:

• Chat naturally about any topic
• Search for real-time information (news, prices, events)
• Remember our conversation context
• Clear our conversation history on request

Just send me a message and I'll do my best to help!"#;

/// Main orchestrator that coordinates message processing.
///
/// The orchestrator:
/// - Routes messages through maple-brain for classification
/// - Executes multiple tool calls and actions
/// - Sends interim status messages to the user
/// - Maintains typing indicators throughout processing
/// - Keeps all routing decisions private (via maple-brain TEE)
pub struct Orchestrator<S: MessageSender> {
    /// Router for message classification (stateless).
    router: Router,
    /// Brain for generating responses (stateful).
    brain: MapleBrain,
    /// Tool executor for real-time search.
    search: Arc<GrokToolExecutor>,
    /// Message sender for Signal or other transports.
    sender: S,
}

impl<S: MessageSender> Orchestrator<S> {
    /// Create a new orchestrator with the given components.
    pub fn new(
        router: Router,
        brain: MapleBrain,
        search: GrokToolExecutor,
        sender: S,
    ) -> Self {
        Self {
            router,
            brain,
            search: Arc::new(search),
            sender,
        }
    }

    /// Create an orchestrator from environment variables.
    ///
    /// This creates all components (router, brain, search) from environment.
    pub async fn from_env(sender: S) -> Result<Self, OrchestratorError> {
        // Create router (uses its own system prompt)
        let router = Router::from_env().await?;

        // Create brain for responses (with tool support)
        let brain_config = MapleBrainConfig::from_env()
            .map_err(|e| OrchestratorError::RoutingFailed(format!("Brain config error: {}", e)))?;

        // Create Grok tool executor (shared between orchestrator and brain)
        let search = Arc::new(
            GrokToolExecutor::from_env()
                .map_err(|e| OrchestratorError::ToolFailed(format!("Grok config error: {}", e)))?,
        );

        // Create brain with shared tool support
        let brain = MapleBrain::with_shared_tools(brain_config, search.clone())
            .await
            .map_err(|e| OrchestratorError::RoutingFailed(format!("Brain init error: {}", e)))?;

        Ok(Self {
            router,
            brain,
            search,
            sender,
        })
    }

    /// Create an orchestrator with a shared tool executor.
    pub async fn with_shared_tools(
        router: Router,
        brain_config: MapleBrainConfig,
        search: Arc<GrokToolExecutor>,
        sender: S,
    ) -> Result<Self, OrchestratorError> {
        let brain = MapleBrain::with_shared_tools(brain_config, search.clone())
            .await
            .map_err(|e| OrchestratorError::RoutingFailed(format!("Brain init error: {}", e)))?;

        Ok(Self {
            router,
            brain,
            search,
            sender,
        })
    }

    /// Get the history key for a message.
    ///
    /// Uses group ID for group messages, sender for direct messages.
    fn history_key(message: &InboundMessage) -> String {
        message
            .group_id
            .as_ref()
            .map(|g| format!("group:{}", g))
            .unwrap_or_else(|| message.sender.clone())
    }

    /// Process an incoming message end-to-end.
    ///
    /// This method:
    /// 1. Starts typing indicator
    /// 2. Gets conversation context (local, fast)
    /// 3. Routes the message with context to determine actions
    /// 4. Executes actions (search, clear context, etc.)
    /// 5. Generates and returns the final response
    pub async fn process(&self, message: InboundMessage) -> Result<OutboundMessage, OrchestratorError> {
        let recipient = message.group_id.as_ref().unwrap_or(&message.sender);
        let is_group = message.group_id.is_some();
        let history_key = Self::history_key(&message);

        info!(
            "Processing message from {} (group: {}, history_key: {})",
            message.sender, is_group, history_key
        );

        // 1. Start typing indicator
        if let Err(e) = self.sender.set_typing(recipient, is_group, true).await {
            warn!("Failed to start typing indicator: {}", e);
            // Continue processing even if typing indicator fails
        }

        // 2. Get conversation context (local operation, fast)
        let context = self.brain.get_context_summary(&history_key).await;
        if let Some(ref ctx) = context {
            debug!("Conversation context: {}", ctx);
        }

        // 3. Route the message with context
        let plan = self.router.route(&message.text, context.as_deref()).await;
        info!("Routing plan: {} actions", plan.actions.len());

        // 4. Execute actions, building context
        let result = self.execute_plan(&message, &plan, recipient, is_group, &history_key).await;

        // 5. Stop typing indicator (always, even on error)
        if let Err(e) = self.sender.set_typing(recipient, is_group, false).await {
            warn!("Failed to stop typing indicator: {}", e);
        }

        result
    }

    /// Execute the routing plan and return the final response.
    async fn execute_plan(
        &self,
        message: &InboundMessage,
        plan: &RoutingPlan,
        recipient: &str,
        is_group: bool,
        history_key: &str,
    ) -> Result<OutboundMessage, OrchestratorError> {
        let mut context = Context::new();

        for action in &plan.actions {
            match action {
                OrchestratorAction::Search { query, message: status_msg } => {
                    self.execute_search(query, status_msg.as_deref(), &mut context, recipient, is_group).await?;
                }

                OrchestratorAction::ClearContext { .. } => {
                    self.execute_clear_context(history_key).await?;
                }

                OrchestratorAction::Help => {
                    // Help returns immediately
                    return Ok(OutboundMessage::reply_to(message, HELP_TEXT));
                }

                OrchestratorAction::Skip { reason } => {
                    info!("Skipping message: {}", reason);
                    return Err(OrchestratorError::Skipped(reason.clone()));
                }

                OrchestratorAction::Ignore => {
                    // Silently ignore accidental messages
                    info!("Ignoring accidental message");
                    return Err(OrchestratorError::Skipped("accidental message".to_string()));
                }

                OrchestratorAction::Respond => {
                    // Final response with accumulated context
                    return self.execute_respond(message, &context).await;
                }
            }
        }

        // If no Respond action in plan, generate one anyway
        info!("No Respond action in plan, generating response anyway");
        self.execute_respond(message, &context).await
    }

    /// Execute a search action.
    async fn execute_search(
        &self,
        query: &str,
        status_message: Option<&str>,
        context: &mut Context,
        recipient: &str,
        is_group: bool,
    ) -> Result<(), OrchestratorError> {
        info!("Executing search: {}", query);

        // Notify user that we're searching (use custom message if provided)
        let search_msg = status_message
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Searching: {}", query));

        if let Err(e) = self.sender.send_message(recipient, &search_msg, is_group).await {
            warn!("Failed to send search notification: {}", e);
            // Continue with search even if notification fails
        }

        // Restart typing indicator after sending message
        if let Err(e) = self.sender.set_typing(recipient, is_group, true).await {
            warn!("Failed to restart typing indicator: {}", e);
        }

        // Execute the search
        let request = ToolRequest::from_call(
            "orchestrator-search".to_string(),
            "realtime_search".to_string(),
            &format!(r#"{{"query": "{}"}}"#, query.replace('"', "\\\"")),
        ).map_err(|e| OrchestratorError::ToolFailed(format!("Invalid search request: {}", e)))?;

        let result = self.search.execute(request).await;

        if result.success {
            info!("Search completed successfully ({} chars)", result.content.len());
            context.add_search_result(query, &result.content);
        } else {
            warn!("Search failed: {}", result.content);
            // Add error to context so brain knows search failed
            context.add_search_result(query, &format!("Search failed: {}", result.content));
        }

        Ok(())
    }

    /// Execute a clear context action (silent - no user notification).
    async fn execute_clear_context(&self, history_key: &str) -> Result<(), OrchestratorError> {
        info!("Clearing conversation history for {}", history_key);
        self.brain.clear_history(history_key).await;
        Ok(())
    }

    /// Execute a respond action - generate the final response.
    async fn execute_respond(
        &self,
        message: &InboundMessage,
        context: &Context,
    ) -> Result<OutboundMessage, OrchestratorError> {
        // Augment message with search context if any
        let augmented = context.augment_message(message);

        debug!("Context summary: {}", context.format_summary());

        // Process through brain
        use brain_core::Brain;
        let response = self.brain.process(augmented).await?;

        info!("Generated response: {} chars", response.text.len());
        Ok(response)
    }

    /// Get the sender.
    pub fn sender(&self) -> &S {
        &self.sender
    }

    /// Get the brain.
    pub fn brain(&self) -> &MapleBrain {
        &self.brain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sender::NoOpSender;

    // Note: Most tests require actual API keys and are integration tests.
    // These unit tests verify the basic structure.

    #[test]
    fn test_history_key_direct() {
        let message = InboundMessage::direct("+1234567890", "hello", 123);
        assert_eq!(Orchestrator::<NoOpSender>::history_key(&message), "+1234567890");
    }

    #[test]
    fn test_history_key_group() {
        let message = InboundMessage::group("+1234567890", "hello", 123, "group123");
        assert_eq!(Orchestrator::<NoOpSender>::history_key(&message), "group:group123");
    }

    #[test]
    fn test_help_text_not_empty() {
        assert!(!HELP_TEXT.is_empty());
        assert!(HELP_TEXT.contains("AI assistant"));
    }
}
