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
        Self {
            smtp_host: smtp_host.into(),
            smtp_port,
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
    pub fn from_env() -> Result<Self, ProtonError> {
        let smtp_host = env::var("PROTON_SMTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

        let smtp_port = env::var("PROTON_SMTP_PORT")
            .unwrap_or_else(|_| "1025".to_string())
            .parse::<u16>()
            .map_err(|e| ProtonError::Config(format!("Invalid PROTON_SMTP_PORT: {}", e)))?;

        let username =
            env::var("PROTON_USERNAME").map_err(|_| ProtonError::MissingEnvVar("PROTON_USERNAME".to_string()))?;

        let password =
            env::var("PROTON_PASSWORD").map_err(|_| ProtonError::MissingEnvVar("PROTON_PASSWORD".to_string()))?;

        Ok(Self::new(smtp_host, smtp_port, username, password))
    }

    /// Get the password (exposes the secret).
    pub(crate) fn password(&self) -> &str {
        self.password.expose_secret()
    }

    /// Builder method to set SMTP host.
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.smtp_host = host.into();
        self
    }

    /// Builder method to set SMTP port.
    pub fn with_port(mut self, port: u16) -> Self {
        self.smtp_port = port;
        self
    }
}
