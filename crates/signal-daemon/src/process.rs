//! Process management for spawning signal-cli daemon.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use tokio::time::sleep;
use tracing::{debug, error, info};

use crate::error::DaemonError;
use crate::DaemonConfig;

/// Default path to signal-cli.jar relative to project root.
pub const DEFAULT_JAR_PATH: &str = "build/signal-cli.jar";

/// Configuration for spawning a signal-cli daemon process.
#[derive(Debug, Clone)]
pub struct ProcessConfig {
    /// Path to signal-cli.jar.
    pub jar_path: PathBuf,
    /// Signal account phone number (e.g., "+1234567890").
    pub account: String,
    /// HTTP bind address (e.g., "127.0.0.1:8080").
    pub http_addr: String,
    /// Optional config directory for signal-cli data.
    pub config_dir: Option<PathBuf>,
    /// Send read receipts automatically.
    pub send_read_receipts: bool,
    /// Trust new identities on first use.
    pub trust_new_identities: bool,
}

impl ProcessConfig {
    /// Create a new process config with required fields.
    pub fn new(jar_path: impl Into<PathBuf>, account: impl Into<String>) -> Self {
        Self {
            jar_path: jar_path.into(),
            account: account.into(),
            http_addr: "127.0.0.1:8080".to_string(),
            config_dir: None,
            send_read_receipts: true,
            trust_new_identities: true,
        }
    }

    /// Set the HTTP bind address.
    pub fn with_http_addr(mut self, addr: impl Into<String>) -> Self {
        self.http_addr = addr.into();
        self
    }

    /// Set the signal-cli config directory.
    pub fn with_config_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config_dir = Some(dir.into());
        self
    }

    /// Get the base URL for connecting to this daemon.
    pub fn base_url(&self) -> String {
        format!("http://{}", self.http_addr)
    }

    /// Convert to a DaemonConfig for connecting.
    pub fn to_daemon_config(&self) -> DaemonConfig {
        DaemonConfig::with_account(self.base_url(), &self.account)
    }
}

/// A running signal-cli daemon process.
pub struct DaemonProcess {
    child: Child,
    config: ProcessConfig,
}

impl DaemonProcess {
    /// Spawn a new signal-cli daemon process.
    pub fn spawn(config: ProcessConfig) -> Result<Self, DaemonError> {
        // Verify JAR exists
        if !config.jar_path.exists() {
            return Err(DaemonError::Config(format!(
                "signal-cli.jar not found at {:?}. Run scripts/build-signal-cli.sh first.",
                config.jar_path
            )));
        }

        // Build command
        let mut cmd = Command::new("java");
        cmd.arg("-jar")
            .arg(&config.jar_path);

        // Add config dir if specified
        if let Some(ref config_dir) = config.config_dir {
            cmd.arg("--config").arg(config_dir);
        }

        // Trust new identities
        if config.trust_new_identities {
            cmd.arg("--trust-new-identities=on-first-use");
        }

        // Account
        cmd.arg("-a").arg(&config.account);

        // Daemon mode with HTTP
        cmd.arg("daemon")
            .arg(format!("--http={}", config.http_addr));

        // Read receipts
        if config.send_read_receipts {
            cmd.arg("--send-read-receipts");
        }

        // Suppress stdout/stderr or pipe them
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped());

        info!("Spawning signal-cli daemon: java -jar {:?} -a {} daemon --http={}",
            config.jar_path, config.account, config.http_addr);

        let child = cmd.spawn().map_err(|e| {
            DaemonError::Connection(format!("Failed to spawn signal-cli: {}", e))
        })?;

        debug!("Daemon process started with PID {}", child.id());

        Ok(Self { child, config })
    }

    /// Wait for the daemon to become ready (health check passes).
    pub async fn wait_ready(&self, timeout: Duration) -> Result<(), DaemonError> {
        let client = reqwest::Client::new();
        let check_url = format!("{}/api/v1/check", self.config.base_url());
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(100);

        info!("Waiting for daemon to be ready at {}...", check_url);

        loop {
            if start.elapsed() > timeout {
                return Err(DaemonError::Connection(format!(
                    "Daemon not ready after {:?}",
                    timeout
                )));
            }

            match client.get(&check_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    info!("Daemon is ready");
                    return Ok(());
                }
                Ok(_) => {
                    debug!("Health check returned non-success, retrying...");
                }
                Err(e) => {
                    debug!("Health check failed: {}, retrying...", e);
                }
            }

            sleep(poll_interval).await;
        }
    }

    /// Get the process ID.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Get the process config.
    pub fn config(&self) -> &ProcessConfig {
        &self.config
    }

    /// Check if the process is still running.
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Kill the daemon process.
    pub fn kill(&mut self) -> Result<(), DaemonError> {
        info!("Killing daemon process (PID {})", self.child.id());
        self.child.kill().map_err(|e| {
            DaemonError::Connection(format!("Failed to kill daemon: {}", e))
        })
    }

    /// Wait for the process to exit.
    pub fn wait(&mut self) -> Result<std::process::ExitStatus, DaemonError> {
        self.child.wait().map_err(|e| {
            DaemonError::Connection(format!("Failed to wait for daemon: {}", e))
        })
    }
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        if self.is_running() {
            if let Err(e) = self.kill() {
                error!("Failed to kill daemon on drop: {}", e);
            }
        }
    }
}

/// Spawn a daemon and return a connected client.
pub async fn spawn_and_connect(
    config: ProcessConfig,
    ready_timeout: Duration,
) -> Result<(DaemonProcess, crate::SignalClient), DaemonError> {
    let daemon_config = config.to_daemon_config();
    let process = DaemonProcess::spawn(config)?;
    process.wait_ready(ready_timeout).await?;
    let client = crate::SignalClient::connect(daemon_config).await?;
    Ok((process, client))
}
