use std::env;
use std::path::PathBuf;
use std::time::Duration;

use agent_brain::AgentBrain;
use message_listener::{Brain, MessageProcessor, ProcessorConfig};
use signal_daemon::{DaemonConfig, ProcessConfig, SignalClient, spawn_and_connect};
use tracing::info;

/// Default timeout for waiting for daemon to be ready.
const DAEMON_READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Check if we should connect to an external daemon or spawn our own.
fn should_spawn_daemon() -> bool {
    // If SIGNAL_DAEMON_URL is set, use external daemon
    if env::var("SIGNAL_DAEMON_URL").is_ok() {
        return false;
    }
    // If SPAWN_DAEMON is explicitly set to false, don't spawn
    if let Ok(val) = env::var("SPAWN_DAEMON") {
        return !matches!(val.to_lowercase().as_str(), "false" | "0" | "no");
    }
    // Default: spawn daemon
    true
}

fn get_process_config() -> Result<ProcessConfig, Box<dyn std::error::Error>> {
    let account = env::var("AMAN_NUMBER")
        .map_err(|_| "AMAN_NUMBER environment variable is required")?;

    let jar_path = env::var("SIGNAL_CLI_JAR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("build/signal-cli.jar"));

    let http_addr = env::var("HTTP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    let mut config = ProcessConfig::new(jar_path, &account)
        .with_http_addr(&http_addr);

    // Use custom config dir if specified
    if let Ok(config_dir) = env::var("SIGNAL_CONFIG_DIR") {
        config = config.with_config_dir(config_dir);
    }

    Ok(config)
}

fn get_daemon_config() -> DaemonConfig {
    let base_url = if let Ok(url) = env::var("SIGNAL_DAEMON_URL") {
        url
    } else if let Ok(addr) = env::var("HTTP_ADDR") {
        if addr.starts_with("http://") || addr.starts_with("https://") {
            addr
        } else {
            format!("http://{}", addr)
        }
    } else {
        "http://127.0.0.1:8080".to_string()
    };

    match env::var("SIGNAL_DAEMON_ACCOUNT") {
        Ok(account) => DaemonConfig::with_account(base_url, account),
        Err(_) => DaemonConfig::new(base_url),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt::init();

    let brain = AgentBrain::from_env().await?;
    let bot_number = env::var("AMAN_NUMBER").ok();

    // Either spawn daemon or connect to existing one
    let (client, _daemon_process) = if should_spawn_daemon() {
        let process_config = get_process_config()?;
        info!(
            "Spawning signal-cli daemon (jar: {:?}, account: {}, http: {})",
            process_config.jar_path, process_config.account, process_config.http_addr
        );

        let (daemon, client) = spawn_and_connect(process_config, DAEMON_READY_TIMEOUT).await?;
        info!("Daemon started with PID {}", daemon.pid());

        // Keep daemon process alive by storing it
        // It will be killed when dropped (on shutdown)
        (client, Some(daemon))
    } else {
        let config = get_daemon_config();
        info!("Connecting to external daemon at {}", config.base_url);

        let client = SignalClient::connect(config).await?;
        (client, None)
    };

    let mut processor_config = match bot_number {
        Some(number) => ProcessorConfig::with_bot_number(number),
        None => ProcessorConfig::default(),
    };
    processor_config.send_typing_indicators = true;

    info!("Starting message processor with brain: {}", brain.name());
    let processor = MessageProcessor::new(client, brain, processor_config);
    processor.run().await?;

    Ok(())
}
