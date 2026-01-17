//! SQLite persistence layer for Aman.
//!
//! This crate provides async database operations for users, topics, and notification
//! subscriptions using SQLx with SQLite.
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
//!     // Subscribe to a topic
//!     database::notification::subscribe(db.pool(), &user.id, "iran").await?;
//!
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod models;
pub mod notification;
pub mod topic;
pub mod user;

pub use error::{DatabaseError, Result};
pub use models::{Notification, Topic, User};

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
    pub async fn connect(url: &str) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        tracing::info!("Connected to database: {}", url);

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

    #[tokio::test]
    async fn test_topic_crud() {
        let db = test_db().await;

        // Initial topics from migration
        let topics = topic::list_topics(db.pool()).await.unwrap();
        assert!(topics.iter().any(|t| t.slug == "iran"));

        // Create
        let new_topic = topic::create_topic(db.pool(), "test-topic").await.unwrap();
        assert_eq!(new_topic.slug, "test-topic");

        // Read
        let fetched = topic::get_topic(db.pool(), "test-topic").await.unwrap();
        assert_eq!(fetched.slug, "test-topic");

        // Delete
        topic::delete_topic(db.pool(), "test-topic").await.unwrap();
        let result = topic::get_topic(db.pool(), "test-topic").await;
        assert!(matches!(result, Err(DatabaseError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_subscriptions() {
        let db = test_db().await;

        // Create a user
        let user = User {
            id: "sub-test-user".to_string(),
            name: "Subscriber".to_string(),
            language: "English".to_string(),
        };
        user::create_user(db.pool(), &user).await.unwrap();

        // Subscribe to topics
        notification::subscribe(db.pool(), &user.id, "iran").await.unwrap();
        notification::subscribe(db.pool(), &user.id, "bitcoin").await.unwrap();

        // Check subscription
        assert!(notification::is_subscribed(db.pool(), &user.id, "iran").await.unwrap());
        assert!(!notification::is_subscribed(db.pool(), &user.id, "uganda").await.unwrap());

        // Get user subscriptions
        let subs = notification::get_user_subscriptions(db.pool(), &user.id).await.unwrap();
        assert_eq!(subs.len(), 2);

        // Get topic subscribers
        let subscribers = notification::get_topic_subscribers(db.pool(), "iran").await.unwrap();
        assert_eq!(subscribers.len(), 1);
        assert_eq!(subscribers[0].id, user.id);

        // Unsubscribe
        notification::unsubscribe(db.pool(), &user.id, "iran").await.unwrap();
        assert!(!notification::is_subscribed(db.pool(), &user.id, "iran").await.unwrap());
    }
}
