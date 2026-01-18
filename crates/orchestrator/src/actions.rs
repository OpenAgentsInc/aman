//! Routing plan and action types for orchestration.

use serde::{Deserialize, Serialize};

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
            actions: vec![OrchestratorAction::Respond],
        }
    }

    /// Check if the plan is empty.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Check if the plan contains a search action.
    pub fn has_search(&self) -> bool {
        self.actions.iter().any(|a| matches!(a, OrchestratorAction::Search { .. }))
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
    Respond,

    /// Skip processing (e.g., for system messages).
    Skip {
        /// Reason for skipping.
        reason: String,
    },

    /// Ignore the message silently (accidental/mistaken input like "?" or typos).
    /// Unlike Skip, this produces no response at all.
    Ignore,
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

    /// Create a skip action with the given reason.
    pub fn skip(reason: impl Into<String>) -> Self {
        Self::Skip {
            reason: reason.into(),
        }
    }

    /// Get a human-readable description of this action.
    pub fn description(&self) -> String {
        match self {
            Self::Search { query, .. } => format!("Search: {}", query),
            Self::ClearContext { .. } => "Clear conversation history".to_string(),
            Self::Help => "Show help information".to_string(),
            Self::Respond => "Generate response".to_string(),
            Self::Skip { reason } => format!("Skip: {}", reason),
            Self::Ignore => "Ignore (accidental message)".to_string(),
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
                {"type": "respond"}
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
    fn test_parse_search_with_message() {
        let json = r#"{
            "actions": [
                {"type": "search", "query": "weather NYC", "message": "Let me check the forecast..."},
                {"type": "respond"}
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
            OrchestratorAction::Respond,
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
}
