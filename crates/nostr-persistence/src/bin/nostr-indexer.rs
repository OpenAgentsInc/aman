use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tracing::info;

use nostr_persistence::{
    IndexerConfig, NostrIndexer, NostrIndexerImpl, NostrKinds, NoopCodec, SecretBoxCodec,
};

#[derive(Debug, Parser)]
#[command(name = "nostr-indexer")]
#[command(about = "Backfill and subscribe to Nostr events into SQLite")]
struct Args {
    #[arg(long, required = true)]
    relay: Vec<String>,
    #[arg(long, default_value = "./data/nostr.db")]
    db: PathBuf,
    #[arg(long)]
    author: Vec<String>,
    #[arg(long, default_value_t = 10)]
    timeout_secs: u64,
    #[arg(long)]
    since: Option<u64>,
    #[arg(long)]
    limit: Option<u64>,
    #[arg(long)]
    secretbox_key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let codec: Arc<dyn nostr_persistence::PayloadCodec> = if let Some(key) = args
        .secretbox_key
        .or_else(|| std::env::var("NOSTR_SECRETBOX_KEY").ok())
    {
        Arc::new(SecretBoxCodec::from_str(&key)?)
    } else {
        Arc::new(NoopCodec)
    };

    let config = IndexerConfig {
        relays: args.relay,
        authors: args.author,
        timeout: Duration::from_secs(args.timeout_secs),
        kinds: NostrKinds::default(),
        db_path: args.db,
        backfill_since: args.since,
        backfill_limit: args.limit,
        codec,
    };

    let indexer = NostrIndexerImpl::new(config).await?;
    info!(db = %indexer.db_path().display(), "Starting Nostr indexer");

    indexer.backfill().await?;
    indexer.start().await?;

    Ok(())
}
