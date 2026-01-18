//! Main orchestrator that coordinates message processing.

use std::collections::HashMap;
use std::env;
use std::sync::Arc;

use brain_core::{
    format_memory_prompt, hash_prompt, Brain, InboundMessage, OutboundMessage, ToolExecutor,
    ToolRequest,
};
use database::Database;
use grok_brain::{GrokBrain, GrokBrainConfig, GrokToolExecutor};
use maple_brain::{MapleBrain, MapleBrainConfig};
use proton_proxy::ProtonClient;
use serde_json::{json, Value};
use agent_tools::ToolRegistry;
use tracing::{debug, info, warn};

use brain_core::{Sensitivity, TaskHint};
use crate::actions::{OrchestratorAction, PrivacyChoice, RoutingPlan, UserPreference};
use crate::context::Context;
use crate::error::OrchestratorError;
use crate::formatting::format_with_footer;
use crate::memory::{MemorySettings, MemoryStore};
use crate::model_selection::ModelSelector;
use crate::preferences::{AgentIndicator, PreferenceStore};
use crate::profile::ProfileStore;
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

Profile Settings:
• "show my settings" - View your profile
• "set my email to X" - Update your email
• "set my default model to X" - Set preferred AI model
• "clear my email" - Remove a setting
• "delete my profile" - Clear all settings

I automatically detect sensitive topics (health, finances, personal) and route them securely. General queries use fast mode for better real-time info.

Just send me a message and I'll do my best to help!"#;

