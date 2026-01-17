use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Parser;
use sha2::{Digest, Sha256};
use tracing::info;
use uuid::Uuid;

use nostr_persistence::{
    k_tag, unix_timestamp, ChunkOffsets, ChunkRef, DocChunk, DocManifest, IndexerConfig, NoopCodec,
    NostrEvent, NostrIndexer, NostrIndexerImpl, NostrKinds, NostrPublisher, NostrPublisherImpl,
    NostrTag, PublisherConfig, TAG_KIND_CHUNK_REF, TAG_KIND_DOC_MANIFEST,
};

#[derive(Debug, Parser)]
#[command(name = "ingester")]
#[command(about = "Ingest a document, chunk it, and publish/index via Nostr")]
struct Args {
    /// Input document path (text/markdown)
    #[arg(long)]
    file: PathBuf,

    /// Nostr relay URL(s) to publish to
    #[arg(long)]
    relay: Vec<String>,

    /// Nostr secret key (hex or bech32). Falls back to NOSTR_SECRET_KEY env.
    #[arg(long)]
    key: Option<String>,

    /// Index directly into a local Nostr SQLite DB (no relay required)
    #[arg(long)]
    index_db: Option<PathBuf>,

    /// Output directory for chunk files
    #[arg(long, default_value = "./data/ingest")]
    out_dir: PathBuf,

    /// Chunk size in characters
    #[arg(long, default_value_t = 800)]
    chunk_size: usize,

    /// Chunk overlap in characters
    #[arg(long, default_value_t = 200)]
    chunk_overlap: usize,

    /// Document title (defaults to filename)
    #[arg(long)]
    title: Option<String>,

    /// Language code (default: en)
    #[arg(long, default_value = "en")]
    lang: String,

    /// Source type (default: file_ingest)
    #[arg(long, default_value = "file_ingest")]
    source_type: String,

    /// Minimum relay acks required
    #[arg(long, default_value_t = 1)]
    min_acks: usize,

    /// Publish timeout in seconds
    #[arg(long, default_value_t = 10)]
    timeout_secs: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let text = fs::read_to_string(&args.file)?;
    let title = args
        .title
        .clone()
        .unwrap_or_else(|| args.file.file_name().unwrap().to_string_lossy().to_string());
    let doc_hash = sha256_hex(text.as_bytes());
    let doc_id = format!("doc_{}", short_hash(&doc_hash));

    let chunks = chunk_text(&text, args.chunk_size, args.chunk_overlap);
    let (doc_chunks, chunk_refs) =
        write_chunks(&doc_id, &args.out_dir, &chunks)?;

    let mut manifest = DocManifest::new(
        doc_id.clone(),
        title,
        args.lang.clone(),
        "text/plain".to_string(),
        args.source_type.clone(),
        format!("sha256:{}", doc_hash),
        doc_chunks,
    );
    manifest.blob_ref = Some(args.file.canonicalize()?.display().to_string());

    if !args.relay.is_empty() {
        let key = args
            .key
            .or_else(|| env::var("NOSTR_SECRET_KEY").ok())
            .ok_or("Missing NOSTR secret key (--key or NOSTR_SECRET_KEY)")?;

        let config = PublisherConfig {
            relays: args.relay.clone(),
            secret_key: key,
            min_acks: args.min_acks,
            timeout: Duration::from_secs(args.timeout_secs),
            kinds: NostrKinds::default(),
            codec: std::sync::Arc::new(NoopCodec),
        };

        let publisher = NostrPublisherImpl::new(config).await?;
        let result = publisher
            .publish_doc_manifest(manifest.clone(), vec![])
            .await?;
        info!(event_id = %result.event_id, "Published doc manifest");

        for chunk_ref in &chunk_refs {
            let result = publisher
                .publish_chunk_ref(chunk_ref.clone(), vec![])
                .await?;
            info!(event_id = %result.event_id, chunk_id = %chunk_ref.chunk_id, "Published chunk ref");
        }
    }

