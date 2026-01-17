//! Database error types.

use thiserror::Error;

/// Errors that can occur during database operations.
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// SQLx error (connection, query, etc.)
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    /// Migration error
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    /// Record not found
    #[error("{entity} not found: {id}")]
    NotFound { entity: &'static str, id: String },

    /// Record already exists
    #[error("{entity} already exists: {id}")]
    AlreadyExists { entity: &'static str, id: String },
}

/// Result type for database operations.
pub type Result<T> = std::result::Result<T, DatabaseError>;
