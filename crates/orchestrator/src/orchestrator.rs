//! Main orchestrator that coordinates message processing.

use std::sync::Arc;

use brain_core::{Brain, InboundMessage, OutboundMessage, ToolExecutor, ToolRequest};
use grok_brain::{GrokBrain, GrokBrainConfig, GrokToolExecutor};
use maple_brain::{MapleBrain, MapleBrainConfig};
use tracing::{debug, info, warn};

use crate::actions::{OrchestratorAction, RoutingPlan, Sensitivity, TaskHint, UserPreference};
use crate::context::Context;
use crate::error::OrchestratorError;
use crate::model_selection::ModelSelector;
use crate::preferences::{AgentIndicator, PreferenceStore};
use crate::router::Router;
use crate::sender::MessageSender;

/// Help text shown when user asks for help.
pub const HELP_TEXT: &str = r#"I'm an AI assistant with two modes:

Privacy Mode (Maple): Secure enclave processing for sensitive topics
Speed Mode (Grok): Fast responses with real-time search

Commands:
• "use grok" or "prefer speed" - Switch to speed mode
• "use maple" or "prefer privacy" - Switch to privacy mode
• "reset preferences" - Return to default (auto-detect)
• "grok: <query>" - One-time direct query to Grok
• "maple: <query>" - One-time direct query to Maple
• "forget our chat" - Clear conversation history

I automatically detect sensitive topics (health, finances, personal) and route them securely. General queries use fast mode for better real-time info.

Just send me a message and I'll do my best to help!"#;

/// Main orchestrator that coordinates message processing.
///
/// The orchestrator:
/// - Routes messages through maple-brain for classification
/// - Uses sensitivity-based routing (sensitive→Maple, insensitive→Grok)
/// - Respects user preferences for agent selection
/// - Executes multiple tool calls and actions
/// - Sends interim status messages to the user
/// - Maintains typing indicators throughout processing
/// - Keeps all routing decisions private (via maple-brain TEE)
pub struct Orchestrator<S: MessageSender> {
    /// Router for message classification (stateless, uses Maple).
    router: Router,
    /// Maple brain for sensitive responses (TEE, privacy-preserving).
    maple_brain: MapleBrain,
    /// Grok brain for insensitive responses (fast, has native search).
    grok_brain: GrokBrain,
    /// Tool executor for real-time search (used by Maple for tool calls).
    search: Arc<GrokToolExecutor>,
    /// Message sender for Signal or other transports.
    sender: S,
    /// User preference storage.
    preferences: PreferenceStore,
    /// Model selector for task-based model selection.
    model_selector: ModelSelector,
}

impl<S: MessageSender> Orchestrator<S> {
    /// Create a new orchestrator with the given components.
    pub fn new(
        router: Router,
        maple_brain: MapleBrain,
        grok_brain: GrokBrain,
        search: GrokToolExecutor,
        sender: S,
    ) -> Self {
        Self {
            router,
            maple_brain,
            grok_brain,
            search: Arc::new(search),
            sender,
            preferences: PreferenceStore::new(),
            model_selector: ModelSelector::default(),
        }
    }