    if let Some(db_path) = args.index_db.as_ref() {
        index_to_db(db_path, &manifest, &chunk_refs).await?;
        info!(db = %db_path.display(), "Indexed into local Nostr DB");
    }

    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(12).collect()
}

fn chunk_text(text: &str, chunk_size: usize, chunk_overlap: usize) -> Vec<(usize, usize, String)> {
    let size = chunk_size.max(1);
    let overlap = chunk_overlap.min(size.saturating_sub(1));
    let chars: Vec<char> = text.chars().collect();
    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < chars.len() {
        let end = (start + size).min(chars.len());
        let chunk: String = chars[start..end].iter().collect();
        chunks.push((start, end, chunk));
        if end == chars.len() {
            break;
        }
        start = end.saturating_sub(overlap);
    }

    chunks
}

fn write_chunks(
    doc_id: &str,
    out_dir: &Path,
    chunks: &[(usize, usize, String)],
) -> Result<(Vec<DocChunk>, Vec<ChunkRef>), Box<dyn std::error::Error>> {
    let doc_dir = out_dir.join(doc_id);
    fs::create_dir_all(&doc_dir)?;

    let mut doc_chunks = Vec::new();
    let mut chunk_refs = Vec::new();

    for (ord, (start, end, chunk)) in chunks.iter().enumerate() {
        let chunk_id = format!("{}_chunk_{}", doc_id, ord);
        let chunk_hash = sha256_hex(chunk.as_bytes());
        let chunk_path = doc_dir.join(format!("chunk_{}.txt", ord));
        fs::write(&chunk_path, chunk)?;
        let blob_ref = chunk_path.canonicalize()?.display().to_string();

        let offsets = ChunkOffsets {
            start: *start as u64,
            end: *end as u64,
        };

        doc_chunks.push(DocChunk {
            chunk_id: chunk_id.clone(),
            ord: ord as u32,
            offsets: offsets.clone(),
            chunk_hash: format!("sha256:{}", chunk_hash),
            blob_ref: Some(blob_ref.clone()),
        });

        let mut chunk_ref = ChunkRef::new(
            chunk_id.clone(),
            doc_id.to_string(),
            ord as u32,
            offsets,
            format!("sha256:{}", chunk_hash),
        );
        chunk_ref.blob_ref = Some(blob_ref);
        chunk_refs.push(chunk_ref);
    }

    Ok((doc_chunks, chunk_refs))
}

async fn index_to_db(
    db_path: &Path,
    manifest: &DocManifest,
    chunk_refs: &[ChunkRef],
) -> Result<(), Box<dyn std::error::Error>> {
    let config = IndexerConfig {
        relays: vec![],
        authors: vec![],
        timeout: Duration::from_secs(1),
        kinds: NostrKinds::default(),
        db_path: db_path.to_path_buf(),
        backfill_since: None,
        backfill_limit: None,
        codec: std::sync::Arc::new(NoopCodec),
    };

    let indexer = NostrIndexerImpl::new(config).await?;
    let now = unix_timestamp();

    let manifest_event = NostrEvent {
        event_id: format!("local-{}", Uuid::new_v4()),
        kind: NostrKinds::default().doc_manifest.as_u16(),
        pubkey: "local".to_string(),
        created_at: now,
        content: serde_json::to_string(manifest)?,
        tags: vec![d_tag(&manifest.doc_id), k_tag(TAG_KIND_DOC_MANIFEST)],
        raw_json: "{}".to_string(),
    };

    indexer.handle_event(manifest_event).await?;

    for chunk in chunk_refs {
        let chunk_event = NostrEvent {
            event_id: format!("local-{}", Uuid::new_v4()),
            kind: NostrKinds::default().chunk_ref.as_u16(),
            pubkey: "local".to_string(),
            created_at: now,
            content: serde_json::to_string(chunk)?,
            tags: vec![d_tag(&chunk.chunk_id), k_tag(TAG_KIND_CHUNK_REF)],
            raw_json: "{}".to_string(),
        };
        indexer.handle_event(chunk_event).await?;
    }

    Ok(())
}

fn d_tag(value: &str) -> NostrTag {
    NostrTag::new("d", vec![value.to_string()])
}
