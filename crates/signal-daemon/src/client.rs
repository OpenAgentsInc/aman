//! Signal-cli daemon HTTP client.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::config::DaemonConfig;
use crate::error::DaemonError;
use crate::types::{SendParams, SendResult, TextStyleParam, TypingParams};

/// JSON-RPC 2.0 request structure.
#[derive(Debug, Serialize)]
struct RpcRequest<'a, T: Serialize> {
    jsonrpc: &'static str,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<T>,
    id: u64,
}

/// JSON-RPC 2.0 response structure.
#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: String,
    result: Option<T>,
    error: Option<RpcError>,
    #[allow(dead_code)]
    id: u64,
}

/// JSON-RPC 2.0 error.
#[derive(Debug, Deserialize)]
struct RpcError {
    code: i32,
    message: String,
}

/// Version response from signal-cli.
#[derive(Debug, Deserialize)]
struct VersionResponse {
    version: String,
}

/// Account number response from signal-cli.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AccountResponse {
    /// Plain string response.
    String(String),
    /// Object with number field.
    Object { number: String },
}

/// Client for communicating with the signal-cli daemon.
#[derive(Clone)]
pub struct SignalClient {
    http: Client,
    config: DaemonConfig,
    request_id: Arc<std::sync::atomic::AtomicU64>,
    connected: Arc<AtomicBool>,
}

impl SignalClient {
    /// Connect to the signal-cli daemon.
    pub async fn connect(config: DaemonConfig) -> Result<Self, DaemonError> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(DaemonError::Http)?;

        let client = Self {
            http,
            config,
            request_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            connected: Arc::new(AtomicBool::new(false)),
        };

        // Verify connection with health check
        if client.health_check().await? {
            client.connected.store(true, Ordering::SeqCst);
            info!("Connected to signal-cli daemon at {}", client.config.base_url);
        } else {
            return Err(DaemonError::HealthCheckFailed);
        }

