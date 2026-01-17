//! User CRUD operations.

use sqlx::SqlitePool;

use crate::error::{DatabaseError, Result};
use crate::models::User;

/// Create a new user.
pub async fn create_user(pool: &SqlitePool, user: &User) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO users (id, name, language)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(&user.id)
    .bind(&user.name)
    .bind(&user.language)
    .execute(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.is_unique_violation() {
                return DatabaseError::AlreadyExists {
                    entity: "User",
                    id: user.id.clone(),
                };
            }
        }
        DatabaseError::Sqlx(e)
    })?;

    Ok(())
}

/// Get a user by ID.
pub async fn get_user(pool: &SqlitePool, id: &str) -> Result<User> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT id, name, language
        FROM users
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DatabaseError::NotFound {
        entity: "User",
        id: id.to_string(),
    })
}

/// Get a user by name.
pub async fn get_user_by_name(pool: &SqlitePool, name: &str) -> Result<User> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT id, name, language
        FROM users
        WHERE name = ?
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DatabaseError::NotFound {
        entity: "User",
        id: name.to_string(),
    })
}

/// Update an existing user.
pub async fn update_user(pool: &SqlitePool, user: &User) -> Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE users
        SET name = ?, language = ?
        WHERE id = ?
        "#,
    )
    .bind(&user.name)
    .bind(&user.language)
    .bind(&user.id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(DatabaseError::NotFound {
            entity: "User",
            id: user.id.clone(),
        });
    }

    Ok(())
}

/// Delete a user by ID.
pub async fn delete_user(pool: &SqlitePool, id: &str) -> Result<()> {
    let result = sqlx::query(
        r#"
        DELETE FROM users
        WHERE id = ?
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(DatabaseError::NotFound {
            entity: "User",
            id: id.to_string(),
        });
    }

    Ok(())
}

/// List all users.
pub async fn list_users(pool: &SqlitePool) -> Result<Vec<User>> {
    let users = sqlx::query_as::<_, User>(
        r#"
        SELECT id, name, language
        FROM users
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(users)
}

/// Count total users.
pub async fn count_users(pool: &SqlitePool) -> Result<i64> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM users
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(count)
}

/// Count users grouped by language.
pub async fn count_users_by_language(pool: &SqlitePool) -> Result<Vec<(String, i64)>> {
    let rows = sqlx::query_as::<_, (String, i64)>(
        r#"
        SELECT language, COUNT(*) as count
        FROM users
        GROUP BY language
        ORDER BY count DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
