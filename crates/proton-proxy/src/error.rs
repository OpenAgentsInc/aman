use thiserror::Error;

/// Errors that can occur when using the Proton proxy client.
#[derive(Debug, Error)]
pub enum ProtonError {
    /// Failed to build SMTP transport
    #[error("SMTP transport error: {0}")]
    Transport(String),

    /// Failed to send email
    #[error("Failed to send email: {0}")]
    Send(String),

    /// Failed to build email message
    #[error("Failed to build email: {0}")]
    BuildEmail(String),

    /// Invalid email address
    #[error("Invalid email address: {0}")]
    InvalidAddress(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Missing required environment variable
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),

    /// IO error (e.g., reading attachment file)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid attachment
    #[error("Invalid attachment: {0}")]
    Attachment(String),
}
