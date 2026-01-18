use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tracing::info;

use nostr_persistence::{
    project_memory, IndexerConfig, NostrIndexer, NostrIndexerImpl, NostrKinds, NoopCodec,
    SecretBoxCodec,
};

#[derive(Debug, Parser)]
#[command(name = "nostr-rehydrate-memory")]
#[command(about = "Backfill Nostr memory events and project into the Aman runtime DB")]
struct Args {
    /// Nostr relay URL(s) to backfill from
    #[arg(long, required = true)]
    relay: Vec<String>,
    /// Path to the Nostr SQLite DB
    #[arg(long, default_value = "./data/nostr.db")]
    nostr_db: PathBuf,
    /// Path to the Aman runtime SQLite DB
    #[arg(long, default_value = "./data/aman.db")]
    aman_db: PathBuf,
    /// Backfill timeout in seconds
    #[arg(long, default_value_t = 10)]
    timeout_secs: u64,
    /// Optional backfill start timestamp (unix seconds)
    #[arg(long)]
    since: Option<u64>,
    /// Optional secretbox key (falls back to NOSTR_SECRETBOX_KEY)
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
        authors: vec![],
        timeout: Duration::from_secs(args.timeout_secs),
        kinds: NostrKinds::default(),
        db_path: args.nostr_db.clone(),
        backfill_since: args.since,
        backfill_limit: None,
        codec,
    };

    let indexer = NostrIndexerImpl::new(config).await?;
    info!(db = %indexer.db_path().display(), "Backfilling Nostr memory events");
    indexer.backfill().await?;

    let stats = project_memory(&args.nostr_db, &args.aman_db)?;
    info!(
        preferences = stats.preferences,
        summaries = stats.summaries,
        tool_history = stats.tool_history,
        clear_events = stats.clear_events,
        "Projected memory into runtime DB"
    );

    Ok(())
}
