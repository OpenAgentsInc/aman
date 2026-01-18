//! User profile management for the orchestrator.

use aman_database::{user_profile, Database, ProfileField, UserProfile, ValidationError};
use aman_database::validation::{validate_bolt12_offer, validate_email, validate_model_length};
use std::fmt;
use tracing::{debug, warn};

use crate::model_selection::MapleModels;

/// Errors that can occur during profile operations.
#[derive(Debug)]
pub enum ProfileError {
    /// Validation error (invalid format).
    Validation(ValidationError),
    /// Database error.
    Database(String),
    /// Profile store not configured (no database).
    NotConfigured,
    /// Invalid model name.
    InvalidModel(String),
    /// Unknown field name.
    UnknownField(String),
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::Validation(e) => write!(f, "{}", e),
            ProfileError::Database(e) => write!(f, "Database error: {}", e),
            ProfileError::NotConfigured => write!(f, "Profile storage is not configured"),
            ProfileError::InvalidModel(msg) => write!(f, "{}", msg),
            ProfileError::UnknownField(field) => {
                write!(f, "Unknown field '{}'. Valid fields: default_model, email, bolt12_offer", field)
            }
        }
    }
}

impl std::error::Error for ProfileError {}

impl From<ValidationError> for ProfileError {
    fn from(e: ValidationError) -> Self {
        ProfileError::Validation(e)
    }
}

impl From<aman_database::DatabaseError> for ProfileError {
    fn from(e: aman_database::DatabaseError) -> Self {
        ProfileError::Database(e.to_string())
    }
}

/// Known Grok models for validation.
const GROK_MODELS: &[&str] = &["grok-4-1-fast", "grok-4-1", "grok-3", "grok-3-mini", "grok-4"];

/// User profile store for managing personal settings.
pub struct ProfileStore {
    database: Option<Database>,
}

impl ProfileStore {
    /// Create a new profile store without database (in-memory only).
    pub fn new() -> Self {
        Self { database: None }
    }

    /// Create a profile store with database persistence.
    pub fn with_database(database: Database) -> Self {
        Self {
            database: Some(database),
        }
    }

    /// Get a user's profile.
    pub async fn get(&self, sender_id: &str) -> Option<UserProfile> {
        let database = self.database.as_ref()?;
        match user_profile::get_profile(database.pool(), sender_id).await {
            Ok(profile) => profile,
            Err(e) => {
                warn!("Failed to get profile for {}: {}", sender_id, e);
                None
            }
        }
    }

    /// Update a profile field.
    ///
    /// Validates the value before storing.
    pub async fn update_field(
        &self,
        sender_id: &str,
        field: ProfileField,
        value: Option<&str>,
    ) -> Result<(), ProfileError> {
        let database = self.database.as_ref().ok_or(ProfileError::NotConfigured)?;

        // Validate the value if present
        if let Some(val) = value {
            match field {
                ProfileField::Email => validate_email(val)?,
                ProfileField::Bolt12Offer => validate_bolt12_offer(val)?,
                ProfileField::DefaultModel => {
                    validate_model_length(val)?;
                    Self::validate_model(val)?;
                }
            }
        }

        user_profile::upsert_profile_field(database.pool(), sender_id, field, value)
            .await
            .map_err(|e| ProfileError::Database(e.to_string()))?;

        debug!(
            "Updated profile field {:?} for {} (has value: {})",
            field,
            sender_id,
            value.is_some()
        );

        Ok(())
    }

    /// Delete a user's entire profile.
    ///
    /// Returns true if a profile was deleted.
    pub async fn delete(&self, sender_id: &str) -> Result<bool, ProfileError> {
        let database = self.database.as_ref().ok_or(ProfileError::NotConfigured)?;

        let deleted = user_profile::delete_profile(database.pool(), sender_id)
            .await
            .map_err(|e| ProfileError::Database(e.to_string()))?;

        if deleted {
            debug!("Deleted profile for {}", sender_id);
        }

        Ok(deleted)
    }

    /// Validate a model name against known models.
    pub fn validate_model(model: &str) -> Result<(), ProfileError> {
        let model_lower = model.to_lowercase();

        // Check Maple model aliases
        let maple_aliases = MapleModels::model_aliases();
        if maple_aliases.iter().any(|alias| alias.eq_ignore_ascii_case(model)) {
            return Ok(());
        }

        // Check full Maple model names
        for (alias, canonical) in MapleModels::available_models() {
            if model_lower == canonical.to_lowercase() || model_lower == *alias {
                return Ok(());
            }
        }

        // Check Grok models
        if GROK_MODELS.iter().any(|m| m.eq_ignore_ascii_case(model)) {
            return Ok(());
        }

        // Build helpful error message
        let available: Vec<String> = maple_aliases
            .iter()
            .map(|s| s.to_string())
            .chain(GROK_MODELS.iter().map(|s| s.to_string()))
            .collect();

        Err(ProfileError::InvalidModel(format!(
            "Unknown model '{}'. Available: {}",
            model,
            available.join(", ")
        )))
    }

