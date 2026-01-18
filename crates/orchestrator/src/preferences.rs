//! User preference storage for agent selection.

use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::actions::{Sensitivity, UserPreference};

/// In-memory storage for user preferences.
///
/// Thread-safe storage that maps sender IDs to their agent preferences.
/// Preferences are stored in memory and lost on restart (MVP behavior).
pub struct PreferenceStore {
    preferences: RwLock<HashMap<String, UserPreference>>,
}

impl Default for PreferenceStore {
    fn default() -> Self {
        Self::new()
    }
}

impl PreferenceStore {
    /// Create a new empty preference store.
    pub fn new() -> Self {
        Self {
            preferences: RwLock::new(HashMap::new()),
        }
    }

    /// Get the preference for a sender, or default if not set.
    pub async fn get(&self, sender: &str) -> UserPreference {
        self.preferences
            .read()
            .await
            .get(sender)
            .copied()
            .unwrap_or_default()
    }

    /// Set the preference for a sender.
    pub async fn set(&self, sender: &str, preference: UserPreference) {
        self.preferences
            .write()
            .await
            .insert(sender.to_string(), preference);
    }

    /// Clear the preference for a sender (reset to default).
    pub async fn clear(&self, sender: &str) {
        self.preferences.write().await.remove(sender);
    }

    /// Clear all preferences.
    pub async fn clear_all(&self) {
        self.preferences.write().await.clear();
    }

    /// Determine which agent to use based on sensitivity and user preference.
    ///
    /// Returns `true` if Grok should be used, `false` if Maple should be used.
    pub async fn should_use_grok(&self, sender: &str, sensitivity: Sensitivity) -> bool {
        let preference = self.get(sender).await;
        Self::resolve_agent(preference, sensitivity)
    }

    /// Resolve which agent to use given preference and sensitivity.
    ///
    /// Returns `true` for Grok, `false` for Maple.
    pub fn resolve_agent(preference: UserPreference, sensitivity: Sensitivity) -> bool {
        match preference {
            UserPreference::PreferSpeed => {
                // Use Grok for everything except explicitly sensitive content
                !matches!(sensitivity, Sensitivity::Sensitive)
            }
            UserPreference::PreferPrivacy => {
                // Always use Maple
                false
            }
            UserPreference::Default => {
                // Use Grok only for insensitive content
                // Sensitive and uncertain go to Maple
                matches!(sensitivity, Sensitivity::Insensitive)
            }
        }
    }
}

/// Agent indicator for response messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentIndicator {
    /// Privacy-preserving mode (Maple TEE).
    Privacy,
    /// Fast mode (Grok).
    Speed,
}

impl AgentIndicator {
    /// Get a subtle prefix for the response.
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Privacy => "",   // No indicator for privacy (default/expected)
            Self::Speed => "[*] ", // Subtle indicator for speed mode
        }
    }

    /// Get a description for status messages.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Privacy => "privacy mode",
            Self::Speed => "speed mode",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_preference_store_default() {
        let store = PreferenceStore::new();
        let pref = store.get("user1").await;
        assert_eq!(pref, UserPreference::Default);
    }

    #[tokio::test]
    async fn test_preference_store_set_get() {
        let store = PreferenceStore::new();
        store.set("user1", UserPreference::PreferSpeed).await;

        let pref = store.get("user1").await;
        assert_eq!(pref, UserPreference::PreferSpeed);

        // Other users still have default
        let pref2 = store.get("user2").await;
        assert_eq!(pref2, UserPreference::Default);
    }

    #[tokio::test]
    async fn test_preference_store_clear() {
        let store = PreferenceStore::new();
        store.set("user1", UserPreference::PreferPrivacy).await;
        store.clear("user1").await;

        let pref = store.get("user1").await;
        assert_eq!(pref, UserPreference::Default);
    }

    #[test]
    fn test_resolve_agent_default() {
        // Default: Grok for insensitive, Maple for sensitive/uncertain
        assert!(PreferenceStore::resolve_agent(
            UserPreference::Default,
            Sensitivity::Insensitive
        ));
        assert!(!PreferenceStore::resolve_agent(
            UserPreference::Default,
            Sensitivity::Sensitive
        ));
        assert!(!PreferenceStore::resolve_agent(
            UserPreference::Default,
            Sensitivity::Uncertain
        ));
    }

    #[test]
    fn test_resolve_agent_prefer_speed() {
        // Prefer speed: Grok for insensitive and uncertain, Maple only for sensitive
        assert!(PreferenceStore::resolve_agent(
            UserPreference::PreferSpeed,
            Sensitivity::Insensitive
        ));
        assert!(!PreferenceStore::resolve_agent(
            UserPreference::PreferSpeed,
            Sensitivity::Sensitive
        ));
        assert!(PreferenceStore::resolve_agent(
            UserPreference::PreferSpeed,
            Sensitivity::Uncertain
        ));
    }

    #[test]
    fn test_resolve_agent_prefer_privacy() {
        // Prefer privacy: always Maple
        assert!(!PreferenceStore::resolve_agent(
            UserPreference::PreferPrivacy,
            Sensitivity::Insensitive
        ));
        assert!(!PreferenceStore::resolve_agent(
            UserPreference::PreferPrivacy,
            Sensitivity::Sensitive
        ));
        assert!(!PreferenceStore::resolve_agent(
            UserPreference::PreferPrivacy,
            Sensitivity::Uncertain
        ));
    }

    #[tokio::test]
    async fn test_should_use_grok() {
        let store = PreferenceStore::new();

        // Default user with insensitive query -> Grok
        assert!(store.should_use_grok("user1", Sensitivity::Insensitive).await);

        // Default user with sensitive query -> Maple
        assert!(!store.should_use_grok("user1", Sensitivity::Sensitive).await);

        // Speed user with uncertain query -> Grok
        store.set("user2", UserPreference::PreferSpeed).await;
        assert!(store.should_use_grok("user2", Sensitivity::Uncertain).await);

        // Privacy user with insensitive query -> Maple
        store.set("user3", UserPreference::PreferPrivacy).await;
        assert!(!store.should_use_grok("user3", Sensitivity::Insensitive).await);
    }
}
