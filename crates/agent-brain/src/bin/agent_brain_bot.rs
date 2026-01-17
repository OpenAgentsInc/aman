use std::env;

use agent_brain::AgentBrain;
use message_listener::{MessageProcessor, ProcessorConfig};
use signal_daemon::{DaemonConfig, SignalClient};

fn daemon_base_url() -> String {
    if let Ok(url) = env::var("SIGNAL_DAEMON_URL") {
        return url;
    }
    if let Ok(addr) = env::var("HTTP_ADDR") {
        if addr.starts_with("http://") || addr.starts_with("https://") {
            return addr;
        }
        return format!("http://{}", addr);
    }
    "http://localhost:8080".to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt::init();

    let brain = AgentBrain::from_env().await?;
    let bot_number = env::var("AMAN_NUMBER").ok();
    let daemon_url = daemon_base_url();
    let daemon_account = env::var("SIGNAL_DAEMON_ACCOUNT").ok();

    let config = match daemon_account {
        Some(account) => DaemonConfig::with_account(daemon_url, account),
        None => DaemonConfig::new(daemon_url),
    };

    let client = SignalClient::connect(config).await?;

    let mut processor_config = match bot_number {
        Some(number) => ProcessorConfig::with_bot_number(number),
        None => ProcessorConfig::default(),
    };
    processor_config.send_typing_indicators = true;

    let processor = MessageProcessor::new(client, brain, processor_config);
    processor.run().await?;

    Ok(())
}
