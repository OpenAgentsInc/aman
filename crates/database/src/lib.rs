//! SQLite persistence layer for Aman.
//!
//! This crate provides async database operations for users, preferences, and
//! conversation state using SQLx with SQLite.
//!
//! # Example
//!
//! ```no_run
//! use database::{Database, models::User, user};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect and run migrations
//!     let db = Database::connect("sqlite:aman.db?mode=rwc").await?;
//!     db.migrate().await?;
//!
//!     // Create a user
//!     let user = User {
//!         id: "c27fb365-0c84-4cf2-8555-814bb065e448".to_string(),
//!         name: "Bob".to_string(),
//!         language: "Arabic".to_string(),
//!     };
//!     user::create_user(db.pool(), &user).await?;
//!
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod models;
pub mod preference;
pub mod conversation_summary;
pub mod tool_history;
pub mod clear_context_event;
pub mod user;
pub mod user_profile;
pub mod validation;

pub use error::{DatabaseError, Result};
pub use models::{
    ClearContextEvent, ConversationSummary, Preference, ToolHistoryEntry,
    User, UserProfile,
};
pub use user_profile::ProfileField;
pub use validation::ValidationError;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

/// Database connection wrapper.
#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Connect to a SQLite database.
    ///
    /// The URL should be in the format `sqlite:path/to/db.sqlite?mode=rwc`.
    /// Use `?mode=rwc` to create the database file if it doesn't exist.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn example() -> database::Result<()> {
    /// // File database
    /// let db = database::Database::connect("sqlite:data/aman.db?mode=rwc").await?;
    ///
    /// // In-memory database (for testing)
    /// let db = database::Database::connect("sqlite::memory:").await?;
    /// # Ok(())
    /// # }
    /// ```
    /// Default pool size for database connections.
    /// Set high enough to handle concurrent message processing with memory operations.
    const DEFAULT_POOL_SIZE: u32 = 20;

    pub async fn connect(url: &str) -> Result<Self> {
        Self::connect_with_pool_size(url, Self::DEFAULT_POOL_SIZE).await
    }

    /// Connect to a SQLite database with a custom pool size.
    pub async fn connect_with_pool_size(url: &str, pool_size: u32) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(pool_size)
            .acquire_timeout(std::time::Duration::from_secs(30))
            .connect_with(options)
            .await?;

        tracing::info!(
            "Connected to database: {} (pool size: {})",
            url,
            pool_size
        );

        Ok(Self { pool })
    }

    /// Run database migrations.
    ///
    /// This should be called once after connecting to ensure the schema is up to date.
    pub async fn migrate(&self) -> Result<()> {
        tracing::info!("Running database migrations...");

        sqlx::migrate!("./migrations").run(&self.pool).await?;

        tracing::info!("Migrations complete");
        Ok(())
    }

    /// Get a reference to the connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Close the database connection pool.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> Database {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.migrate().await.unwrap();
        db
    }

    #[tokio::test]
    async fn test_user_crud() {
        let db = test_db().await;

        // Create
        let user = User {
            id: "test-uuid-123".to_string(),
            name: "Alice".to_string(),
            language: "English".to_string(),
        };
        user::create_user(db.pool(), &user).await.unwrap();

        // Read
        let fetched = user::get_user(db.pool(), &user.id).await.unwrap();
        assert_eq!(fetched.name, "Alice");

        // Update
        let updated = User {
            language: "Spanish".to_string(),
            ..user.clone()
        };
        user::update_user(db.pool(), &updated).await.unwrap();
        let fetched = user::get_user(db.pool(), &user.id).await.unwrap();
        assert_eq!(fetched.language, "Spanish");

        // List
        let users = user::list_users(db.pool()).await.unwrap();
        assert_eq!(users.len(), 1);

        // Delete
        user::delete_user(db.pool(), &user.id).await.unwrap();
        let result = user::get_user(db.pool(), &user.id).await;
        assert!(matches!(result, Err(DatabaseError::NotFound { .. })));
    }
}
