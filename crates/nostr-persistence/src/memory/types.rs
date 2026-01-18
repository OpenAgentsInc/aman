use serde::{Deserialize, Serialize};

use crate::events::NostrTag;

pub const MEMORY_SCHEMA_VERSION: u32 = 1;

pub const KIND_AMAN_PREFERENCE: u16 = 30093;
pub const KIND_AMAN_SUMMARY: u16 = 30094;
pub const KIND_AMAN_TOOL_HISTORY: u16 = 30095;
pub const KIND_AMAN_CLEAR_CONTEXT: u16 = 30096;
pub const KIND_AMAN_SUBSCRIPTION_STATE: u16 = 30097;

pub const TAG_KIND_AMAN_PREFERENCE: &str = "aman_preference";
pub const TAG_KIND_AMAN_SUMMARY: &str = "aman_summary";
pub const TAG_KIND_AMAN_TOOL_HISTORY: &str = "aman_tool_history";
pub const TAG_KIND_AMAN_CLEAR_CONTEXT: &str = "aman_clear_context";
pub const TAG_KIND_AMAN_SUBSCRIPTION_STATE: &str = "aman_subscription_state";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmanPreferenceEvent {
    pub history_key: String,
    pub preference: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmanSummaryEvent {
    pub history_key: String,
    pub summary: String,
    pub message_count: i64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmanToolHistoryEvent {
    pub history_key: String,
    pub tool_name: String,
    pub success: bool,
    pub content: String,
    pub sender_id: Option<String>,
    pub group_id: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmanClearContextEvent {
    pub history_key: String,
    pub sender_id: Option<String>,
    pub created_at: u64,
}

pub fn hk_tag(value: &str) -> NostrTag {
    NostrTag::new("hk", vec![value.to_string()])
}

pub fn v_tag(value: u32) -> NostrTag {
    NostrTag::new("v", vec![value.to_string()])
}

pub fn ts_tag(value: u64) -> NostrTag {
    NostrTag::new("ts", vec![value.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_roundtrip() {
        let event = AmanPreferenceEvent {
            history_key: "hk".to_string(),
            preference: "opt-in".to_string(),
            updated_at: 1700000000,
        };
        let json = serde_json::to_vec(&event).unwrap();
        let parsed: AmanPreferenceEvent = serde_json::from_slice(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn summary_roundtrip() {
        let event = AmanSummaryEvent {
            history_key: "hk".to_string(),
            summary: "Summary".to_string(),
            message_count: 3,
            updated_at: 1700000001,
        };
        let json = serde_json::to_vec(&event).unwrap();
        let parsed: AmanSummaryEvent = serde_json::from_slice(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn tool_history_roundtrip() {
        let event = AmanToolHistoryEvent {
            history_key: "hk".to_string(),
            tool_name: "weather".to_string(),
            success: true,
            content: "sunny".to_string(),
            sender_id: Some("user-1".to_string()),
            group_id: None,
            created_at: 1700000002,
        };
        let json = serde_json::to_vec(&event).unwrap();
        let parsed: AmanToolHistoryEvent = serde_json::from_slice(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn clear_context_roundtrip() {
        let event = AmanClearContextEvent {
            history_key: "hk".to_string(),
            sender_id: None,
            created_at: 1700000003,
        };
        let json = serde_json::to_vec(&event).unwrap();
        let parsed: AmanClearContextEvent = serde_json::from_slice(&json).unwrap();
        assert_eq!(event, parsed);
    }
}
