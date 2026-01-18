//! User profile storage for personal settings.

use sqlx::SqlitePool;

use crate::models::UserProfile;
use crate::Result;

/// Profile field identifiers for updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileField {
    /// Default model preference.
    DefaultModel,
    /// Email address.
    Email,
    /// Bolt 12 offer for Lightning payments.
    Bolt12Offer,
}

impl ProfileField {
    /// Get the database column name for this field.
    pub fn column_name(&self) -> &'static str {
        match self {
            ProfileField::DefaultModel => "default_model",
            ProfileField::Email => "email",
            ProfileField::Bolt12Offer => "bolt12_offer",
        }
    }

    /// Parse a field name from user input.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "default_model" | "model" => Some(ProfileField::DefaultModel),
            "email" | "e-mail" => Some(ProfileField::Email),
            "bolt12_offer" | "bolt12" | "lightning" => Some(ProfileField::Bolt12Offer),
            _ => None,
        }
    }

    /// Get a human-readable display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            ProfileField::DefaultModel => "default model",
            ProfileField::Email => "email",
            ProfileField::Bolt12Offer => "Bolt 12 offer",
        }
    }
}

/// Get a user's profile.
pub async fn get_profile(pool: &SqlitePool, sender_id: &str) -> Result<Option<UserProfile>> {
    let record = sqlx::query_as::<_, UserProfile>(
        r#"
        SELECT sender_id, default_model, email, bolt12_offer, created_at, updated_at
        FROM user_profiles
        WHERE sender_id = ?
        "#,
    )
    .bind(sender_id)
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Update a single field in a user's profile.
///
/// Creates the profile if it doesn't exist.
/// If value is None, clears the field.
pub async fn upsert_profile_field(
    pool: &SqlitePool,
    sender_id: &str,
    field: ProfileField,
    value: Option<&str>,
) -> Result<()> {
    // Build the query dynamically based on the field.
    // SQLite doesn't support parameterized column names, so we use string formatting
    // but the column name is validated through the ProfileField enum.
    let column = field.column_name();
    let query = format!(
        r#"
        INSERT INTO user_profiles (sender_id, {column})
        VALUES (?, ?)
        ON CONFLICT(sender_id) DO UPDATE SET
            {column} = excluded.{column},
            updated_at = datetime('now')
        "#,
        column = column
    );

    sqlx::query(&query)
        .bind(sender_id)
        .bind(value)
        .execute(pool)
        .await?;

    Ok(())
}

/// Delete a user's entire profile.
///
/// Returns true if a profile was deleted, false if none existed.
pub async fn delete_profile(pool: &SqlitePool, sender_id: &str) -> Result<bool> {
    let result = sqlx::query(
        r#"
        DELETE FROM user_profiles
        WHERE sender_id = ?
        "#,
    )
    .bind(sender_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    async fn test_db() -> Database {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.migrate().await.unwrap();
        db
    }

    #[tokio::test]
    async fn test_get_profile_not_found() {
        let db = test_db().await;
        let profile = get_profile(db.pool(), "+1234567890").await.unwrap();
        assert!(profile.is_none());
    }

    #[tokio::test]
    async fn test_upsert_and_get_profile() {
        let db = test_db().await;
        let sender = "+1234567890";

        // Create with email
        upsert_profile_field(db.pool(), sender, ProfileField::Email, Some("test@example.com"))
            .await
            .unwrap();

        let profile = get_profile(db.pool(), sender).await.unwrap().unwrap();
        assert_eq!(profile.sender_id, sender);
        assert_eq!(profile.email, Some("test@example.com".to_string()));
        assert!(profile.default_model.is_none());
        assert!(profile.bolt12_offer.is_none());
    }

    #[tokio::test]
    async fn test_update_multiple_fields() {
        let db = test_db().await;
        let sender = "+1234567890";

        // Set email
        upsert_profile_field(db.pool(), sender, ProfileField::Email, Some("test@example.com"))
            .await
            .unwrap();

        // Set model
        upsert_profile_field(db.pool(), sender, ProfileField::DefaultModel, Some("llama"))
            .await
            .unwrap();

        // Set bolt12
        upsert_profile_field(
            db.pool(),
            sender,
            ProfileField::Bolt12Offer,
            Some("lno1abc123"),
        )
        .await
        .unwrap();

        let profile = get_profile(db.pool(), sender).await.unwrap().unwrap();
        assert_eq!(profile.email, Some("test@example.com".to_string()));
        assert_eq!(profile.default_model, Some("llama".to_string()));
        assert_eq!(profile.bolt12_offer, Some("lno1abc123".to_string()));
    }

    #[tokio::test]
    async fn test_clear_field() {
        let db = test_db().await;
        let sender = "+1234567890";

        // Set email
        upsert_profile_field(db.pool(), sender, ProfileField::Email, Some("test@example.com"))
            .await
            .unwrap();

        // Clear email
        upsert_profile_field(db.pool(), sender, ProfileField::Email, None)
            .await
            .unwrap();

        let profile = get_profile(db.pool(), sender).await.unwrap().unwrap();
        assert!(profile.email.is_none());
    }

    #[tokio::test]
    async fn test_delete_profile() {
        let db = test_db().await;
        let sender = "+1234567890";

        // Create profile
        upsert_profile_field(db.pool(), sender, ProfileField::Email, Some("test@example.com"))
            .await
            .unwrap();

        // Delete
        let deleted = delete_profile(db.pool(), sender).await.unwrap();
        assert!(deleted);

        // Verify gone
        let profile = get_profile(db.pool(), sender).await.unwrap();
        assert!(profile.is_none());

        // Delete again returns false
        let deleted = delete_profile(db.pool(), sender).await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_profile_field_from_str() {
        assert_eq!(ProfileField::from_str("model"), Some(ProfileField::DefaultModel));
        assert_eq!(ProfileField::from_str("default_model"), Some(ProfileField::DefaultModel));
        assert_eq!(ProfileField::from_str("email"), Some(ProfileField::Email));
        assert_eq!(ProfileField::from_str("e-mail"), Some(ProfileField::Email));
        assert_eq!(ProfileField::from_str("bolt12"), Some(ProfileField::Bolt12Offer));
        assert_eq!(ProfileField::from_str("lightning"), Some(ProfileField::Bolt12Offer));
        assert_eq!(ProfileField::from_str("invalid"), None);
    }
}
