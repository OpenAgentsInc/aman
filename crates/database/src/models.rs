//! Database models.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// A user in the system, identified by their Signal UUID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct User {
    /// Signal UUID (e.g., "c27fb365-0c84-4cf2-8555-814bb065e448")
    pub id: String,
    /// Display name
    pub name: String,
    /// Preferred language (e.g., "Arabic", "English")
    pub language: String,
}

/// A topic that users can subscribe to for notifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Topic {
    /// Unique slug identifier (e.g., "iran", "bitcoin")
    pub slug: String,
}

/// A notification subscription linking a user to a topic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Notification {
    /// Topic slug
    pub topic_slug: String,
    /// User ID (Signal UUID)
    pub user_id: String,
    /// When the subscription was created
    pub created_at: String,
}

/// A stored routing preference for a sender or group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Preference {
    /// History key for sender or group.
    pub history_key: String,
    /// Stored preference value.
    pub preference: String,
    /// Last update timestamp.
    pub updated_at: String,
}

/// A stored conversation summary for a sender or group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct ConversationSummary {
    /// History key for sender or group.
    pub history_key: String,
    /// Rolling summary text.
    pub summary: String,
    /// Number of exchanges summarized.
    pub message_count: i64,
    /// Last update timestamp.
    pub updated_at: String,
}

/// A tool execution record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct ToolHistoryEntry {
    /// Auto-incrementing ID.
    pub id: i64,
    /// History key for sender or group.
    pub history_key: String,
    /// Tool name executed.
    pub tool_name: String,
    /// Whether the tool succeeded.
    pub success: bool,
    /// Tool output content (possibly truncated).
    pub content: String,
    /// Sender ID, if available.
    pub sender_id: Option<String>,
    /// Group ID, if available.
    pub group_id: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
}

/// A clear context event record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct ClearContextEvent {
    /// Auto-incrementing ID.
    pub id: i64,
    /// History key for sender or group.
    pub history_key: String,
    /// Sender ID, if available.
    pub sender_id: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
}