    /// Create an orchestrator from environment variables.
    ///
    /// This creates all components (router, brains, search) from environment.
    pub async fn from_env(sender: S) -> Result<Self, OrchestratorError> {
        // Create router (uses its own system prompt)
        let router = Router::from_env().await?;

        // Create Maple brain config for responses (with tool support)
        let maple_config = MapleBrainConfig::from_env()
            .map_err(|e| OrchestratorError::RoutingFailed(format!("Maple config error: {}", e)))?;

        // Create Grok brain config for responses
        let grok_config = GrokBrainConfig::from_env()
            .map_err(|e| OrchestratorError::ToolFailed(format!("Grok config error: {}", e)))?;

        // Create Grok tool executor (shared for search operations)
        let search = Arc::new(
            GrokToolExecutor::from_env()
                .map_err(|e| OrchestratorError::ToolFailed(format!("Grok executor error: {}", e)))?,
        );

        // Create Maple brain with shared tool support
        let maple_brain = MapleBrain::with_shared_tools(maple_config, search.clone())
            .await
            .map_err(|e| OrchestratorError::RoutingFailed(format!("Maple init error: {}", e)))?;

        // Create Grok brain for direct queries
        let grok_brain = GrokBrain::new(grok_config)
            .map_err(|e| OrchestratorError::ToolFailed(format!("Grok brain init error: {}", e)))?;

        // Create model selector from environment
        let model_selector = ModelSelector::from_env();

        Ok(Self {
            router,
            maple_brain,
            grok_brain,
            search,
            sender,
            preferences: PreferenceStore::new(),
            model_selector,
        })
    }

    /// Create an orchestrator with shared components.
    pub async fn with_shared_tools(
        router: Router,
        maple_config: MapleBrainConfig,
        grok_config: GrokBrainConfig,
        search: Arc<GrokToolExecutor>,
        sender: S,
    ) -> Result<Self, OrchestratorError> {
        let maple_brain = MapleBrain::with_shared_tools(maple_config, search.clone())
            .await
            .map_err(|e| OrchestratorError::RoutingFailed(format!("Maple init error: {}", e)))?;

        let grok_brain = GrokBrain::new(grok_config)
            .map_err(|e| OrchestratorError::ToolFailed(format!("Grok brain init error: {}", e)))?;

        Ok(Self {
            router,
            maple_brain,
            grok_brain,
            search,
            sender,
            preferences: PreferenceStore::new(),
            model_selector: ModelSelector::from_env(),
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
    pub async fn process(
        &self,
        message: InboundMessage,
    ) -> Result<OutboundMessage, OrchestratorError> {
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
        }

        // 2. Get conversation context (local operation, fast)
        let context = self.maple_brain.get_context_summary(&history_key).await;
        if let Some(ref ctx) = context {
            debug!("Conversation context: {}", ctx);
        }

        // 3. Route the message with context
        let plan = self.router.route(&message.text, context.as_deref()).await;
        info!("Routing plan: {} actions", plan.actions.len());

        // 4. Execute actions, building context
        let result = self
            .execute_plan(&message, &plan, recipient, is_group, &history_key)
            .await;

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
                OrchestratorAction::Search {
                    query,
                    message: status_msg,
                } => {
                    self.execute_search(
                        query,
                        status_msg.as_deref(),
                        &mut context,
                        recipient,
                        is_group,
                    )
                    .await?;
                }

                OrchestratorAction::ClearContext { .. } => {
                    self.execute_clear_context(history_key).await?;
                }

                OrchestratorAction::Help => {
                    return Ok(OutboundMessage::reply_to(message, HELP_TEXT));
                }

                OrchestratorAction::Respond {
                    sensitivity,
                    task_hint,
                } => {
                    return self
                        .execute_respond(message, &context, *sensitivity, *task_hint, history_key)
                        .await;
                }

                OrchestratorAction::Grok { query, task_hint } => {
                    return self
                        .execute_direct_grok(message, query, &context, *task_hint)
                        .await;
                }

                OrchestratorAction::Maple { query, task_hint } => {
                    return self
                        .execute_direct_maple(message, query, &context, *task_hint, history_key)
                        .await;
                }

                OrchestratorAction::SetPreference { preference } => {
                    return self
                        .execute_set_preference(message, preference, history_key)
                        .await;
                }

                OrchestratorAction::Skip { reason } => {
                    info!("Skipping message: {}", reason);
                    return Err(OrchestratorError::Skipped(reason.clone()));
                }

                OrchestratorAction::Ignore => {
                    info!("Ignoring accidental message");
                    return Err(OrchestratorError::Skipped("accidental message".to_string()));
                }
            }
        }

        // If no Respond action in plan, generate one with default sensitivity and task hint
        info!("No response action in plan, generating response anyway");
        self.execute_respond(
            message,
            &context,
            Sensitivity::default(),
            TaskHint::default(),
            history_key,
        )
        .await
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

        // Notify user that we're searching
        let search_msg = status_message
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Searching: {}", query));

        if let Err(e) = self
            .sender
            .send_message(recipient, &search_msg, is_group)
            .await
        {
            warn!("Failed to send search notification: {}", e);
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
        )
        .map_err(|e| OrchestratorError::ToolFailed(format!("Invalid search request: {}", e)))?;

        let result = self.search.execute(request).await;

        if result.success {
            info!(
                "Search completed successfully ({} chars)",
                result.content.len()
            );
            context.add_search_result(query, &result.content);
        } else {
            warn!("Search failed: {}", result.content);
            context.add_search_result(query, &format!("Search failed: {}", result.content));
        }

        Ok(())
    }

