//! Routing plan and action types for orchestration.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Sensitivity level for a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    /// Sensitive content - use privacy-preserving mode (Maple TEE).
    /// Examples: health, finances, legal, personal info, relationships.
    Sensitive,

    /// Insensitive content - can use fast mode (Grok).
    /// Examples: weather, news, sports, general knowledge, coding.
    #[default]
    Insensitive,

    /// Uncertain - could go either way, follow user preference.
    Uncertain,
}

impl Sensitivity {
    /// Check if this sensitivity level should use Maple (privacy mode).
    pub fn prefers_maple(&self) -> bool {
        matches!(self, Sensitivity::Sensitive | Sensitivity::Uncertain)
    }

    /// Check if this sensitivity level can use Grok (fast mode).
    pub fn allows_grok(&self) -> bool {
        matches!(self, Sensitivity::Insensitive)
    }
}

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

/// Task hint for model selection.
///
/// The router classifies the type of task to help select the best model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskHint {
    /// General conversation and questions (default).
    #[default]
    General,

    /// Programming and technical development tasks.
    /// Best models: qwen3-coder-480b, deepseek-r1, grok-3
    Coding,

    /// Mathematical and analytical reasoning.
    /// Best models: deepseek-r1-0528
    Math,

    /// Creative writing and content generation.
    /// Best models: gpt-oss-120b
    Creative,

    /// Non-English or translation tasks.
    /// Best models: qwen2-5-72b
    Multilingual,

    /// Simple queries needing fast responses.
    /// Best models: grok-3-mini, mistral-small-3-1-24b
    Quick,

    /// Image/vision analysis tasks.
    /// Best models: qwen3-vl-30b (Maple only - Grok has no vision support)
    /// Note: Vision tasks MUST use Maple regardless of sensitivity.
    Vision,
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
            }],
        }
    }

    /// Create a simple plan with just a respond action with specific sensitivity.
    pub fn respond_with_sensitivity(sensitivity: Sensitivity) -> Self {
        Self {
            actions: vec![OrchestratorAction::Respond {
                sensitivity,
                task_hint: TaskHint::default(),
            }],
        }
    }

    /// Create a simple plan with respond action with sensitivity and task hint.
    pub fn respond_with_hint(sensitivity: Sensitivity, task_hint: TaskHint) -> Self {
        Self {
            actions: vec![OrchestratorAction::Respond {
                sensitivity,
                task_hint,
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
        }
    }

    /// Create a respond action with sensitivity and task hint.
    pub fn respond_with_hint(sensitivity: Sensitivity, task_hint: TaskHint) -> Self {
        Self::Respond {
            sensitivity,
            task_hint,
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

    /// Get a human-readable description of this action.
    pub fn description(&self) -> String {
        match self {
            Self::Search { query, .. } => format!("Search: {}", query),
            Self::ClearContext { .. } => "Clear conversation history".to_string(),
            Self::Help => "Show help information".to_string(),
            Self::Respond {
                sensitivity,
                task_hint,
            } => format!("Generate response ({:?}, {:?})", sensitivity, task_hint),
            Self::Grok { query, task_hint } => {
                format!("Direct Grok ({:?}): {}", task_hint, query)
            }
            Self::Maple { query, task_hint } => {
                format!("Direct Maple ({:?}): {}", task_hint, query)
            }
            Self::SetPreference { preference } => format!("Set preference: {}", preference),
            Self::Skip { reason } => format!("Skip: {}", reason),
            Self::Ignore => "Ignore (accidental message)".to_string(),
            Self::UseTool { name, args, .. } => {
                format!("Use tool '{}' with {} args", name, args.len())
            }
        }
    }

    /// Get the task hint from this action, if it has one.
    pub fn task_hint(&self) -> Option<TaskHint> {
        match self {
            Self::Respond { task_hint, .. } => Some(*task_hint),
            Self::Grok { task_hint, .. } => Some(*task_hint),
            Self::Maple { task_hint, .. } => Some(*task_hint),
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
        let hints = ["general", "coding", "math", "creative", "multilingual", "quick"];
        let expected = [
            TaskHint::General,
            TaskHint::Coding,
            TaskHint::Math,
            TaskHint::Creative,
            TaskHint::Multilingual,
            TaskHint::Quick,
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
        } = &plan.actions[0]
        {
            assert_eq!(*sensitivity, Sensitivity::Sensitive);
            assert_eq!(*task_hint, TaskHint::Math);
        } else {
            panic!("Expected Respond action");
        }
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
}
