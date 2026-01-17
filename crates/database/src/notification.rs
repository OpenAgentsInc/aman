//! Notification subscription CRUD operations.

use sqlx::SqlitePool;

use crate::error::{DatabaseError, Result};
use crate::models::{Notification, Topic, User};

/// Subscribe a user to a topic.
pub async fn subscribe(pool: &SqlitePool, user_id: &str, topic_slug: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO notifications (user_id, topic_slug)
        VALUES (?, ?)
        "#,
    )
    .bind(user_id)
    .bind(topic_slug)
    .execute(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.is_unique_violation() {
                return DatabaseError::AlreadyExists {
                    entity: "Subscription",
                    id: format!("{}/{}", user_id, topic_slug),
                };
            }
        }
        DatabaseError::Sqlx(e)
    })?;

    Ok(())
}

/// Unsubscribe a user from a topic.
pub async fn unsubscribe(pool: &SqlitePool, user_id: &str, topic_slug: &str) -> Result<()> {
    let result = sqlx::query(
        r#"
        DELETE FROM notifications
        WHERE user_id = ? AND topic_slug = ?
        "#,
    )
    .bind(user_id)
    .bind(topic_slug)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(DatabaseError::NotFound {
            entity: "Subscription",
            id: format!("{}/{}", user_id, topic_slug),
        });
    }

    Ok(())
}

/// Check if a user is subscribed to a topic.
pub async fn is_subscribed(pool: &SqlitePool, user_id: &str, topic_slug: &str) -> Result<bool> {
    let result = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT 1
        FROM notifications
        WHERE user_id = ? AND topic_slug = ?
        "#,
    )
    .bind(user_id)
    .bind(topic_slug)
    .fetch_optional(pool)
    .await?;

    Ok(result.is_some())
}

/// Get all topics a user is subscribed to.
pub async fn get_user_subscriptions(pool: &SqlitePool, user_id: &str) -> Result<Vec<Topic>> {
    let topics = sqlx::query_as::<_, Topic>(
        r#"
        SELECT t.slug
        FROM topics t
        INNER JOIN notifications n ON n.topic_slug = t.slug
        WHERE n.user_id = ?
        ORDER BY t.slug
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(topics)
}

/// Get all users subscribed to a topic.
pub async fn get_topic_subscribers(pool: &SqlitePool, topic_slug: &str) -> Result<Vec<User>> {
    let users = sqlx::query_as::<_, User>(
        r#"
        SELECT u.id, u.name, u.language
        FROM users u
        INNER JOIN notifications n ON n.user_id = u.id
        WHERE n.topic_slug = ?
        ORDER BY u.name
        "#,
    )
    .bind(topic_slug)
    .fetch_all(pool)
    .await?;

    Ok(users)
}

/// Get all notification subscriptions.
pub async fn list_subscriptions(pool: &SqlitePool) -> Result<Vec<Notification>> {
    let notifications = sqlx::query_as::<_, Notification>(
        r#"
        SELECT topic_slug, user_id, created_at
        FROM notifications
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(notifications)
}

/// Get all notification subscriptions for a user.
pub async fn get_user_notifications(pool: &SqlitePool, user_id: &str) -> Result<Vec<Notification>> {
    let notifications = sqlx::query_as::<_, Notification>(
        r#"
        SELECT topic_slug, user_id, created_at
        FROM notifications
        WHERE user_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(notifications)
}