    /// Format a profile for display to the user.
    pub fn format_profile(profile: Option<&UserProfile>) -> String {
        match profile {
            None => "No profile settings saved yet.\n\n\
                    Available settings:\n\
                    • default_model - Your preferred AI model\n\
                    • email - Your email address\n\
                    • bolt12_offer - Lightning payment offer"
                .to_string(),
            Some(p) => {
                let mut lines = Vec::new();
                lines.push("Your profile settings:".to_string());
                lines.push(String::new());

                if let Some(ref model) = p.default_model {
                    lines.push(format!("• Default model: {}", model));
                } else {
                    lines.push("• Default model: (not set)".to_string());
                }

                if let Some(ref email) = p.email {
                    lines.push(format!("• Email: {}", email));
                } else {
                    lines.push("• Email: (not set)".to_string());
                }

                if let Some(ref bolt12) = p.bolt12_offer {
                    // Truncate long offers for display
                    let display = if bolt12.len() > 40 {
                        format!("{}...", &bolt12[..40])
                    } else {
                        bolt12.clone()
                    };
                    lines.push(format!("• Bolt 12 offer: {}", display));
                } else {
                    lines.push("• Bolt 12 offer: (not set)".to_string());
                }

                lines.join("\n")
            }
        }
    }

    /// Parse a field name from user input.
    pub fn parse_field(field: &str) -> Result<ProfileField, ProfileError> {
        ProfileField::from_str(field).ok_or_else(|| ProfileError::UnknownField(field.to_string()))
    }
}

impl Default for ProfileStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_model_maple_aliases() {
        assert!(ProfileStore::validate_model("llama").is_ok());
        assert!(ProfileStore::validate_model("deepseek").is_ok());
        assert!(ProfileStore::validate_model("qwen").is_ok());
        assert!(ProfileStore::validate_model("mistral").is_ok());
        assert!(ProfileStore::validate_model("gpt-oss").is_ok());
    }

    #[test]
    fn test_validate_model_grok() {
        assert!(ProfileStore::validate_model("grok-4-1-fast").is_ok());
        assert!(ProfileStore::validate_model("grok-3").is_ok());
        assert!(ProfileStore::validate_model("grok-4").is_ok());
    }

    #[test]
    fn test_validate_model_case_insensitive() {
        assert!(ProfileStore::validate_model("LLAMA").is_ok());
        assert!(ProfileStore::validate_model("Grok-4-1-Fast").is_ok());
    }

    #[test]
    fn test_validate_model_invalid() {
        let result = ProfileStore::validate_model("unknown-model");
        assert!(matches!(result, Err(ProfileError::InvalidModel(_))));
    }

    #[test]
    fn test_format_profile_none() {
        let formatted = ProfileStore::format_profile(None);
        assert!(formatted.contains("No profile settings"));
        assert!(formatted.contains("default_model"));
    }

    #[test]
    fn test_format_profile_partial() {
        let profile = UserProfile {
            sender_id: "+1234567890".to_string(),
            default_model: Some("llama".to_string()),
            email: None,
            bolt12_offer: None,
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-01".to_string(),
        };
        let formatted = ProfileStore::format_profile(Some(&profile));
        assert!(formatted.contains("Default model: llama"));
        assert!(formatted.contains("Email: (not set)"));
    }

    #[test]
    fn test_format_profile_full() {
        let profile = UserProfile {
            sender_id: "+1234567890".to_string(),
            default_model: Some("grok-4-1-fast".to_string()),
            email: Some("test@example.com".to_string()),
            bolt12_offer: Some("lno1qcp4256ypqpq8q2qqqqqq".to_string()),
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-01".to_string(),
        };
        let formatted = ProfileStore::format_profile(Some(&profile));
        assert!(formatted.contains("Default model: grok-4-1-fast"));
        assert!(formatted.contains("Email: test@example.com"));
        assert!(formatted.contains("Bolt 12 offer: lno1qcp4256ypqpq8q2qqqqqq"));
    }

    #[test]
    fn test_parse_field() {
        assert_eq!(
            ProfileStore::parse_field("model").unwrap(),
            ProfileField::DefaultModel
        );
        assert_eq!(
            ProfileStore::parse_field("email").unwrap(),
            ProfileField::Email
        );
        assert_eq!(
            ProfileStore::parse_field("bolt12").unwrap(),
            ProfileField::Bolt12Offer
        );
        assert!(ProfileStore::parse_field("invalid").is_err());
    }

    #[test]
    fn test_profile_error_display() {
        let err = ProfileError::InvalidModel("test".to_string());
        assert_eq!(err.to_string(), "test");

        let err = ProfileError::NotConfigured;
        assert!(err.to_string().contains("not configured"));
    }
}
