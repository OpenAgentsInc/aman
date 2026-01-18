use secrecy::{ExposeSecret, SecretString};
use std::env;

use crate::ProtonError;

/// Configuration for connecting to Proton Mail Bridge.
#[derive(Debug, Clone)]
pub struct ProtonConfig {
    /// SMTP host (default: 127.0.0.1)
    pub smtp_host: String,
    /// SMTP port (default: 1025)
    pub smtp_port: u16,
    /// IMAP host (default: 127.0.0.1)
    pub imap_host: String,
    /// IMAP port (default: 1143)
    pub imap_port: u16,
    /// Proton email address (sender)
    pub username: String,
    /// Bridge-generated password
    password: SecretString,
}

impl ProtonConfig {
    /// Create a new configuration with explicit values.
    pub fn new(
        smtp_host: impl Into<String>,
        smtp_port: u16,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        let host = smtp_host.into();
        Self {
            smtp_host: host.clone(),
            smtp_port,
            imap_host: host,
            imap_port: 1143,
            username: username.into(),
            password: SecretString::from(password.into()),
        }
    }

    /// Create configuration from environment variables.
    ///
    /// Required:
    /// - `PROTON_USERNAME` - Proton email address
    /// - `PROTON_PASSWORD` - Bridge-generated password
    ///
    /// Optional (with defaults):
    /// - `PROTON_SMTP_HOST` - Default: 127.0.0.1
    /// - `PROTON_SMTP_PORT` - Default: 1025
    /// - `PROTON_IMAP_HOST` - Default: 127.0.0.1
    /// - `PROTON_IMAP_PORT` - Default: 1143
    pub fn from_env() -> Result<Self, ProtonError> {
        let smtp_host = env::var("PROTON_SMTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

        let smtp_port = env::var("PROTON_SMTP_PORT")
            .unwrap_or_else(|_| "1025".to_string())
            .parse::<u16>()
            .map_err(|e| ProtonError::Config(format!("Invalid PROTON_SMTP_PORT: {}", e)))?;

        let imap_host = env::var("PROTON_IMAP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

        let imap_port = env::var("PROTON_IMAP_PORT")
            .unwrap_or_else(|_| "1143".to_string())
            .parse::<u16>()
            .map_err(|e| ProtonError::Config(format!("Invalid PROTON_IMAP_PORT: {}", e)))?;

        let username =
            env::var("PROTON_USERNAME").map_err(|_| ProtonError::MissingEnvVar("PROTON_USERNAME".to_string()))?;

        let password =
            env::var("PROTON_PASSWORD").map_err(|_| ProtonError::MissingEnvVar("PROTON_PASSWORD".to_string()))?;

        Ok(Self {
            smtp_host,
            smtp_port,
            imap_host,
            imap_port,
            username,
            password: SecretString::from(password),
        })
    }

    /// Get the password (exposes the secret).
    pub(crate) fn password(&self) -> &str {
        self.password.expose_secret()
    }

    /// Builder method to set SMTP host.
    pub fn with_smtp_host(mut self, host: impl Into<String>) -> Self {
        self.smtp_host = host.into();
        self
    }

    /// Builder method to set SMTP port.
    pub fn with_smtp_port(mut self, port: u16) -> Self {
        self.smtp_port = port;
        self
    }

    /// Builder method to set IMAP host.
    pub fn with_imap_host(mut self, host: impl Into<String>) -> Self {
        self.imap_host = host.into();
        self
    }

    /// Builder method to set IMAP port.
    pub fn with_imap_port(mut self, port: u16) -> Self {
        self.imap_port = port;
        self
    }
}
