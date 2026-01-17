use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nostr_persistence::{
    ChunkOffsets, DocChunk, DocManifest, IndexerConfig, NostrIndexer, NostrIndexerImpl,
    NostrKinds, NostrPublisher, NostrPublisherImpl, NoopCodec, PublisherConfig,
};
use nostr_sdk::prelude::*;
use rusqlite::{params, Connection, OptionalExtension};

#[tokio::test]
#[ignore]
async fn publish_and_index_fixture() {
    let relay = std::env::var("NOSTR_TEST_RELAY").expect("NOSTR_TEST_RELAY missing");
    let secret = std::env::var("NOSTR_TEST_KEY").expect("NOSTR_TEST_KEY missing");

    let keys = Keys::parse(&secret).expect("invalid secret key");
    let author = keys.public_key().to_string();

    let doc_id = format!("doc_test_{}", nostr_persistence::unix_timestamp());
    let chunk = DocChunk {
        chunk_id: format!("chunk_{}", doc_id),
        ord: 0,
        offsets: ChunkOffsets { start: 0, end: 32 },
        chunk_hash: "sha256:test".to_string(),
        blob_ref: None,
    };

    let doc = DocManifest::new(
        doc_id.clone(),
        "Test Doc",
        "en",
        "text/plain",
        "fixture",
        "sha256:doc",
        vec![chunk],
    );

    let publisher = NostrPublisherImpl::new(PublisherConfig {
        relays: vec![relay.clone()],
        secret_key: secret,
        min_acks: 1,
        timeout: Duration::from_secs(10),
        kinds: NostrKinds::default(),
        codec: Arc::new(NoopCodec),
    })
    .await
    .unwrap();

    publisher.publish_doc_manifest(doc, vec![]).await.unwrap();

    let db_path = temp_db_path();
    let indexer = NostrIndexerImpl::new(IndexerConfig {
        relays: vec![relay],
        authors: vec![author],
        timeout: Duration::from_secs(10),
        kinds: NostrKinds::default(),
        db_path: db_path.clone(),
        backfill_since: None,
        backfill_limit: None,
        codec: Arc::new(NoopCodec),
    })
    .await
    .unwrap();

    indexer.backfill().await.unwrap();

    let conn = Connection::open(db_path).unwrap();
    let found: Option<String> = conn
        .query_row(
            "SELECT doc_id FROM docs WHERE doc_id = ?1",
            params![doc_id],
            |row| row.get(0),
        )
        .optional()
        .unwrap();

    assert!(found.is_some());
}

fn temp_db_path() -> PathBuf {
    let name = format!("nostr_indexer_test_{}.db", nostr_persistence::unix_timestamp());
    std::env::temp_dir().join(name)
}
