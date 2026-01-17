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
