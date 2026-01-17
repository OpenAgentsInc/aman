use std::env;
use std::fs;

use agent_brain::{AgentBrain, RegionEvent};
use broadcaster::Broadcaster;
use signal_daemon::DaemonConfig;

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

    let mut args = env::args().skip(1);
    let event_path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!("Usage: region_event_send <event.json>");
            std::process::exit(1);
        }
    };

    let event_data = fs::read_to_string(&event_path)?;
    let event: RegionEvent = serde_json::from_str(&event_data)?;

    let brain = AgentBrain::from_env().await?;
    let messages = brain.fanout_event(&event).await?;

    if messages.is_empty() {
        println!("No subscribers for region: {}", event.region);
        return Ok(());
    }

    let daemon_url = daemon_base_url();
    let daemon_account = env::var("SIGNAL_DAEMON_ACCOUNT").ok();

    let config = match daemon_account {
        Some(account) => DaemonConfig::with_account(daemon_url, account),
        None => DaemonConfig::new(daemon_url),
    };

    let broadcaster = Broadcaster::connect(config).await?;

    for message in messages {
        if message.is_group {
            broadcaster.send_to_group(&message.recipient, &message.text).await?;
        } else {
            broadcaster.send_text(&message.recipient, &message.text).await?;
        }
    }

    Ok(())
}
