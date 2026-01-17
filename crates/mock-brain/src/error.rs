//! Error types for brain operations.

use thiserror::Error;

/// Errors that can occur during brain processing.
#[derive(Debug, Error)]
pub enum BrainError {
    /// The brain is temporarily unavailable.
    #[error("brain unavailable: {0}")]
    Unavailable(String),

    /// The message could not be processed.
    #[error("processing failed: {0}")]
    ProcessingFailed(String),

    /// The brain has been shut down.
    #[error("brain shut down")]
    ShutDown,

    /// A timeout occurred during processing.
    #[error("processing timed out")]
    Timeout,
}
