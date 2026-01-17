//! Topic CRUD operations.

use sqlx::SqlitePool;

use crate::error::{DatabaseError, Result};
use crate::models::Topic;

/// Create a new topic.
pub async fn create_topic(pool: &SqlitePool, slug: &str) -> Result<Topic> {
    sqlx::query(
        r#"
        INSERT INTO topics (slug)
        VALUES (?)
        "#,
    )
    .bind(slug)
    .execute(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.is_unique_violation() {
                return DatabaseError::AlreadyExists {
                    entity: "Topic",
                    id: slug.to_string(),
                };
            }
        }
        DatabaseError::Sqlx(e)
    })?;

    Ok(Topic {
        slug: slug.to_string(),
    })
}

/// Get a topic by slug.
pub async fn get_topic(pool: &SqlitePool, slug: &str) -> Result<Topic> {
    sqlx::query_as::<_, Topic>(
        r#"
        SELECT slug
        FROM topics
        WHERE slug = ?
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DatabaseError::NotFound {
        entity: "Topic",
        id: slug.to_string(),
    })
}

/// Delete a topic by slug.
pub async fn delete_topic(pool: &SqlitePool, slug: &str) -> Result<()> {
    let result = sqlx::query(
        r#"
        DELETE FROM topics
        WHERE slug = ?
        "#,
    )
    .bind(slug)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(DatabaseError::NotFound {
            entity: "Topic",
            id: slug.to_string(),
        });
    }

    Ok(())
}

/// List all topics.
pub async fn list_topics(pool: &SqlitePool) -> Result<Vec<Topic>> {
    let topics = sqlx::query_as::<_, Topic>(
        r#"
        SELECT slug
        FROM topics
        ORDER BY slug
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(topics)
}
