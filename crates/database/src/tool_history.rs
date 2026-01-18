//! Tool history persistence.

use std::time::Duration;

use sqlx::SqlitePool;

use crate::models::ToolHistoryEntry;
use crate::Result;

/// Insert a tool history entry.
pub async fn insert_tool_history(
    pool: &SqlitePool,
    history_key: &str,
    tool_name: &str,
    success: bool,
    content: &str,
    sender_id: Option<&str>,
    group_id: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO tool_history (history_key, tool_name, success, content, sender_id, group_id)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(history_key)
    .bind(tool_name)
    .bind(success)
    .bind(content)
    .bind(sender_id)
    .bind(group_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get recent tool history entries for a history key.
pub async fn list_tool_history(
    pool: &SqlitePool,
    history_key: &str,
    limit: i64,
) -> Result<Vec<ToolHistoryEntry>> {
    let rows = sqlx::query_as::<_, ToolHistoryEntry>(
        r#"
        SELECT id, history_key, tool_name, success, content, sender_id, group_id, created_at
        FROM tool_history
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

/// Prune tool history older than the specified TTL.
pub async fn prune_older_than(pool: &SqlitePool, ttl: Duration) -> Result<u64> {
    let modifier = format!("-{} seconds", ttl.as_secs());
    let result = sqlx::query(
        r#"
        DELETE FROM tool_history
        WHERE created_at < datetime('now', ?)
        "#,
    )
    .bind(modifier)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Prune tool history to a maximum row count.
pub async fn prune_over_limit(pool: &SqlitePool, max_rows: usize) -> Result<u64> {
    if max_rows == 0 {
        let result = sqlx::query(
            r#"
            DELETE FROM tool_history
            "#,
        )
        .execute(pool)
        .await?;
        return Ok(result.rows_affected());
    }

    let result = sqlx::query(
        r#"
        DELETE FROM tool_history
        WHERE id NOT IN (
            SELECT id
            FROM tool_history
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

/// Prune tool history for a specific history key to a maximum row count.
pub async fn prune_over_limit_for_key(
    pool: &SqlitePool,
    history_key: &str,
    max_rows: usize,
) -> Result<u64> {
    if max_rows == 0 {
        let result = sqlx::query(
            r#"
            DELETE FROM tool_history
            WHERE history_key = ?
            "#,
        )
        .bind(history_key)
        .execute(pool)
        .await?;
        return Ok(result.rows_affected());
    }

    let result = sqlx::query(
        r#"
        DELETE FROM tool_history
        WHERE id IN (
            SELECT id
            FROM tool_history
            WHERE history_key = ?
            ORDER BY created_at DESC
            LIMIT -1 OFFSET ?
        )
        "#,
    )
    .bind(history_key)
    .bind(max_rows as i64)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
