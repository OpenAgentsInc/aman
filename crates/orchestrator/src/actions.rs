//! Routing plan and action types for orchestration.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// Re-export types from brain_core for consistency
pub use brain_core::{Sensitivity, TaskHint};

/// User preference for which agent to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UserPreference {
    /// Default behavior: sensitive→Maple, insensitive→Grok, uncertain→Maple.
    #[default]
    Default,

    /// Prefer privacy: always use Maple (except explicit "grok:" commands).
    PreferPrivacy,

    /// Prefer speed: always use Grok (except explicit sensitive detection overrides).
    PreferSpeed,
}

/// User's choice for how to handle detected PII.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyChoice {
    /// Sanitize: Remove PII and use fast mode (Grok).
    Sanitize,
    /// Private: Keep PII and use privacy mode (Maple TEE).
    Private,
    /// FastUncensored: Keep PII and use fast mode (Grok) - user accepts risk.
    FastUncensored,
    /// Cancel: Don't process the message.
    Cancel,
}

impl PrivacyChoice {
    /// Parse a privacy choice from user input.
    pub fn from_input(input: &str) -> Option<Self> {
        let input = input.trim().to_lowercase();
        match input.as_str() {
            "1" | "sanitize" | "sanitise" | "remove" => Some(Self::Sanitize),
            "2" | "private" | "privacy" | "secure" | "maple" => Some(Self::Private),
            "3" | "fast" | "uncensored" | "grok" | "speed" => Some(Self::FastUncensored),
            "4" | "cancel" | "stop" | "nevermind" | "never mind" | "no" => Some(Self::Cancel),
            _ => None,
        }
    }

    /// Get a human-readable description of this choice.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Sanitize => "sanitize and use fast mode",
            Self::Private => "keep private and use secure mode",
            Self::FastUncensored => "use fast mode with full data (accepts risk)",
            Self::Cancel => "cancel the request",
        }
    }
}

impl UserPreference {
    /// Parse a preference from a string (e.g., from router action).
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "prefer_privacy" | "privacy" | "maple" => Self::PreferPrivacy,
            "prefer_speed" | "speed" | "grok" | "fast" => Self::PreferSpeed,
            _ => Self::Default,
        }
    }

    /// Get the storage string for this preference.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::PreferPrivacy => "prefer_privacy",
            Self::PreferSpeed => "prefer_speed",
        }
    }

    /// Get a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Default => "default (privacy for sensitive, speed for general)",
            Self::PreferPrivacy => "privacy mode (all requests use secure enclave)",
            Self::PreferSpeed => "speed mode (all requests use fast processing)",
        }
    }
}

/// The routing plan from the first-pass analysis.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoutingPlan {
    /// Ordered list of actions to execute.
    pub actions: Vec<OrchestratorAction>,
}

impl RoutingPlan {
    /// Create a new routing plan with the given actions.
    pub fn new(actions: Vec<OrchestratorAction>) -> Self {
        Self { actions }
    }

    /// Create a simple plan with just a respond action.
    pub fn respond_only() -> Self {
        Self {
            actions: vec![OrchestratorAction::Respond {
                sensitivity: Sensitivity::default(),
                task_hint: TaskHint::default(),
                has_pii: false,
                pii_types: Vec::new(),
            }],
        }
    }

    /// Create a simple plan with just a respond action with specific sensitivity.
    pub fn respond_with_sensitivity(sensitivity: Sensitivity) -> Self {
        Self {
            actions: vec![OrchestratorAction::Respond {
                sensitivity,
                task_hint: TaskHint::default(),
                has_pii: false,
                pii_types: Vec::new(),
            }],
        }
    }

    /// Create a simple plan with respond action with sensitivity and task hint.
    pub fn respond_with_hint(sensitivity: Sensitivity, task_hint: TaskHint) -> Self {
        Self {
            actions: vec![OrchestratorAction::Respond {
                sensitivity,
                task_hint,
                has_pii: false,
                pii_types: Vec::new(),
            }],
        }
    }

    /// Check if the plan is empty.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Check if the plan contains a search action.
    pub fn has_search(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::Search { .. }))
    }

    /// Check if the plan contains a clear context action.
    pub fn has_clear_context(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::ClearContext { .. }))
    }

    /// Check if the plan is an ignore action (accidental message).
    pub fn is_ignore(&self) -> bool {
        self.actions.len() == 1 && matches!(self.actions[0], OrchestratorAction::Ignore)
    }

    /// Check if the plan contains a direct Grok action.
    pub fn has_direct_grok(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::Grok { .. }))
    }

    /// Check if the plan contains a direct Maple action.
    pub fn has_direct_maple(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::Maple { .. }))
    }

    /// Check if the plan contains a Maple model action.
    pub fn has_maple_model(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::MapleModel { .. }))
    }

    /// Check if the plan contains a set preference action.
    pub fn has_set_preference(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::SetPreference { .. }))
    }

    /// Check if the plan contains a use_tool action.
    pub fn has_use_tool(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::UseTool { .. }))
    }

    /// Check if the plan contains an ask_privacy_choice action.
    pub fn has_ask_privacy_choice(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::AskPrivacyChoice { .. }))
    }

    /// Check if any action in the plan detected PII.
    pub fn has_pii(&self) -> bool {
        self.actions.iter().any(|a| a.has_pii())
    }

    /// Check if the plan contains a privacy_choice_response action.
    pub fn has_privacy_choice_response(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::PrivacyChoiceResponse { .. }))
    }

    /// Check if the plan contains a send_email action.
    pub fn has_send_email(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::SendEmail { .. }))
    }

    /// Check if the plan contains a view_profile action.
    pub fn has_view_profile(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::ViewProfile))
    }

    /// Check if the plan contains an update_profile action.
    pub fn has_update_profile(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::UpdateProfile { .. }))
    }

    /// Check if the plan contains a clear_profile action.
    pub fn has_clear_profile(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::ClearProfile))
    }
}

