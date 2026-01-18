//! Preference storage for routing decisions.

use sqlx::SqlitePool;

use crate::models::Preference;
use crate::Result;

/// Create or update a preference entry.
pub async fn upsert_preference(
    pool: &SqlitePool,
    history_key: &str,
    preference: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO preferences (history_key, preference)
        VALUES (?, ?)
        ON CONFLICT(history_key) DO UPDATE SET
            preference = excluded.preference,
            updated_at = datetime('now')
        "#,
    )
    .bind(history_key)
    .bind(preference)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get a preference entry by history key.
pub async fn get_preference(pool: &SqlitePool, history_key: &str) -> Result<Option<Preference>> {
    let record = sqlx::query_as::<_, Preference>(
        r#"
        SELECT history_key, preference, updated_at
        FROM preferences
        WHERE history_key = ?
        "#,
    )
    .bind(history_key)
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Clear a preference entry.
pub async fn clear_preference(pool: &SqlitePool, history_key: &str) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM preferences
        WHERE history_key = ?
        "#,
    )
    .bind(history_key)
    .execute(pool)
    .await?;

    Ok(())
}

/// Clear all preferences.
pub async fn clear_all(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM preferences
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