#[derive(Debug, Default)]
struct MemoryContext {
    summary: Option<String>,
    prompt: Option<String>,
    summary_updated_at: Option<String>,
    source: Option<String>,
}

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
    maple_brain: Arc<MapleBrain>,
    /// Grok brain for insensitive responses (fast, has native search).
    grok_brain: GrokBrain,
    /// Tool executor for real-time search (used by Maple for tool calls).
    search: Arc<GrokToolExecutor>,
    /// Message sender for Signal or other transports.
    sender: S,
    /// User preference storage.
    preferences: PreferenceStore,
    /// Optional durable memory store.
    memory: Option<MemoryStore>,
    /// Model selector for task-based model selection.
    model_selector: ModelSelector,
    /// Tool registry for executing tools.
    tool_registry: ToolRegistry,
    /// Email client for sending attachments (optional).
    email_client: Option<ProtonClient>,
    /// User profile store for personal settings.
    profile: ProfileStore,
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
        let maple_brain = Arc::new(maple_brain);
        let mut tool_registry = agent_tools::default_registry();
        let brain: Arc<dyn Brain> = maple_brain.clone();
        tool_registry.set_brain(brain);

        Self {
            router,
            maple_brain,
            grok_brain,
            search: Arc::new(search),
            sender,
            preferences: PreferenceStore::new(),
            memory: None,
            model_selector: ModelSelector::default(),
            tool_registry,
            email_client: None,
            profile: ProfileStore::new(),
        }
    }

    /// Create a new orchestrator with a custom tool registry.
    pub fn with_tools(
        router: Router,
        maple_brain: MapleBrain,
        grok_brain: GrokBrain,
        search: GrokToolExecutor,
        sender: S,
        tool_registry: ToolRegistry,
    ) -> Self {
        let maple_brain = Arc::new(maple_brain);
        let mut tool_registry = tool_registry;
        let brain: Arc<dyn Brain> = maple_brain.clone();
        tool_registry.set_brain(brain);

        Self {
            router,
            maple_brain,
            grok_brain,
            search: Arc::new(search),
            sender,
            preferences: PreferenceStore::new(),
            memory: None,
            model_selector: ModelSelector::default(),
            tool_registry,
            email_client: None,
            profile: ProfileStore::new(),
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

        let (preferences, memory, profile) = Self::load_persistence_from_env().await?;

        // Try to initialize email client from environment
        let email_client = Self::load_email_client_from_env();

        let maple_brain = Arc::new(maple_brain);
        let mut tool_registry = agent_tools::default_registry();
        let brain: Arc<dyn Brain> = maple_brain.clone();
        tool_registry.set_brain(brain);

        Ok(Self {
            router,
            maple_brain,
            grok_brain,
            search,
            sender,
            preferences,
            memory,
            model_selector,
            tool_registry,
            email_client,
            profile,
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

        let maple_brain = Arc::new(maple_brain);
        let mut tool_registry = agent_tools::default_registry();
        let brain: Arc<dyn Brain> = maple_brain.clone();
        tool_registry.set_brain(brain);

        let (preferences, memory, profile) = Self::load_persistence_from_env().await?;

        // Try to initialize email client from environment
        let email_client = Self::load_email_client_from_env();

        Ok(Self {
            router,
            maple_brain,
            grok_brain,
            search,
            sender,
            preferences,
            memory,
            model_selector: ModelSelector::from_env(),
            tool_registry,
            email_client,
            profile,
        })
    }

    /// Get the history key for a message.
    ///
    /// Uses group ID for group messages, sender for direct messages.
    fn history_key(message: &InboundMessage) -> String {
        message.history_key()
    }

    fn resolve_task_hint(message: &InboundMessage, task_hint: TaskHint) -> TaskHint {
        if message.has_images() {
            TaskHint::Vision
        } else {
            task_hint
        }
    }

    fn attach_routing_info(
        &self,
        message: &mut InboundMessage,
        sensitivity: Option<Sensitivity>,
        task_hint: TaskHint,
        model_override: Option<String>,
        use_grok: bool,
        memory_context: Option<&MemoryContext>,
    ) {
        let mut routing = message.routing.clone().unwrap_or_default();
        routing.sensitivity = sensitivity;
        routing.task_hint = Some(task_hint);
        routing.model_override = model_override;
        routing.router_prompt_hash = Some(self.router.prompt_hash().to_string());

        let system_prompt_hash = if use_grok {
            self.grok_brain.system_prompt_hash()
        } else {
            self.maple_brain.system_prompt_hash()
        };
        routing.system_prompt_hash = system_prompt_hash.map(|hash| hash.to_string());
        if let Some(memory) = memory_context {
            if let Some(prompt) = memory.prompt.as_ref() {
                routing.memory_prompt = Some(prompt.clone());
                routing.memory_prompt_hash = Some(hash_prompt(prompt));
                routing.memory_source = memory.source.clone();
                routing.memory_summary_updated_at = memory.summary_updated_at.clone();
            }
        }
        message.routing = Some(routing);
    }

    async fn load_memory_context(&self, history_key: &str) -> MemoryContext {
        let Some(memory) = &self.memory else {
            return MemoryContext::default();
        };

        let policy = memory.prompt_policy_for(history_key);
        match memory.snapshot_with_policy(history_key, &policy).await {
            Ok(snapshot) => MemoryContext {
                summary: snapshot.summary.clone(),
                summary_updated_at: snapshot.summary_updated_at.clone(),
                prompt: format_memory_prompt(&snapshot, &policy),
                source: Some("sqlite".to_string()),
            },
            Err(err) => {
                warn!("Failed to load memory snapshot: {}", err);
                MemoryContext::default()
            }
        }
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
        let memory_context = self.load_memory_context(&history_key).await;
        let maple_context = self.maple_brain.get_context_summary(&history_key).await;
        let mut routing_context = memory_context.summary.clone();
        if routing_context.is_none() {
            routing_context = maple_context;
        }
        if let Some(ref ctx) = routing_context {
            debug!("Conversation context: {}", ctx);
        }

        // 3. Route the message with context and attachments
        let plan = self
            .router
            .route_with_attachments(
                &message.text,
                routing_context.as_deref(),
                &message.attachments,
            )
            .await;
        info!(
            "Routing plan: {} actions (attachments: {})",
            plan.actions.len(),
            message.attachments.len()
        );

        // 4. Execute actions, building context
        let memory_context_ref = if memory_context.prompt.is_some() {
            Some(&memory_context)
        } else {
            None
        };
        let result = self
            .execute_plan(
                &message,
                &plan,
                recipient,
                is_group,
                &history_key,
                memory_context_ref,
            )
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
        memory_context: Option<&MemoryContext>,
    ) -> Result<OutboundMessage, OrchestratorError> {
        let mut context = Context::new();

        for action in &plan.actions {
            match action {
                OrchestratorAction::Search {
                    query,
                    message: status_msg,
                } => {
                    self.execute_search(
                        message,
                        history_key,
                        query,
                        status_msg.as_deref(),
                        &mut context,
                        recipient,
                        is_group,
                    )
                    .await?;
                }

                OrchestratorAction::ClearContext { .. } => {
                    self.execute_clear_context(history_key, &message.sender).await?;
                }

                OrchestratorAction::Help => {
                    return Ok(OutboundMessage::reply_to(message, HELP_TEXT));
                }

                OrchestratorAction::Respond {
                    sensitivity,
                    task_hint,
                    has_pii,
                    pii_types,
                } => {
                    // If PII is detected, ask user how they want to handle it
                    if *has_pii && !pii_types.is_empty() {
                        return self
                            .execute_ask_privacy_choice(
                                message,
                                pii_types,
                                &message.text,
                                *sensitivity,
                                *task_hint,
                            )
                            .await;
                    }
                    return self
                        .execute_respond(
                            message,
                            &context,
                            *sensitivity,
                            *task_hint,
                            history_key,
                            memory_context,
                        )
                        .await;
                }

                OrchestratorAction::Grok { query, task_hint } => {
                    return self
                        .execute_direct_grok(message, query, &context, *task_hint, memory_context)
                        .await;
                }

                OrchestratorAction::Maple { query, task_hint } => {
                    return self
                        .execute_direct_maple(
                            message,
                            query,
                            &context,
                            *task_hint,
                            history_key,
                            memory_context,
                        )
                        .await;
                }

                OrchestratorAction::MapleModel { query, model, task_hint } => {
                    return self
                        .execute_maple_with_model(message, query, model, &context, *task_hint, history_key)
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

                OrchestratorAction::UseTool {
                    name,
                    args,
                    message: status_msg,
                } => {
                    self.execute_use_tool(
                        message,
                        history_key,
                        name,
                        args,
                        status_msg.as_deref(),
                        &mut context,
                        recipient,
                        is_group,
                    )
                    .await?;
                }

                OrchestratorAction::AskPrivacyChoice {
                    pii_types,
                    original_message,
                    sensitivity,
                    task_hint,
                } => {
                    return self
                        .execute_ask_privacy_choice(
                            message,
                            pii_types,
                            original_message,
                            *sensitivity,
                            *task_hint,
                        )
                        .await;
                }

                OrchestratorAction::PrivacyChoiceResponse { choice } => {
                    return self
                        .execute_privacy_choice_response(message, *choice, history_key)
                        .await;
                }

                OrchestratorAction::SendEmail {
                    recipient,
                    subject,
                    body,
                } => {
                    return self
                        .execute_send_email(message, recipient, subject.as_deref(), body.as_deref())
                        .await;
                }

                OrchestratorAction::ViewProfile => {
                    return self.execute_view_profile(message).await;
                }

                OrchestratorAction::UpdateProfile { field, value } => {
                    return self
                        .execute_update_profile(message, field, value.as_deref())
                        .await;
                }

                OrchestratorAction::ClearProfile => {
                    return self.execute_clear_profile(message).await;
                }
            }
        }

        // If no Respond action in plan, generate one with default sensitivity and task hint
        info!("No response action in plan, generating response anyway");
        let fallback_task_hint = if message.has_images() {
            TaskHint::Vision
        } else {
            TaskHint::default()
        };
        let fallback_sensitivity = if message.has_images() {
            Sensitivity::Sensitive
        } else {
            Sensitivity::default()
        };

        self.execute_respond(
            message,
            &context,
            fallback_sensitivity,
            fallback_task_hint,
            history_key,
            memory_context,
        )
        .await
    }

    /// Execute a search action.
    async fn execute_search(
        &self,
        message: &InboundMessage,
        history_key: &str,
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
        let args_json = json!({ "query": query }).to_string();
        let request = ToolRequest::from_call(
            "orchestrator-search".to_string(),
            "realtime_search".to_string(),
            &args_json,
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

        self.record_tool_history(
            history_key,
            "realtime_search",
            result.success,
            &result.content,
            message,
        )
        .await;

        Ok(())
    }

    /// Execute a clear context action (silent - no user notification).
    async fn execute_clear_context(
        &self,
        history_key: &str,
        sender_id: &str,
    ) -> Result<(), OrchestratorError> {
        info!("Clearing conversation history for {}", history_key);
        self.maple_brain.clear_history(history_key).await;
        self.grok_brain.clear_history(history_key).await;

        if let Some(memory) = &self.memory {
            if let Err(err) = memory.clear_context(history_key, Some(sender_id)).await {
                warn!("Failed to record clear context: {}", err);
            }
        }
        Ok(())
    }

    /// Execute a use_tool action.
    async fn execute_use_tool(
        &self,
        message: &InboundMessage,
        history_key: &str,
        name: &str,
        args: &HashMap<String, Value>,
        status_message: Option<&str>,
        context: &mut Context,
        recipient: &str,
        is_group: bool,
    ) -> Result<(), OrchestratorError> {
        info!("Executing tool '{}' with {} args", name, args.len());

        // Send status message if provided
        if let Some(msg) = status_message {
            if let Err(e) = self.sender.send_message(recipient, msg, is_group).await {
                warn!("Failed to send tool status notification: {}", e);
            }

            // Restart typing indicator after sending message
            if let Err(e) = self.sender.set_typing(recipient, is_group, true).await {
                warn!("Failed to restart typing indicator: {}", e);
            }
        }

        // Execute the tool
        let (tool_success, tool_content) = match self.tool_registry.execute(name, args.clone()).await {
            Ok(result) => {
                let content = result.content;
                if result.success {
                    info!(
                        "Tool '{}' completed successfully ({} chars)",
                        name,
                        content.len()
                    );
                    context.add_tool_result(name, &content);
                } else {
                    warn!("Tool '{}' returned failure: {}", name, content);
                    context.add_tool_result(name, &format!("Tool failed: {}", content));
                }
                (result.success, content)
            }
            Err(e) => {
                warn!("Tool '{}' execution error: {}", name, e);
                let content = format!("Tool error: {}", e);
                context.add_tool_result(name, &content);
                (false, content)
            }
        };

        self.record_tool_history(history_key, name, tool_success, &tool_content, message)
            .await;

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
        memory_context: Option<&MemoryContext>,
    ) -> Result<OutboundMessage, OrchestratorError> {
        // Vision tasks MUST use Maple - Grok has no vision support
        let effective_task_hint = Self::resolve_task_hint(message, task_hint);
        let force_maple = effective_task_hint == TaskHint::Vision;

        // Determine which agent to use based on sensitivity and user preference
        // (unless vision/images force Maple)
        let use_grok = if force_maple {
            false
        } else {
            self.preferences
                .should_use_grok(history_key, sensitivity)
                .await
        };

        let indicator = if use_grok {
            AgentIndicator::Speed
        } else {
            AgentIndicator::Privacy
        };

        // Select the best model based on task hint
        let selected_model = if use_grok {
            self.model_selector.select_grok(effective_task_hint)
        } else {
            self.model_selector.select_maple(effective_task_hint)
        };

        info!(
            "Generating response with {:?} (sensitivity: {:?}, task_hint: {:?}, model: {}, use_grok: {}, force_maple: {})",
            indicator, sensitivity, effective_task_hint, selected_model, use_grok, force_maple
        );

        // Augment message with search context if any
        let mut augmented = context.augment_message(message);
        self.attach_routing_info(
            &mut augmented,
            Some(sensitivity),
            effective_task_hint,
            Some(selected_model.to_string()),
            use_grok,
            memory_context,
        );

        debug!("Context summary: {}", context.format_summary());

        // Process through the appropriate brain
        // Note: Currently using the default model configured in the brain.
        // TODO: Add per-request model override support to brains for dynamic model selection.
        let mut response = if use_grok {
            self.grok_brain.process(augmented).await?
        } else {
            self.maple_brain.process(augmented).await?
        };
        let summary_text = response.text.clone();

        // Format response with metadata footer
        let mode_label = indicator.label();
        let tools_used: Option<Vec<String>> = if context.has_results() {
            Some(context.tools_used())
        } else {
            None
        };
        let formatted = format_with_footer(
            &response.text,
            mode_label,
            Some(selected_model),
            tools_used.as_deref(),
        );

        // Apply formatting to response
        response.text = formatted.text;
        response.styles = formatted.styles;

        self.record_exchange(history_key, &message.text, &summary_text)
            .await;

        info!("Generated response: {} chars, {} styles", response.text.len(), response.styles.len());
        Ok(response)
    }

    /// Execute a direct Grok query (user explicitly requested).
    ///
    /// Note: If the message has images and task_hint is Vision, this falls back to Maple
    /// since Grok doesn't support vision.
    async fn execute_direct_grok(
        &self,
        message: &InboundMessage,
        query: &str,
        context: &Context,
        task_hint: TaskHint,
        memory_context: Option<&MemoryContext>,
    ) -> Result<OutboundMessage, OrchestratorError> {
        // If this is a vision task, fall back to Maple (Grok doesn't support vision)
        if task_hint == TaskHint::Vision || message.has_images() {
            warn!(
                "Grok requested but message has images - falling back to Maple (Grok has no vision support)"
            );
            return self
                .execute_direct_maple(
                    message,
                    query,
                    context,
                    task_hint,
                    &Self::history_key(message),
                    memory_context,
                )
                .await;
        }

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
        let mut augmented = context.augment_message(&modified);
        self.attach_routing_info(
            &mut augmented,
            None,
            task_hint,
            Some(selected_model.to_string()),
            true,
            memory_context,
        );

        // Process through Grok
        // Note: Currently using the default model configured in the brain.
        // TODO: Add per-request model override support for dynamic model selection.
        let mut response = self.grok_brain.process(augmented).await?;
        let summary_text = response.text.clone();

        // Add speed indicator
        let indicator = AgentIndicator::Speed;
        if !indicator.prefix().is_empty() {
            response.text = format!("{}{}", indicator.prefix(), response.text);
        }

        let history_key = Self::history_key(message);
        self.record_exchange(&history_key, query, &summary_text).await;

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
        history_key: &str,
        memory_context: Option<&MemoryContext>,
    ) -> Result<OutboundMessage, OrchestratorError> {
        let effective_task_hint = Self::resolve_task_hint(message, task_hint);

        // Select the best model based on task hint
        let selected_model = self.model_selector.select_maple(effective_task_hint);

        info!(
            "Direct Maple query (task_hint: {:?}, model: {}): {}",
            effective_task_hint, selected_model, query
        );

        // Create a modified message with the extracted query
        let mut modified = message.clone();
        modified.text = query.to_string();

        // Augment with context if any
        let mut augmented = context.augment_message(&modified);
        self.attach_routing_info(
            &mut augmented,
            None,
            effective_task_hint,
            Some(selected_model.to_string()),
            false,
            memory_context,
        );

        // Process through Maple
        // Note: Currently using the default model configured in the brain.
        // TODO: Add per-request model override support for dynamic model selection.
        let response = self.maple_brain.process(augmented).await?;

        self.record_exchange(history_key, query, &response.text).await;

        info!("Direct Maple response: {} chars", response.text.len());
        Ok(response)
    }

    /// Execute a Maple query with a specific model (one-time use).
    async fn execute_maple_with_model(
        &self,
        message: &InboundMessage,
        query: &str,
        model_alias: &str,
        context: &Context,
        task_hint: TaskHint,
        history_key: &str,
    ) -> Result<OutboundMessage, OrchestratorError> {
        use crate::model_selection::MapleModels;

        let effective_task_hint = Self::resolve_task_hint(message, task_hint);

        // Validate and normalize the model alias
        let selected_model = match MapleModels::normalize_model(model_alias) {
            Some(canonical) => canonical.to_string(),
            None => {
                // Invalid model - return a helpful error message
                let available = MapleModels::model_aliases().join(", ");
                let error_msg = format!(
                    "Unknown model '{}'. Available models: {}",
                    model_alias, available
                );
                warn!("{}", error_msg);
                return Ok(OutboundMessage::reply_to(message, error_msg));
            }
        };

        info!(
            "Maple query with model (alias: {}, model: {}, task_hint: {:?}): {}",
            model_alias, selected_model, effective_task_hint, query
        );

        // Create a modified message with the extracted query
        let mut modified = message.clone();
        modified.text = query.to_string();

        // Augment with context if any
        let mut augmented = context.augment_message(&modified);
        self.attach_routing_info(
            &mut augmented,
            None,
            effective_task_hint,
            Some(selected_model.clone()),
            false,
            None,
        );

        // Process through Maple
        // Note: Currently using the default model configured in the brain.
        // TODO: Add per-request model override support for dynamic model selection.
        let response = self.maple_brain.process(augmented).await?;

        self.record_exchange(history_key, query, &response.text).await;

        info!("Maple with model '{}' response: {} chars", selected_model, response.text.len());
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

    /// Execute an ask privacy choice action - ask user how to handle detected PII.
    async fn execute_ask_privacy_choice(
        &self,
        message: &InboundMessage,
        pii_types: &[String],
        _original_message: &str,
        _sensitivity: Sensitivity,
        _task_hint: TaskHint,
    ) -> Result<OutboundMessage, OrchestratorError> {
        let pii_list = pii_types.join(", ");

        let response_text = format!(
            "I noticed your message contains personal information ({}).\n\n\
             How would you like me to handle it?\n\n\
             1. Sanitize - Remove personal details and use fast mode\n\
             2. Private - Keep as-is and use secure enclave\n\
             3. Fast - Keep as-is and use fast mode (data sent to external service)\n\
             4. Cancel - Don't process this message\n\n\
             Reply with 1, 2, 3, or 4",
            pii_list
        );

        info!("Asking privacy choice for PII types: {}", pii_list);
        Ok(OutboundMessage::reply_to(message, response_text))
    }

    /// Execute a privacy choice response - handle user's choice for PII handling.
    ///
    /// SECURITY NOTE: Sanitization is not yet implemented. Only FastUncensored and
    /// Cancel are currently functional. Private and Sanitize return error messages
    /// to avoid misleading users about data handling.
    async fn execute_privacy_choice_response(
        &self,
        message: &InboundMessage,
        choice: PrivacyChoice,
        history_key: &str,
    ) -> Result<OutboundMessage, OrchestratorError> {
        info!("Processing privacy choice: {:?}", choice);
        self.record_privacy_choice(history_key, choice, message).await;

        let response_text = match choice {
            PrivacyChoice::Cancel => {
                "Request cancelled. Your message was not processed."
            }
            PrivacyChoice::FastUncensored => {
                "Processing with fast mode. Note: Your data will be sent to an external AI service."
            }
            PrivacyChoice::Sanitize | PrivacyChoice::Private => {
                // SECURITY: Sanitization is not yet implemented. Return an honest error.
                "Sorry, the privacy choice feature is temporarily unavailable. \
                 Your message was not processed to protect your privacy. \
                 Please try again later, use option 3 (Fast) if you accept the risk, \
                 or rephrase your request without sensitive information."
            }
        };
        Ok(OutboundMessage::reply_to(message, response_text))
    }

    /// Execute a send_email action - send attachments to email via proton-proxy.
    async fn execute_send_email(
        &self,
        message: &InboundMessage,
        recipient: &str,
        subject: Option<&str>,
        body: Option<&str>,
    ) -> Result<OutboundMessage, OrchestratorError> {
        // Validate email address
        if !is_valid_email(recipient) {
            let error_msg = format!("Invalid email address: {}", recipient);
            warn!("{}", error_msg);
            return Ok(OutboundMessage::reply_to(message, error_msg));
        }

        // Check if email client is configured
        let client = match &self.email_client {
            Some(c) => c,
            None => {
                let error_msg = "Email sending is not configured. Please set up proton-proxy.";
                warn!("{}", error_msg);
                return Ok(OutboundMessage::reply_to(message, error_msg));
            }
        };

        // Check if there are attachments to send
        if message.attachments.is_empty() {
            let error_msg = "No attachments to send. Please include a file with your message.";
            return Ok(OutboundMessage::reply_to(message, error_msg));
        }

        // Build the email
        let subject = subject.unwrap_or("Attachment from Signal");
        let body_text = body.unwrap_or("Please find the attached file(s).");

        let mut email = proton_proxy::Email::new(recipient, subject, body_text);

        // Add each attachment
        let mut attachment_count = 0;
        for att in &message.attachments {
            if let Some(ref file_path) = att.file_path {
                match proton_proxy::Attachment::from_file(file_path) {
                    Ok(email_att) => {
                        email.attach(email_att);
                        attachment_count += 1;
                    }
                    Err(e) => {
                        warn!("Failed to load attachment {}: {}", file_path, e);
                    }
                }
            }
        }

        if attachment_count == 0 {
            let error_msg = "Could not load any attachments. Files may be unavailable.";
            return Ok(OutboundMessage::reply_to(message, error_msg));
        }

        // Send the email
        match client.send(&email).await {
            Ok(()) => {
                let response = format!("Sent {} attachment(s) to {}", attachment_count, recipient);
                info!("{}", response);
                Ok(OutboundMessage::reply_to(message, response))
            }
            Err(e) => {
                let error_msg = format!("Failed to send email: {}", e);
                warn!("{}", error_msg);
                Ok(OutboundMessage::reply_to(message, error_msg))
            }
        }
    }

    /// Execute a view_profile action - show user's profile settings.
    async fn execute_view_profile(
        &self,
        message: &InboundMessage,
    ) -> Result<OutboundMessage, OrchestratorError> {
        info!("Viewing profile for {}", message.sender);

        let profile = self.profile.get(&message.sender).await;
        let response_text = ProfileStore::format_profile(profile.as_ref());

        Ok(OutboundMessage::reply_to(message, response_text))
    }

    /// Execute an update_profile action - update a profile field.
    async fn execute_update_profile(
        &self,
        message: &InboundMessage,
        field_name: &str,
        value: Option<&str>,
    ) -> Result<OutboundMessage, OrchestratorError> {
        // Parse the field name
        let field = match ProfileStore::parse_field(field_name) {
            Ok(f) => f,
            Err(e) => {
                let error_msg = e.to_string();
                warn!("Invalid profile field: {}", error_msg);
                return Ok(OutboundMessage::reply_to(message, error_msg));
            }
        };

        // Log the action (field name only, not values for privacy)
        info!(
            "Updating profile field {:?} for {} (has value: {})",
            field,
            message.sender,
            value.is_some()
        );

        // Attempt the update
        match self.profile.update_field(&message.sender, field, value).await {
            Ok(()) => {
                let response_text = match value {
                    Some(_) => format!("Updated your {}.", field.display_name()),
                    None => format!("Cleared your {}.", field.display_name()),
                };
                Ok(OutboundMessage::reply_to(message, response_text))
            }
            Err(e) => {
                let error_msg = e.to_string();
                warn!("Profile update failed: {}", error_msg);
                Ok(OutboundMessage::reply_to(message, error_msg))
            }
        }
    }

    /// Execute a clear_profile action - delete all profile settings.
    async fn execute_clear_profile(
        &self,
        message: &InboundMessage,
    ) -> Result<OutboundMessage, OrchestratorError> {
        info!("Clearing profile for {}", message.sender);

        match self.profile.delete(&message.sender).await {
            Ok(true) => Ok(OutboundMessage::reply_to(
                message,
                "All your profile settings have been deleted.",
            )),
            Ok(false) => Ok(OutboundMessage::reply_to(
                message,
                "You don't have any profile settings to delete.",
            )),
            Err(e) => {
                let error_msg = format!("Failed to delete profile: {}", e);
                warn!("{}", error_msg);
                Ok(OutboundMessage::reply_to(message, error_msg))
            }
        }
    }

    async fn record_exchange(
        &self,
        history_key: &str,
        user_text: &str,
        assistant_text: &str,
    ) {
        if let Some(memory) = &self.memory {
            if let Err(err) = memory
                .record_exchange(history_key, user_text, assistant_text)
                .await
            {
                warn!("Failed to update memory summary: {}", err);
            }
        }
    }

    async fn record_privacy_choice(
        &self,
        history_key: &str,
        choice: PrivacyChoice,
        message: &InboundMessage,
    ) {
        // Log the privacy choice for auditing purposes
        // TODO: Add persistent storage if needed for compliance/auditing
        debug!(
            "Privacy choice recorded: {:?} for {} (sender: {})",
            choice, history_key, message.sender
        );
    }

    async fn record_tool_history(
        &self,
        history_key: &str,
        tool_name: &str,
        success: bool,
        content: &str,
        message: &InboundMessage,
    ) {
        if let Some(memory) = &self.memory {
            if let Err(err) = memory
                .record_tool(
                    history_key,
                    tool_name,
                    success,
                    content,
                    Some(&message.sender),
                    message.group_id.as_deref(),
                )
                .await
            {
                warn!("Failed to record tool history: {}", err);
            }
        }
    }

    async fn load_persistence_from_env(
    ) -> Result<(PreferenceStore, Option<MemoryStore>, ProfileStore), OrchestratorError> {
        let sqlite_path = match env::var("SQLITE_PATH") {
            Ok(path) => path,
            Err(_) => return Ok((PreferenceStore::new(), None, ProfileStore::new())),
        };

        let sqlite_url = sqlite_url_from_path(&sqlite_path);
        let database = Database::connect(&sqlite_url)
            .await
            .map_err(|e| OrchestratorError::ToolFailed(format!("Database error: {}", e)))?;
        database
            .migrate()
            .await
            .map_err(|e| OrchestratorError::ToolFailed(format!("Database migration error: {}", e)))?;

        let preferences = PreferenceStore::with_database(database.clone());
        let settings = MemorySettings::from_env();
        let memory = Some(MemoryStore::new(database.clone(), settings));
        let profile = ProfileStore::with_database(database);

        Ok((preferences, memory, profile))
    }

    /// Try to create an email client from environment variables.
    /// Returns None if not configured (missing PROTON_USERNAME/PROTON_PASSWORD).
    fn load_email_client_from_env() -> Option<ProtonClient> {
        match proton_proxy::ProtonConfig::from_env() {
            Ok(config) => match ProtonClient::new(config) {
                Ok(client) => {
                    info!("Email client initialized");
                    Some(client)
                }
                Err(e) => {
                    warn!("Failed to create email client: {}", e);
                    None
                }
            }
            Err(_) => {
                debug!("Email client not configured (missing PROTON_USERNAME/PROTON_PASSWORD)");
                None
            }
        }
    }

    /// Get the sender.
    pub fn sender(&self) -> &S {
        &self.sender
    }

    /// Get the Maple brain.
    pub fn maple_brain(&self) -> &MapleBrain {
        self.maple_brain.as_ref()
    }

    /// Get the Grok brain.
    pub fn grok_brain(&self) -> &GrokBrain {
        &self.grok_brain
    }

    /// Get the preference store.
    pub fn preferences(&self) -> &PreferenceStore {
        &self.preferences
    }

    /// Get the memory store, if configured.
    pub fn memory(&self) -> Option<&MemoryStore> {
        self.memory.as_ref()
    }

    /// Get the model selector.
    pub fn model_selector(&self) -> &ModelSelector {
        &self.model_selector
    }

    /// Get the tool registry.
    pub fn tool_registry(&self) -> &ToolRegistry {
        &self.tool_registry
    }

    /// Get a mutable reference to the tool registry.
    pub fn tool_registry_mut(&mut self) -> &mut ToolRegistry {
        &mut self.tool_registry
    }

    /// Get the profile store.
    pub fn profile(&self) -> &ProfileStore {
        &self.profile
    }
}

fn sqlite_url_from_path(path: &str) -> String {
    if path.starts_with("sqlite:") {
        path.to_string()
    } else {
        format!("sqlite:{}?mode=rwc", path)
    }
}

/// Basic email address validation.
fn is_valid_email(email: &str) -> bool {
    let email = email.trim();
    if email.is_empty() || email.len() > 254 {
        return false;
    }
    // Must have exactly one @, with content on both sides
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }
    let (local, domain) = (parts[0], parts[1]);
    // Basic checks
    !local.is_empty() && !domain.is_empty() && domain.contains('.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sender::NoOpSender;
    use brain_core::InboundAttachment;

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

    #[test]
    fn test_resolve_task_hint_no_images() {
        let message = InboundMessage::direct("+1234567890", "hello", 123);
        let hint = Orchestrator::<NoOpSender>::resolve_task_hint(&message, TaskHint::Math);
        assert_eq!(hint, TaskHint::Math);
    }

    #[test]
    fn test_resolve_task_hint_with_images() {
        let mut message = InboundMessage::direct("+1234567890", "hello", 123);
        message.attachments.push(InboundAttachment {
            content_type: "image/png".to_string(),
            ..Default::default()
        });

        let hint = Orchestrator::<NoOpSender>::resolve_task_hint(&message, TaskHint::General);
        assert_eq!(hint, TaskHint::Vision);
    }

    #[test]
    fn test_is_valid_email() {
        // Valid emails
        assert!(is_valid_email("user@example.com"));
        assert!(is_valid_email("user.name@example.com"));
        assert!(is_valid_email("user@sub.example.com"));
        assert!(is_valid_email("user+tag@example.com"));
        assert!(is_valid_email("  user@example.com  ")); // trimmed

        // Invalid emails
        assert!(!is_valid_email(""));
        assert!(!is_valid_email("   "));
        assert!(!is_valid_email("user"));
        assert!(!is_valid_email("user@"));
        assert!(!is_valid_email("@example.com"));
        assert!(!is_valid_email("user@example")); // no TLD
        assert!(!is_valid_email("user@@example.com")); // double @
        assert!(!is_valid_email("user@example@other.com")); // multiple @
    }
}
