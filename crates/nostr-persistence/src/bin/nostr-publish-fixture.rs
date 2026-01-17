use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tracing::info;

use nostr_persistence::{
    DocManifest, NostrKinds, NostrPublisher, NostrPublisherImpl, NoopCodec, PublisherConfig,
    SecretBoxCodec,
};

#[derive(Debug, Parser)]
#[command(name = "nostr-publish-fixture")]
#[command(about = "Publish a DocManifest fixture to Nostr relays")]
struct Args {
    #[arg(long, required = true)]
    relay: Vec<String>,
    #[arg(long, required = true)]
    key: String,
    #[arg(long, required = true)]
    doc: PathBuf,
    #[arg(long, default_value_t = 1)]
    min_acks: usize,
    #[arg(long, default_value_t = 10)]
    timeout_secs: u64,
    #[arg(long)]
    secretbox_key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let bytes = std::fs::read(&args.doc)?;
    let doc: DocManifest = serde_json::from_slice(&bytes)?;

    let codec: Arc<dyn nostr_persistence::PayloadCodec> = if let Some(key) = args
        .secretbox_key
        .or_else(|| std::env::var("NOSTR_SECRETBOX_KEY").ok())
    {
        Arc::new(SecretBoxCodec::from_str(&key)?)
    } else {
        Arc::new(NoopCodec)
    };

    let config = PublisherConfig {
        relays: args.relay,
        secret_key: args.key,
        min_acks: args.min_acks,
        timeout: Duration::from_secs(args.timeout_secs),
        kinds: NostrKinds::default(),
        codec,
    };

    let publisher = NostrPublisherImpl::new(config).await?;
    let result = publisher.publish_doc_manifest(doc, vec![]).await?;

    info!(event_id = %result.event_id, success = result.success, failed = result.failed, "Published fixture");
    Ok(())
}
