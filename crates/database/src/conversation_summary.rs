//! Conversation summary persistence.

use std::time::Duration;

use sqlx::SqlitePool;

use crate::models::ConversationSummary;
use crate::Result;

/// Create or update a conversation summary.
pub async fn upsert_summary(
    pool: &SqlitePool,
    history_key: &str,
    summary: &str,
    message_count: i64,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO conversation_summaries (history_key, summary, message_count)
        VALUES (?, ?, ?)
        ON CONFLICT(history_key) DO UPDATE SET
            summary = excluded.summary,
            message_count = excluded.message_count,
            updated_at = datetime('now')
        "#,
    )
    .bind(history_key)
    .bind(summary)
    .bind(message_count)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get a conversation summary for a history key.
pub async fn get_summary(
    pool: &SqlitePool,
    history_key: &str,
) -> Result<Option<ConversationSummary>> {
    let record = sqlx::query_as::<_, ConversationSummary>(
        r#"
        SELECT history_key, summary, message_count, updated_at
        FROM conversation_summaries
        WHERE history_key = ?
        "#,
    )
    .bind(history_key)
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Clear a conversation summary.
pub async fn clear_summary(pool: &SqlitePool, history_key: &str) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM conversation_summaries
        WHERE history_key = ?
        "#,
    )
    .bind(history_key)
    .execute(pool)
    .await?;

    Ok(())
}

/// Prune summaries older than the specified TTL.
pub async fn prune_older_than(pool: &SqlitePool, ttl: Duration) -> Result<u64> {
    let modifier = format!("-{} seconds", ttl.as_secs());
    let result = sqlx::query(
        r#"
        DELETE FROM conversation_summaries
        WHERE updated_at < datetime('now', ?)
        "#,
    )
    .bind(modifier)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Prune summaries to a maximum row count.
pub async fn prune_over_limit(pool: &SqlitePool, max_rows: usize) -> Result<u64> {
    if max_rows == 0 {
        let result = sqlx::query(
            r#"
            DELETE FROM conversation_summaries
            "#,
        )
        .execute(pool)
        .await?;
        return Ok(result.rows_affected());
    }

    let result = sqlx::query(
        r#"
        DELETE FROM conversation_summaries
        WHERE history_key NOT IN (
            SELECT history_key
            FROM conversation_summaries
            ORDER BY updated_at DESC
            LIMIT ?
        )
        "#,
    )
    .bind(max_rows as i64)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