        Ok(client)
    }

    /// Check if currently connected to the daemon.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Perform a health check against the daemon.
    pub async fn health_check(&self) -> Result<bool, DaemonError> {
        let url = self.config.check_url();
        debug!("Health check: {}", url);

        match self.http.get(&url).send().await {
            Ok(resp) => {
                let ok = resp.status().is_success();
                self.connected.store(ok, Ordering::SeqCst);
                Ok(ok)
            }
            Err(e) => {
                self.connected.store(false, Ordering::SeqCst);
                Err(DaemonError::Http(e))
            }
        }
    }

    /// Get the signal-cli version.
    pub async fn version(&self) -> Result<String, DaemonError> {
        let resp: VersionResponse = self.rpc_call::<(), _>("version", None).await?;
        Ok(resp.version)
    }

    /// Get the account's phone number.
    pub async fn get_self_number(&self) -> Result<String, DaemonError> {
        let resp: AccountResponse = self.rpc_call::<(), _>("getSelfNumber", None).await?;
        match resp {
            AccountResponse::String(s) => Ok(s),
            AccountResponse::Object { number } => Ok(number),
        }
    }

    /// Send a message using the full SendParams structure.
    pub async fn send(&self, mut params: SendParams) -> Result<SendResult, DaemonError> {
        // Add account if configured and not already set
        if params.account.is_none() {
            params.account = self.config.account.clone();
        }

        self.rpc_call("send", Some(params)).await
    }

    /// Send a text message to a recipient.
    pub async fn send_text(
        &self,
        recipient: &str,
        message: &str,
    ) -> Result<SendResult, DaemonError> {
        let params = SendParams::text(recipient, message);
        self.send(params).await
    }

    /// Send a text message to a group.
    pub async fn send_to_group(
        &self,
        group_id: &str,
        message: &str,
    ) -> Result<SendResult, DaemonError> {
        let params = SendParams::group(group_id, message);
        self.send(params).await
    }

    /// Send a styled text message to a recipient.
    ///
    /// # Arguments
    /// * `recipient` - Phone number to send to
    /// * `message` - Text content (with markdown markers removed)
    /// * `styles` - Text style ranges for formatting
    pub async fn send_styled_text(
        &self,
        recipient: &str,
        message: &str,
        styles: Vec<TextStyleParam>,
    ) -> Result<SendResult, DaemonError> {
        let params = SendParams::text(recipient, message).with_styles(styles);
        self.send(params).await
    }

    /// Send a styled text message to a group.
    ///
    /// # Arguments
    /// * `group_id` - Group ID to send to
    /// * `message` - Text content (with markdown markers removed)
    /// * `styles` - Text style ranges for formatting
    pub async fn send_styled_to_group(
        &self,
        group_id: &str,
        message: &str,
        styles: Vec<TextStyleParam>,
    ) -> Result<SendResult, DaemonError> {
        let params = SendParams::group(group_id, message).with_styles(styles);
        self.send(params).await
    }

    /// Send a typing indicator to a recipient.
    ///
    /// # Arguments
    /// * `recipient` - Phone number to send typing indicator to
    /// * `started` - true for "started typing", false for "stopped typing"
    pub async fn send_typing(
        &self,
        recipient: &str,
        started: bool,
    ) -> Result<(), DaemonError> {
        let params = TypingParams {
            account: self.config.account.clone(),
            recipient: recipient.to_string(),
            group_id: None,
            stop: !started,
        };
        // sendTyping returns an empty result on success
        let _: serde_json::Value = self.rpc_call("sendTyping", Some(params)).await?;
        Ok(())
    }

    /// Send a typing indicator to a group.
    pub async fn send_typing_to_group(
        &self,
        group_id: &str,
        started: bool,
    ) -> Result<(), DaemonError> {
        let params = TypingParams {
            account: self.config.account.clone(),
            recipient: String::new(),
            group_id: Some(group_id.to_string()),
            stop: !started,
        };
        let _: serde_json::Value = self.rpc_call("sendTyping", Some(params)).await?;
        Ok(())
    }

    /// Start a background health monitor that periodically checks the daemon.
    pub fn start_health_monitor(&self, interval: Duration) -> JoinHandle<()> {
        let client = self.clone();

        tokio::spawn(async move {
            let mut consecutive_failures = 0u32;

            loop {
                tokio::time::sleep(interval).await;

                match client.health_check().await {
                    Ok(true) => {
                        if consecutive_failures > 0 {
                            info!("Daemon connection restored");
                        }
                        consecutive_failures = 0;
                    }
                    Ok(false) => {
                        consecutive_failures += 1;
                        warn!(
                            "Health check returned not OK (failures: {})",
                            consecutive_failures
                        );
                    }
                    Err(e) => {
                        consecutive_failures += 1;
                        error!(
                            "Health check failed: {} (failures: {})",
                            e, consecutive_failures
                        );
                    }
                }
            }
        })
    }

    /// Get the configuration.
    pub fn config(&self) -> &DaemonConfig {
        &self.config
    }

    /// Get the underlying HTTP client.
    pub fn http_client(&self) -> &Client {
        &self.http
    }

    /// Make a JSON-RPC call to the daemon.
    async fn rpc_call<P: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: Option<P>,
    ) -> Result<R, DaemonError> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let url = self.config.rpc_url();

        let request = RpcRequest {
            jsonrpc: "2.0",
            method,
            params,
            id,
        };

        debug!("RPC call: {} (id={})", method, id);

        let response = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(DaemonError::Http)?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(DaemonError::Connection(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let rpc_response: RpcResponse<R> = response.json().await.map_err(DaemonError::Http)?;

        if let Some(error) = rpc_response.error {
            return Err(DaemonError::Rpc {
                code: error.code,
                message: error.message,
            });
        }

        rpc_response
            .result
            .ok_or_else(|| DaemonError::Rpc {
                code: -1,
                message: "No result in response".to_string(),
            })
    }
}

impl std::fmt::Debug for SignalClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignalClient")
            .field("config", &self.config)
            .field("connected", &self.is_connected())
            .finish()
    }
}
