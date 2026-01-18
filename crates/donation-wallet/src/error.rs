//! Error types for the donation wallet.

use thiserror::Error;

/// Errors that can occur when using the donation wallet.
#[derive(Error, Debug)]
pub enum DonationWalletError {
    /// Error from the underlying LNI library
    #[error("Lightning error: {0}")]
    Lightning(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Invoice not found
    #[error("Invoice not found: {0}")]
    InvoiceNotFound(String),
}

impl From<lni::ApiError> for DonationWalletError {
    fn from(err: lni::ApiError) -> Self {
        DonationWalletError::Lightning(err.to_string())
    }
}