    /// Execute a clear context action (silent - no user notification).
    async fn execute_clear_context(&self, history_key: &str) -> Result<(), OrchestratorError> {
        info!("Clearing conversation history for {}", history_key);
        self.maple_brain.clear_history(history_key).await;
        self.grok_brain.clear_history(history_key).await;
        Ok(())
    }

    /// Execute a respond action - generate the final response using sensitivity-based routing.
    async fn execute_respond(
        &self,
        message: &InboundMessage,
        context: &Context,
        sensitivity: Sensitivity,
        task_hint: TaskHint,
        history_key: &str,
    ) -> Result<OutboundMessage, OrchestratorError> {
        // Determine which agent to use based on sensitivity and user preference
        let use_grok = self
            .preferences
            .should_use_grok(history_key, sensitivity)
            .await;

        let indicator = if use_grok {
            AgentIndicator::Speed
        } else {
            AgentIndicator::Privacy
        };

        // Select the best model based on task hint
        let selected_model = if use_grok {
            self.model_selector.select_grok(task_hint)
        } else {
            self.model_selector.select_maple(task_hint)
        };

        info!(
            "Generating response with {:?} (sensitivity: {:?}, task_hint: {:?}, model: {}, use_grok: {})",
            indicator, sensitivity, task_hint, selected_model, use_grok
        );

        // Augment message with search context if any
        let augmented = context.augment_message(message);

        debug!("Context summary: {}", context.format_summary());

        // Process through the appropriate brain
        // Note: Currently using the default model configured in the brain.
        // TODO: Add per-request model override support to brains for dynamic model selection.
        let mut response = if use_grok {
            self.grok_brain.process(augmented).await?
        } else {
            self.maple_brain.process(augmented).await?
        };

        // Add indicator prefix if using speed mode
        if indicator == AgentIndicator::Speed && !indicator.prefix().is_empty() {
            response.text = format!("{}{}", indicator.prefix(), response.text);
        }

        info!("Generated response: {} chars", response.text.len());
        Ok(response)
    }

    /// Execute a direct Grok query (user explicitly requested).
    async fn execute_direct_grok(
        &self,
        message: &InboundMessage,
        query: &str,
        context: &Context,
        task_hint: TaskHint,
    ) -> Result<OutboundMessage, OrchestratorError> {
        // Select the best model based on task hint
        let selected_model = self.model_selector.select_grok(task_hint);

        info!(
            "Direct Grok query (task_hint: {:?}, model: {}): {}",
            task_hint, selected_model, query
        );

        // Create a modified message with the extracted query
        let mut modified = message.clone();
        modified.text = query.to_string();

        // Augment with context if any
        let augmented = context.augment_message(&modified);

        // Process through Grok
        // Note: Currently using the default model configured in the brain.
        // TODO: Add per-request model override support for dynamic model selection.
        let mut response = self.grok_brain.process(augmented).await?;

        // Add speed indicator
        let indicator = AgentIndicator::Speed;
        if !indicator.prefix().is_empty() {
            response.text = format!("{}{}", indicator.prefix(), response.text);
        }

        info!("Direct Grok response: {} chars", response.text.len());
        Ok(response)
    }