/// Individual action in the routing plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestratorAction {
    /// Search for real-time information before responding.
    Search {
        /// Privacy-safe search query crafted by the router.
        query: String,
        /// Personal status message to show user (e.g., "Let me look that up for you...")
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    /// Clear conversation context.
    ClearContext {
        /// Personal confirmation message (e.g., "Fresh start! What's on your mind?")
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    /// Show help information.
    Help,

    /// Generate final response (may include gathered context).
    Respond {
        /// Sensitivity level for this response.
        #[serde(default)]
        sensitivity: Sensitivity,
        /// Task hint for model selection.
        #[serde(default)]
        task_hint: TaskHint,
        /// Whether the message contains personally identifiable information.
        #[serde(default)]
        has_pii: bool,
        /// Types of PII detected (e.g., ["name", "ssn", "medical"]).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pii_types: Vec<String>,
    },

    /// Route directly to Grok (user explicitly requested).
    Grok {
        /// The user's query to send to Grok.
        query: String,
        /// Task hint for model selection.
        #[serde(default)]
        task_hint: TaskHint,
    },

    /// Route directly to Maple (user explicitly requested).
    Maple {
        /// The user's query to send to Maple.
        query: String,
        /// Task hint for model selection.
        #[serde(default)]
        task_hint: TaskHint,
    },

    /// Route to Maple with a specific model (one-time use).
    MapleModel {
        /// The user's query to send to Maple.
        query: String,
        /// The specific model alias to use (e.g., "deepseek", "llama").
        model: String,
        /// Task hint for fallback if model is invalid.
        #[serde(default)]
        task_hint: TaskHint,
    },

    /// Set user preference for which agent to use.
    SetPreference {
        /// The preference to set.
        preference: String,
    },

    /// Skip processing (e.g., for system messages).
    Skip {
        /// Reason for skipping.
        reason: String,
    },

    /// Ignore the message silently (accidental/mistaken input like "?" or typos).
    /// Unlike Skip, this produces no response at all.
    Ignore,

    /// Execute a tool from the registry.
    UseTool {
        /// Tool name (e.g., "calculator", "weather", "web_fetch").
        name: String,
        /// Tool arguments as key-value pairs.
        #[serde(default)]
        args: HashMap<String, Value>,
        /// Optional status message to show user while tool runs.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    /// Ask user how to handle detected PII before responding.
    /// This action is generated when PII is detected in a message.
    AskPrivacyChoice {
        /// Types of PII detected in the message.
        pii_types: Vec<String>,
        /// The original message text (for reference).
        original_message: String,
        /// Sensitivity level for the request.
        #[serde(default)]
        sensitivity: Sensitivity,
        /// Task hint for model selection.
        #[serde(default)]
        task_hint: TaskHint,
    },

    /// User's response to a privacy choice prompt.
    /// This action is generated when the router detects the user is responding
    /// to a previous AskPrivacyChoice prompt (based on conversation history).
    PrivacyChoiceResponse {
        /// The user's choice: "sanitize", "private", or "cancel".
        choice: PrivacyChoice,
    },

    /// Send attachments to an email address via proton-proxy.
    SendEmail {
        /// Recipient email address.
        recipient: String,
        /// Optional subject line (defaults to "Attachment from Signal").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject: Option<String>,
        /// Optional body text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        body: Option<String>,
    },

    /// View user's profile settings.
    ViewProfile,

    /// Update a profile setting.
    UpdateProfile {
        /// Field to update: "default_model", "email", or "bolt12_offer".
        field: String,
        /// New value (None to clear the field).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },

    /// Clear all profile settings.
    ClearProfile,
}

impl OrchestratorAction {
    /// Create a search action with the given query.
    pub fn search(query: impl Into<String>) -> Self {
        Self::Search {
            query: query.into(),
            message: None,
        }
    }

    /// Create a search action with a custom status message.
    pub fn search_with_message(query: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Search {
            query: query.into(),
            message: Some(message.into()),
        }
    }

    /// Create a clear context action.
    pub fn clear_context() -> Self {
        Self::ClearContext { message: None }
    }

    /// Create a clear context action with a custom message.
    pub fn clear_context_with_message(message: impl Into<String>) -> Self {
        Self::ClearContext {
            message: Some(message.into()),
        }
    }

    /// Create a respond action with sensitivity.
    pub fn respond(sensitivity: Sensitivity) -> Self {
        Self::Respond {
            sensitivity,
            task_hint: TaskHint::default(),
            has_pii: false,
            pii_types: Vec::new(),
        }
    }

    /// Create a respond action with sensitivity and task hint.
    pub fn respond_with_hint(sensitivity: Sensitivity, task_hint: TaskHint) -> Self {
        Self::Respond {
            sensitivity,
            task_hint,
            has_pii: false,
            pii_types: Vec::new(),
        }
    }

    /// Create a respond action with PII information.
    pub fn respond_with_pii(
        sensitivity: Sensitivity,
        task_hint: TaskHint,
        pii_types: Vec<String>,
    ) -> Self {
        Self::Respond {
            sensitivity,
            task_hint,
            has_pii: !pii_types.is_empty(),
            pii_types,
        }
    }

    /// Create a direct Grok action.
    pub fn grok(query: impl Into<String>) -> Self {
        Self::Grok {
            query: query.into(),
            task_hint: TaskHint::default(),
        }
    }

    /// Create a direct Grok action with task hint.
    pub fn grok_with_hint(query: impl Into<String>, task_hint: TaskHint) -> Self {
        Self::Grok {
            query: query.into(),
            task_hint,
        }
    }

    /// Create a direct Maple action.
    pub fn maple(query: impl Into<String>) -> Self {
        Self::Maple {
            query: query.into(),
            task_hint: TaskHint::default(),
        }
    }

    /// Create a direct Maple action with task hint.
    pub fn maple_with_hint(query: impl Into<String>, task_hint: TaskHint) -> Self {
        Self::Maple {
            query: query.into(),
            task_hint,
        }
    }

    /// Create a Maple action with a specific model.
    pub fn maple_model(query: impl Into<String>, model: impl Into<String>) -> Self {
        Self::MapleModel {
            query: query.into(),
            model: model.into(),
            task_hint: TaskHint::default(),
        }
    }

    /// Create a Maple action with a specific model and task hint.
    pub fn maple_model_with_hint(query: impl Into<String>, model: impl Into<String>, task_hint: TaskHint) -> Self {
        Self::MapleModel {
            query: query.into(),
            model: model.into(),
            task_hint,
        }
    }

    /// Create a set preference action.
    pub fn set_preference(preference: impl Into<String>) -> Self {
        Self::SetPreference {
            preference: preference.into(),
        }
    }

    /// Create a skip action with the given reason.
    pub fn skip(reason: impl Into<String>) -> Self {
        Self::Skip {
            reason: reason.into(),
        }
    }

    /// Create a use_tool action.
    pub fn use_tool(name: impl Into<String>, args: HashMap<String, Value>) -> Self {
        Self::UseTool {
            name: name.into(),
            args,
            message: None,
        }
    }

    /// Create a use_tool action with a status message.
    pub fn use_tool_with_message(
        name: impl Into<String>,
        args: HashMap<String, Value>,
        message: impl Into<String>,
    ) -> Self {
        Self::UseTool {
            name: name.into(),
            args,
            message: Some(message.into()),
        }
    }

    /// Create an ask_privacy_choice action for PII handling.
    pub fn ask_privacy_choice(
        pii_types: Vec<String>,
        original_message: impl Into<String>,
        sensitivity: Sensitivity,
        task_hint: TaskHint,
    ) -> Self {
        Self::AskPrivacyChoice {
            pii_types,
            original_message: original_message.into(),
            sensitivity,
            task_hint,
        }
    }

    /// Create a privacy_choice_response action.
    pub fn privacy_choice_response(choice: PrivacyChoice) -> Self {
        Self::PrivacyChoiceResponse { choice }
    }

    /// Create a send_email action.
    pub fn send_email(recipient: impl Into<String>) -> Self {
        Self::SendEmail {
            recipient: recipient.into(),
            subject: None,
            body: None,
        }
    }

    /// Create a send_email action with a subject.
    pub fn send_email_with_subject(
        recipient: impl Into<String>,
        subject: impl Into<String>,
    ) -> Self {
        Self::SendEmail {
            recipient: recipient.into(),
            subject: Some(subject.into()),
            body: None,
        }
    }

    /// Create a send_email action with subject and body.
    pub fn send_email_full(
        recipient: impl Into<String>,
        subject: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self::SendEmail {
            recipient: recipient.into(),
            subject: Some(subject.into()),
            body: Some(body.into()),
        }
    }

    /// Create a view_profile action.
    pub fn view_profile() -> Self {
        Self::ViewProfile
    }

    /// Create an update_profile action.
    pub fn update_profile(field: impl Into<String>, value: Option<String>) -> Self {
        Self::UpdateProfile {
            field: field.into(),
            value,
        }
    }

    /// Create a clear_profile action.
    pub fn clear_profile() -> Self {
        Self::ClearProfile
    }

    /// Get a human-readable description of this action.
    pub fn description(&self) -> String {
        match self {
            Self::Search { query, .. } => format!("Search: {}", query),
            Self::ClearContext { .. } => "Clear conversation history".to_string(),
            Self::Help => "Show help information".to_string(),
            Self::Respond {
                sensitivity,
                task_hint,
                has_pii,
                ..
            } => {
                if *has_pii {
                    format!(
                        "Generate response ({:?}, {:?}, PII detected)",
                        sensitivity, task_hint
                    )
                } else {
                    format!("Generate response ({:?}, {:?})", sensitivity, task_hint)
                }
            }
            Self::Grok { query, task_hint } => {
                format!("Direct Grok ({:?}): {}", task_hint, query)
            }
            Self::Maple { query, task_hint } => {
                format!("Direct Maple ({:?}): {}", task_hint, query)
            }
            Self::MapleModel { query, model, task_hint } => {
                format!("Maple with model '{}' ({:?}): {}", model, task_hint, query)
            }
            Self::SetPreference { preference } => format!("Set preference: {}", preference),
            Self::Skip { reason } => format!("Skip: {}", reason),
            Self::Ignore => "Ignore (accidental message)".to_string(),
            Self::UseTool { name, args, .. } => {
                format!("Use tool '{}' with {} args", name, args.len())
            }
            Self::AskPrivacyChoice { pii_types, .. } => {
                format!("Ask privacy choice (PII: {})", pii_types.join(", "))
            }
            Self::PrivacyChoiceResponse { choice } => {
                format!("Privacy choice response: {}", choice.description())
            }
            Self::SendEmail {
                recipient, subject, ..
            } => match subject {
                Some(s) => format!("Send email to {} ({})", recipient, s),
                None => format!("Send email to {}", recipient),
            },
            Self::ViewProfile => "View profile settings".to_string(),
            Self::UpdateProfile { field, value } => match value {
                Some(v) => format!("Update profile: {} = {}", field, v),
                None => format!("Clear profile field: {}", field),
            },
            Self::ClearProfile => "Clear all profile settings".to_string(),
        }
    }

    /// Get the task hint from this action, if it has one.
    pub fn task_hint(&self) -> Option<TaskHint> {
        match self {
            Self::Respond { task_hint, .. } => Some(*task_hint),
            Self::Grok { task_hint, .. } => Some(*task_hint),
            Self::Maple { task_hint, .. } => Some(*task_hint),
            Self::MapleModel { task_hint, .. } => Some(*task_hint),
            Self::AskPrivacyChoice { task_hint, .. } => Some(*task_hint),
            _ => None,
        }
    }

    /// Check if this action indicates PII was detected.
    pub fn has_pii(&self) -> bool {
        match self {
            Self::Respond { has_pii, .. } => *has_pii,
            Self::AskPrivacyChoice { .. } => true, // Always has PII
            _ => false,
        }
    }

    /// Get the PII types detected for this action, if any.
    pub fn pii_types(&self) -> Option<&[String]> {
        match self {
            Self::Respond { pii_types, .. } => Some(pii_types),
            Self::AskPrivacyChoice { pii_types, .. } => Some(pii_types),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_routing_plan() {
        let json = r#"{
            "actions": [
                {"type": "search", "query": "bitcoin price today"},
                {"type": "respond", "sensitivity": "insensitive"}
            ]
        }"#;

        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert_eq!(plan.actions.len(), 2);
        assert!(plan.has_search());

        if let OrchestratorAction::Search { query, .. } = &plan.actions[0] {
            assert_eq!(query, "bitcoin price today");
        } else {
            panic!("Expected Search action");
        }
    }

    #[test]
    fn test_parse_respond_with_sensitivity() {
        let json = r#"{"actions": [{"type": "respond", "sensitivity": "sensitive"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();

        if let OrchestratorAction::Respond { sensitivity, .. } = &plan.actions[0] {
            assert_eq!(*sensitivity, Sensitivity::Sensitive);
        } else {
            panic!("Expected Respond action");
        }
    }

    #[test]
    fn test_parse_respond_default_sensitivity() {
        let json = r#"{"actions": [{"type": "respond"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();

        if let OrchestratorAction::Respond { sensitivity, .. } = &plan.actions[0] {
            assert_eq!(*sensitivity, Sensitivity::Insensitive); // default
        } else {
            panic!("Expected Respond action");
        }
    }

    #[test]
    fn test_parse_grok_action() {
        let json = r#"{"actions": [{"type": "grok", "query": "what's trending?"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_direct_grok());

        if let OrchestratorAction::Grok { query, .. } = &plan.actions[0] {
            assert_eq!(query, "what's trending?");
        } else {
            panic!("Expected Grok action");
        }
    }

    #[test]
    fn test_parse_maple_action() {
        let json = r#"{"actions": [{"type": "maple", "query": "private question"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_direct_maple());

        if let OrchestratorAction::Maple { query, .. } = &plan.actions[0] {
            assert_eq!(query, "private question");
        } else {
            panic!("Expected Maple action");
        }
    }

    #[test]
    fn test_parse_set_preference() {
        let json = r#"{"actions": [{"type": "set_preference", "preference": "prefer_speed"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_set_preference());

        if let OrchestratorAction::SetPreference { preference } = &plan.actions[0] {
            assert_eq!(preference, "prefer_speed");
            let pref = UserPreference::from_str(preference);
            assert_eq!(pref, UserPreference::PreferSpeed);
        } else {
            panic!("Expected SetPreference action");
        }
    }

    #[test]
    fn test_parse_search_with_message() {
        let json = r#"{
            "actions": [
                {"type": "search", "query": "weather NYC", "message": "Let me check the forecast..."},
                {"type": "respond", "sensitivity": "insensitive"}
            ]
        }"#;

        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        if let OrchestratorAction::Search { query, message } = &plan.actions[0] {
            assert_eq!(query, "weather NYC");
            assert_eq!(message.as_deref(), Some("Let me check the forecast..."));
        } else {
            panic!("Expected Search action");
        }
    }

    #[test]
    fn test_parse_clear_context() {
        let json = r#"{"actions": [{"type": "clear_context"}]}"#;

        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_clear_context());
    }

    #[test]
    fn test_parse_clear_context_with_message() {
        let json = r#"{"actions": [{"type": "clear_context", "message": "Fresh start!"}]}"#;

        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_clear_context());

        if let OrchestratorAction::ClearContext { message } = &plan.actions[0] {
            assert_eq!(message.as_deref(), Some("Fresh start!"));
        } else {
            panic!("Expected ClearContext action");
        }
    }

    #[test]
    fn test_parse_skip() {
        let json = r#"{"actions": [{"type": "skip", "reason": "not meant for bot"}]}"#;

        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert_eq!(plan.actions.len(), 1);

        if let OrchestratorAction::Skip { reason } = &plan.actions[0] {
            assert_eq!(reason, "not meant for bot");
        } else {
            panic!("Expected Skip action");
        }
    }

    #[test]
    fn test_parse_ignore() {
        let json = r#"{"actions": [{"type": "ignore"}]}"#;

        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.is_ignore());
    }

    #[test]
    fn test_serialize_routing_plan() {
        let plan = RoutingPlan::new(vec![
            OrchestratorAction::search("test query"),
            OrchestratorAction::respond(Sensitivity::Insensitive),
        ]);

        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("search"));
        assert!(json.contains("respond"));
    }

    #[test]
    fn test_respond_only() {
        let plan = RoutingPlan::respond_only();
        assert_eq!(plan.actions.len(), 1);
        assert!(!plan.has_search());
    }

    #[test]
    fn test_sensitivity_prefers_maple() {
        assert!(Sensitivity::Sensitive.prefers_maple());
        assert!(Sensitivity::Uncertain.prefers_maple());
        assert!(!Sensitivity::Insensitive.prefers_maple());
    }

    #[test]
    fn test_sensitivity_allows_grok() {
        assert!(Sensitivity::Insensitive.allows_grok());
        assert!(!Sensitivity::Sensitive.allows_grok());
        assert!(!Sensitivity::Uncertain.allows_grok());
    }

    #[test]
    fn test_user_preference_from_str() {
        assert_eq!(
            UserPreference::from_str("prefer_privacy"),
            UserPreference::PreferPrivacy
        );
        assert_eq!(
            UserPreference::from_str("prefer_speed"),
            UserPreference::PreferSpeed
        );
        assert_eq!(UserPreference::from_str("default"), UserPreference::Default);
        assert_eq!(UserPreference::from_str("grok"), UserPreference::PreferSpeed);
        assert_eq!(
            UserPreference::from_str("maple"),
            UserPreference::PreferPrivacy
        );
    }

    #[test]
    fn test_task_hint_default() {
        assert_eq!(TaskHint::default(), TaskHint::General);
    }

    #[test]
    fn test_parse_respond_with_task_hint() {
        let json = r#"{"actions": [{"type": "respond", "sensitivity": "insensitive", "task_hint": "coding"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();

        if let OrchestratorAction::Respond {
            sensitivity,
            task_hint,
            ..
        } = &plan.actions[0]
        {
            assert_eq!(*sensitivity, Sensitivity::Insensitive);
            assert_eq!(*task_hint, TaskHint::Coding);
        } else {
            panic!("Expected Respond action");
        }
    }

    #[test]
    fn test_parse_respond_default_task_hint() {
        let json = r#"{"actions": [{"type": "respond", "sensitivity": "insensitive"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();

        if let OrchestratorAction::Respond { task_hint, .. } = &plan.actions[0] {
            assert_eq!(*task_hint, TaskHint::General); // default
        } else {
            panic!("Expected Respond action");
        }
    }

    #[test]
    fn test_parse_grok_with_task_hint() {
        let json = r#"{"actions": [{"type": "grok", "query": "help me code", "task_hint": "coding"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();

        if let OrchestratorAction::Grok { query, task_hint } = &plan.actions[0] {
            assert_eq!(query, "help me code");
            assert_eq!(*task_hint, TaskHint::Coding);
        } else {
            panic!("Expected Grok action");
        }
    }

    #[test]
    fn test_parse_maple_with_task_hint() {
        let json = r#"{"actions": [{"type": "maple", "query": "translate this", "task_hint": "multilingual"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();

        if let OrchestratorAction::Maple { query, task_hint } = &plan.actions[0] {
            assert_eq!(query, "translate this");
            assert_eq!(*task_hint, TaskHint::Multilingual);
        } else {
            panic!("Expected Maple action");
        }
    }

    #[test]
    fn test_all_task_hints_parse() {
        let hints = ["general", "coding", "math", "creative", "multilingual", "quick", "vision", "about_bot"];
        let expected = [
            TaskHint::General,
            TaskHint::Coding,
            TaskHint::Math,
            TaskHint::Creative,
            TaskHint::Multilingual,
            TaskHint::Quick,
            TaskHint::Vision,
            TaskHint::AboutBot,
        ];

        for (hint_str, expected_hint) in hints.iter().zip(expected.iter()) {
            let json = format!(
                r#"{{"actions": [{{"type": "respond", "sensitivity": "insensitive", "task_hint": "{}"}}]}}"#,
                hint_str
            );
            let plan: RoutingPlan = serde_json::from_str(&json).unwrap();

            if let OrchestratorAction::Respond { task_hint, .. } = &plan.actions[0] {
                assert_eq!(task_hint, expected_hint, "Failed for hint: {}", hint_str);
            } else {
                panic!("Expected Respond action");
            }
        }
    }

    #[test]
    fn test_action_task_hint_method() {
        let respond = OrchestratorAction::respond_with_hint(Sensitivity::Insensitive, TaskHint::Coding);
        assert_eq!(respond.task_hint(), Some(TaskHint::Coding));

        let grok = OrchestratorAction::grok_with_hint("test", TaskHint::Math);
        assert_eq!(grok.task_hint(), Some(TaskHint::Math));

        let maple = OrchestratorAction::maple_with_hint("test", TaskHint::Creative);
        assert_eq!(maple.task_hint(), Some(TaskHint::Creative));

        let search = OrchestratorAction::search("test");
        assert_eq!(search.task_hint(), None);

        let help = OrchestratorAction::Help;
        assert_eq!(help.task_hint(), None);
    }

    #[test]
    fn test_respond_with_hint_plan() {
        let plan = RoutingPlan::respond_with_hint(Sensitivity::Sensitive, TaskHint::Math);
        assert_eq!(plan.actions.len(), 1);

        if let OrchestratorAction::Respond {
            sensitivity,
            task_hint,
            has_pii,
            pii_types,
        } = &plan.actions[0]
        {
            assert_eq!(*sensitivity, Sensitivity::Sensitive);
            assert_eq!(*task_hint, TaskHint::Math);
            assert!(!has_pii);
            assert!(pii_types.is_empty());
        } else {
            panic!("Expected Respond action");
        }
    }

    #[test]
    fn test_parse_respond_with_pii() {
        let json = r#"{"actions": [{"type": "respond", "sensitivity": "sensitive", "task_hint": "general", "has_pii": true, "pii_types": ["name", "ssn"]}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();

        if let OrchestratorAction::Respond {
            sensitivity,
            task_hint,
            has_pii,
            pii_types,
        } = &plan.actions[0]
        {
            assert_eq!(*sensitivity, Sensitivity::Sensitive);
            assert_eq!(*task_hint, TaskHint::General);
            assert!(*has_pii);
            assert_eq!(pii_types, &vec!["name".to_string(), "ssn".to_string()]);
        } else {
            panic!("Expected Respond action");
        }
    }

    #[test]
    fn test_respond_with_pii_helper() {
        let pii_types = vec!["medical".to_string(), "address".to_string()];
        let action =
            OrchestratorAction::respond_with_pii(Sensitivity::Sensitive, TaskHint::General, pii_types.clone());

        assert!(action.has_pii());
        assert_eq!(action.pii_types(), Some(pii_types.as_slice()));
    }

    #[test]
    fn test_respond_without_pii_default() {
        let json = r#"{"actions": [{"type": "respond", "sensitivity": "insensitive"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();

        // has_pii should default to false
        assert!(!plan.actions[0].has_pii());
    }

    #[test]
    fn test_parse_use_tool() {
        let json = r#"{"actions": [{"type": "use_tool", "name": "calculator", "args": {"expression": "2+2"}}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_use_tool());

        if let OrchestratorAction::UseTool { name, args, message } = &plan.actions[0] {
            assert_eq!(name, "calculator");
            assert_eq!(args.get("expression").unwrap().as_str(), Some("2+2"));
            assert!(message.is_none());
        } else {
            panic!("Expected UseTool action");
        }
    }

    #[test]
    fn test_parse_use_tool_with_message() {
        let json = r#"{"actions": [{"type": "use_tool", "name": "weather", "args": {"location": "NYC"}, "message": "Checking weather..."}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();

        if let OrchestratorAction::UseTool { name, args, message } = &plan.actions[0] {
            assert_eq!(name, "weather");
            assert_eq!(args.get("location").unwrap().as_str(), Some("NYC"));
            assert_eq!(message.as_deref(), Some("Checking weather..."));
        } else {
            panic!("Expected UseTool action");
        }
    }

    #[test]
    fn test_use_tool_helper() {
        let mut args = HashMap::new();
        args.insert("url".to_string(), Value::String("https://example.com".to_string()));

        let action = OrchestratorAction::use_tool("web_fetch", args);

        if let OrchestratorAction::UseTool { name, args, message } = action {
            assert_eq!(name, "web_fetch");
            assert!(args.contains_key("url"));
            assert!(message.is_none());
        } else {
            panic!("Expected UseTool action");
        }
    }

    #[test]
    fn test_parse_ask_privacy_choice() {
        let json = r#"{"actions": [{"type": "ask_privacy_choice", "pii_types": ["name", "ssn"], "original_message": "My name is John and my SSN is 123-45-6789", "sensitivity": "sensitive", "task_hint": "general"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_ask_privacy_choice());
        assert!(plan.has_pii());

        if let OrchestratorAction::AskPrivacyChoice {
            pii_types,
            original_message,
            sensitivity,
            task_hint,
        } = &plan.actions[0]
        {
            assert_eq!(pii_types, &vec!["name".to_string(), "ssn".to_string()]);
            assert_eq!(original_message, "My name is John and my SSN is 123-45-6789");
            assert_eq!(*sensitivity, Sensitivity::Sensitive);
            assert_eq!(*task_hint, TaskHint::General);
        } else {
            panic!("Expected AskPrivacyChoice action");
        }
    }

    #[test]
    fn test_ask_privacy_choice_helper() {
        let pii_types = vec!["medical".to_string(), "income".to_string()];
        let action = OrchestratorAction::ask_privacy_choice(
            pii_types.clone(),
            "I have diabetes and make $100k",
            Sensitivity::Sensitive,
            TaskHint::General,
        );

        assert!(action.has_pii());
        assert_eq!(action.pii_types(), Some(pii_types.as_slice()));
        assert_eq!(action.task_hint(), Some(TaskHint::General));

        if let OrchestratorAction::AskPrivacyChoice {
            original_message, ..
        } = action
        {
            assert_eq!(original_message, "I have diabetes and make $100k");
        } else {
            panic!("Expected AskPrivacyChoice action");
        }
    }

    #[test]
    fn test_ask_privacy_choice_description() {
        let action = OrchestratorAction::ask_privacy_choice(
            vec!["phone".to_string(), "email".to_string()],
            "Call me at 555-1234",
            Sensitivity::Insensitive,
            TaskHint::Quick,
        );

        let desc = action.description();
        assert!(desc.contains("Ask privacy choice"));
        assert!(desc.contains("phone"));
        assert!(desc.contains("email"));
    }

    #[test]
    fn test_plan_has_pii() {
        // Plan with PII in respond
        let json = r#"{"actions": [{"type": "respond", "sensitivity": "sensitive", "has_pii": true, "pii_types": ["name"]}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_pii());

        // Plan without PII
        let json = r#"{"actions": [{"type": "respond", "sensitivity": "insensitive"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(!plan.has_pii());

        // Plan with ask_privacy_choice (always has PII)
        let json = r#"{"actions": [{"type": "ask_privacy_choice", "pii_types": ["ssn"], "original_message": "test"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_pii());
    }

    #[test]
    fn test_privacy_choice_from_input() {
        // Sanitize options
        assert_eq!(PrivacyChoice::from_input("1"), Some(PrivacyChoice::Sanitize));
        assert_eq!(PrivacyChoice::from_input("sanitize"), Some(PrivacyChoice::Sanitize));
        assert_eq!(PrivacyChoice::from_input("SANITIZE"), Some(PrivacyChoice::Sanitize));
        assert_eq!(PrivacyChoice::from_input("remove"), Some(PrivacyChoice::Sanitize));

        // Private options
        assert_eq!(PrivacyChoice::from_input("2"), Some(PrivacyChoice::Private));
        assert_eq!(PrivacyChoice::from_input("private"), Some(PrivacyChoice::Private));
        assert_eq!(PrivacyChoice::from_input("secure"), Some(PrivacyChoice::Private));
        assert_eq!(PrivacyChoice::from_input("maple"), Some(PrivacyChoice::Private));

        // FastUncensored options
        assert_eq!(PrivacyChoice::from_input("3"), Some(PrivacyChoice::FastUncensored));
        assert_eq!(PrivacyChoice::from_input("fast"), Some(PrivacyChoice::FastUncensored));
        assert_eq!(PrivacyChoice::from_input("uncensored"), Some(PrivacyChoice::FastUncensored));
        assert_eq!(PrivacyChoice::from_input("grok"), Some(PrivacyChoice::FastUncensored));

        // Cancel options
        assert_eq!(PrivacyChoice::from_input("4"), Some(PrivacyChoice::Cancel));
        assert_eq!(PrivacyChoice::from_input("cancel"), Some(PrivacyChoice::Cancel));
        assert_eq!(PrivacyChoice::from_input("no"), Some(PrivacyChoice::Cancel));
        assert_eq!(PrivacyChoice::from_input("nevermind"), Some(PrivacyChoice::Cancel));

        // Invalid
        assert_eq!(PrivacyChoice::from_input("hello"), None);
        assert_eq!(PrivacyChoice::from_input("5"), None);
    }

    #[test]
    fn test_parse_privacy_choice_response() {
        let json = r#"{"actions": [{"type": "privacy_choice_response", "choice": "sanitize"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_privacy_choice_response());

        if let OrchestratorAction::PrivacyChoiceResponse { choice } = &plan.actions[0] {
            assert_eq!(*choice, PrivacyChoice::Sanitize);
        } else {
            panic!("Expected PrivacyChoiceResponse action");
        }
    }

    #[test]
    fn test_privacy_choice_response_helper() {
        let action = OrchestratorAction::privacy_choice_response(PrivacyChoice::Private);

        if let OrchestratorAction::PrivacyChoiceResponse { choice } = action {
            assert_eq!(choice, PrivacyChoice::Private);
        } else {
            panic!("Expected PrivacyChoiceResponse action");
        }
    }

    #[test]
    fn test_privacy_choice_response_description() {
        let action = OrchestratorAction::privacy_choice_response(PrivacyChoice::Cancel);
        let desc = action.description();
        assert!(desc.contains("Privacy choice response"));
        assert!(desc.contains("cancel"));
    }

    #[test]
    fn test_all_privacy_choices_parse() {
        let choices = ["sanitize", "private", "fast_uncensored", "cancel"];
        let expected = [PrivacyChoice::Sanitize, PrivacyChoice::Private, PrivacyChoice::FastUncensored, PrivacyChoice::Cancel];

        for (choice_str, expected_choice) in choices.iter().zip(expected.iter()) {
            let json = format!(
                r#"{{"actions": [{{"type": "privacy_choice_response", "choice": "{}"}}]}}"#,
                choice_str
            );
            let plan: RoutingPlan = serde_json::from_str(&json).unwrap();

            if let OrchestratorAction::PrivacyChoiceResponse { choice } = &plan.actions[0] {
                assert_eq!(choice, expected_choice, "Failed for choice: {}", choice_str);
            } else {
                panic!("Expected PrivacyChoiceResponse action");
            }
        }
    }

    #[test]
    fn test_parse_send_email() {
        let json = r#"{"actions": [{"type": "send_email", "recipient": "test@example.com"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_send_email());

        if let OrchestratorAction::SendEmail {
            recipient,
            subject,
            body,
        } = &plan.actions[0]
        {
            assert_eq!(recipient, "test@example.com");
            assert!(subject.is_none());
            assert!(body.is_none());
        } else {
            panic!("Expected SendEmail action");
        }
    }

    #[test]
    fn test_parse_send_email_with_subject() {
        let json = r#"{"actions": [{"type": "send_email", "recipient": "test@example.com", "subject": "Meeting notes"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_send_email());

        if let OrchestratorAction::SendEmail {
            recipient,
            subject,
            body,
        } = &plan.actions[0]
        {
            assert_eq!(recipient, "test@example.com");
            assert_eq!(subject.as_deref(), Some("Meeting notes"));
            assert!(body.is_none());
        } else {
            panic!("Expected SendEmail action");
        }
    }

    #[test]
    fn test_parse_send_email_full() {
        let json = r#"{"actions": [{"type": "send_email", "recipient": "test@example.com", "subject": "Test", "body": "Hello world"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_send_email());

        if let OrchestratorAction::SendEmail {
            recipient,
            subject,
            body,
        } = &plan.actions[0]
        {
            assert_eq!(recipient, "test@example.com");
            assert_eq!(subject.as_deref(), Some("Test"));
            assert_eq!(body.as_deref(), Some("Hello world"));
        } else {
            panic!("Expected SendEmail action");
        }
    }

    #[test]
    fn test_send_email_helper() {
        let action = OrchestratorAction::send_email("alice@proton.me");

        if let OrchestratorAction::SendEmail {
            recipient,
            subject,
            body,
        } = action
        {
            assert_eq!(recipient, "alice@proton.me");
            assert!(subject.is_none());
            assert!(body.is_none());
        } else {
            panic!("Expected SendEmail action");
        }
    }

    #[test]
    fn test_send_email_with_subject_helper() {
        let action = OrchestratorAction::send_email_with_subject("alice@proton.me", "Hello");

        if let OrchestratorAction::SendEmail {
            recipient,
            subject,
            body,
        } = action
        {
            assert_eq!(recipient, "alice@proton.me");
            assert_eq!(subject.as_deref(), Some("Hello"));
            assert!(body.is_none());
        } else {
            panic!("Expected SendEmail action");
        }
    }

    #[test]
    fn test_send_email_full_helper() {
        let action = OrchestratorAction::send_email_full("alice@proton.me", "Hello", "Message body");

        if let OrchestratorAction::SendEmail {
            recipient,
            subject,
            body,
        } = action
        {
            assert_eq!(recipient, "alice@proton.me");
            assert_eq!(subject.as_deref(), Some("Hello"));
            assert_eq!(body.as_deref(), Some("Message body"));
        } else {
            panic!("Expected SendEmail action");
        }
    }

    #[test]
    fn test_send_email_description() {
        let action = OrchestratorAction::send_email("test@example.com");
        let desc = action.description();
        assert!(desc.contains("Send email to"));
        assert!(desc.contains("test@example.com"));

        let action_with_subject = OrchestratorAction::send_email_with_subject("test@example.com", "Important");
        let desc2 = action_with_subject.description();
        assert!(desc2.contains("Send email to"));
        assert!(desc2.contains("test@example.com"));
        assert!(desc2.contains("Important"));
    }
}
