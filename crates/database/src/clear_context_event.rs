//! Clear context event persistence.

use std::time::Duration;

use sqlx::SqlitePool;

use crate::models::ClearContextEvent;
use crate::Result;

/// Insert a clear context event.
pub async fn insert_event(
    pool: &SqlitePool,
    history_key: &str,
    sender_id: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO clear_context_events (history_key, sender_id)
        VALUES (?, ?)
        "#,
    )
    .bind(history_key)
    .bind(sender_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get recent clear context events for a history key.
pub async fn list_events(
    pool: &SqlitePool,
    history_key: &str,
    limit: i64,
) -> Result<Vec<ClearContextEvent>> {
    let rows = sqlx::query_as::<_, ClearContextEvent>(
        r#"
        SELECT id, history_key, sender_id, created_at
        FROM clear_context_events
        WHERE history_key = ?
        ORDER BY created_at DESC
        LIMIT ?
        "#,
    )
    .bind(history_key)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Prune clear context events older than the specified TTL.
pub async fn prune_older_than(pool: &SqlitePool, ttl: Duration) -> Result<u64> {
    let modifier = format!("-{} seconds", ttl.as_secs());
    let result = sqlx::query(
        r#"
        DELETE FROM clear_context_events
        WHERE created_at < datetime('now', ?)
        "#,
    )
    .bind(modifier)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Prune clear context events to a maximum row count.
pub async fn prune_over_limit(pool: &SqlitePool, max_rows: usize) -> Result<u64> {
    if max_rows == 0 {
        let result = sqlx::query(
            r#"
            DELETE FROM clear_context_events
            "#,
        )
        .execute(pool)
        .await?;
        return Ok(result.rows_affected());
    }

    let result = sqlx::query(
        r#"
        DELETE FROM clear_context_events
        WHERE id NOT IN (
            SELECT id
            FROM clear_context_events
            ORDER BY created_at DESC
            LIMIT ?
        )
        "#,
    )
    .bind(max_rows as i64)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