    /// Execute a direct Maple query (user explicitly requested).
    async fn execute_direct_maple(
        &self,
        message: &InboundMessage,
        query: &str,
        context: &Context,
        task_hint: TaskHint,
        _history_key: &str,
    ) -> Result<OutboundMessage, OrchestratorError> {
        // Select the best model based on task hint
        let selected_model = self.model_selector.select_maple(task_hint);

        info!(
            "Direct Maple query (task_hint: {:?}, model: {}): {}",
            task_hint, selected_model, query
        );

        // Create a modified message with the extracted query
        let mut modified = message.clone();
        modified.text = query.to_string();

        // Augment with context if any
        let augmented = context.augment_message(&modified);

        // Process through Maple
        // Note: Currently using the default model configured in the brain.
        // TODO: Add per-request model override support for dynamic model selection.
        let response = self.maple_brain.process(augmented).await?;

        info!("Direct Maple response: {} chars", response.text.len());
        Ok(response)
    }

    /// Execute a set preference action.
    async fn execute_set_preference(
        &self,
        message: &InboundMessage,
        preference_str: &str,
        history_key: &str,
    ) -> Result<OutboundMessage, OrchestratorError> {
        let preference = UserPreference::from_str(preference_str);

        info!(
            "Setting preference for {} to {:?}",
            history_key, preference
        );

        self.preferences.set(history_key, preference).await;

        let response_text = match preference {
            UserPreference::PreferSpeed => {
                "Switched to speed mode. I'll use fast processing for your requests. \
                 Sensitive topics will still be handled securely.\n\n\
                 Say \"use maple\" or \"prefer privacy\" to switch back."
            }
            UserPreference::PreferPrivacy => {
                "Switched to privacy mode. All your requests will be processed in the secure enclave.\n\n\
                 Say \"use grok\" or \"prefer speed\" for faster responses."
            }
            UserPreference::Default => {
                "Preferences reset to default. I'll automatically detect sensitive topics \
                 and route them securely, while using fast mode for general queries.\n\n\
                 Say \"use grok\" for speed mode or \"use maple\" for privacy mode."
            }
        };

        Ok(OutboundMessage::reply_to(message, response_text))
    }

    /// Get the sender.
    pub fn sender(&self) -> &S {
        &self.sender
    }

    /// Get the Maple brain.
    pub fn maple_brain(&self) -> &MapleBrain {
        &self.maple_brain
    }

    /// Get the Grok brain.
    pub fn grok_brain(&self) -> &GrokBrain {
        &self.grok_brain
    }

    /// Get the preference store.
    pub fn preferences(&self) -> &PreferenceStore {
        &self.preferences
    }

    /// Get the model selector.
    pub fn model_selector(&self) -> &ModelSelector {
        &self.model_selector
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sender::NoOpSender;

    #[test]
    fn test_history_key_direct() {
        let message = InboundMessage::direct("+1234567890", "hello", 123);
        assert_eq!(
            Orchestrator::<NoOpSender>::history_key(&message),
            "+1234567890"
        );
    }

    #[test]
    fn test_history_key_group() {
        let message = InboundMessage::group("+1234567890", "hello", 123, "group123");
        assert_eq!(
            Orchestrator::<NoOpSender>::history_key(&message),
            "group:group123"
        );
    }

    #[test]
    fn test_help_text_not_empty() {
        assert!(!HELP_TEXT.is_empty());
        assert!(HELP_TEXT.contains("Privacy Mode"));
        assert!(HELP_TEXT.contains("Speed Mode"));
        assert!(HELP_TEXT.contains("use grok"));
        assert!(HELP_TEXT.contains("use maple"));
    }
}
