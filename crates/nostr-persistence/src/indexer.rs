use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine;
use nostr_sdk::prelude::*;
use rusqlite::{params, Connection};
use tracing::{info, warn};

use crate::config::IndexerConfig;
use crate::events::{
    tag_value, AccessPolicy, ChunkRef, DocManifest, NostrEvent, TAG_KIND_POLICY,
};
use crate::events::{TAG_KIND_CHUNK_REF, TAG_KIND_DOC_MANIFEST};
use crate::Error;

#[async_trait]
pub trait NostrIndexer: Send + Sync {
    async fn backfill(&self) -> Result<(), Error>;
    async fn start(&self) -> Result<(), Error>;
    async fn handle_event(&self, event: NostrEvent) -> Result<(), Error>;
}

#[derive(Clone)]
pub struct NostrIndexerImpl {
    client: Client,
    config: IndexerConfig,
    db: Arc<Mutex<Connection>>,
}

impl NostrIndexerImpl {
    pub async fn new(config: IndexerConfig) -> Result<Self, Error> {
        let client = Client::default();
        for relay in &config.relays {
            client.add_relay(relay).await?;
        }
        client.connect().await;

        let conn = Connection::open(&config.db_path)?;
        init_schema(&conn)?;

        Ok(Self {
            client,
            config,
            db: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn db_path(&self) -> &Path {
        &self.config.db_path
    }

    fn decode_content(&self, event: &NostrEvent) -> Result<Vec<u8>, Error> {
        if let Some(enc) = tag_value(&event.tags, "enc") {
            let expected = self
                .config
                .codec
                .encoding_tag()
                .unwrap_or("noop");
            if expected != enc {
                return Err(Error::EncodingMismatch {
                    expected: expected.to_string(),
                    actual: enc.to_string(),
                });
            }
            let ciphertext = base64::engine::general_purpose::STANDARD.decode(&event.content)?;
            Ok(self.config.codec.decode(&ciphertext)?)
        } else {
            Ok(event.content.as_bytes().to_vec())
        }
    }

    fn insert_event(&self, event: &NostrEvent, d_tag: Option<&str>) -> Result<(), Error> {
        let now = crate::unix_timestamp() as i64;
        let conn = self.db.lock().map_err(|_| Error::MutexPoisoned)?;
        conn.execute(
            "INSERT INTO nostr_events (event_id, kind, author, d_tag, created_at, seen_at, raw_json)\
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)\
            ON CONFLICT(event_id) DO UPDATE SET seen_at = excluded.seen_at",
            params![
                &event.event_id,
                event.kind as i64,
                &event.pubkey,
                d_tag,
                event.created_at as i64,
                now,
                &event.raw_json
            ],
        )?;
        Ok(())
    }

    fn upsert_doc_manifest(&self, event: &NostrEvent, doc: DocManifest) -> Result<(), Error> {
        let doc_id = doc.doc_id.clone();
        {
            let mut conn = self.db.lock().map_err(|_| Error::MutexPoisoned)?;
            let tx = conn.transaction()?;

            tx.execute(
                "INSERT INTO docs (doc_id, title, lang, mime, source_type, content_hash, blob_ref, updated_at) \
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                ON CONFLICT(doc_id) DO UPDATE SET \
                  title = excluded.title, \
                  lang = excluded.lang, \
                  mime = excluded.mime, \
                  source_type = excluded.source_type, \
                  content_hash = excluded.content_hash, \
                  blob_ref = excluded.blob_ref, \
                  updated_at = excluded.updated_at \
                WHERE excluded.updated_at >= docs.updated_at",
                params![
                    &doc_id,
                    doc.title,
                    doc.lang,
                    doc.mime,
                    doc.source_type,
                    doc.content_hash,
                    doc.blob_ref,
                    doc.updated_at as i64
                ],
            )?;

            tx.execute("DELETE FROM chunks WHERE doc_id = ?1", params![&doc_id])?;

            for chunk in doc.chunks {
                tx.execute(
                    "INSERT INTO chunks (chunk_id, doc_id, ord, offset_start, offset_end, chunk_hash, blob_ref) \
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                    ON CONFLICT(chunk_id) DO UPDATE SET \
                      doc_id = excluded.doc_id, \
                      ord = excluded.ord, \
                      offset_start = excluded.offset_start, \
                      offset_end = excluded.offset_end, \
                      chunk_hash = excluded.chunk_hash, \
                      blob_ref = excluded.blob_ref",
                    params![
                        chunk.chunk_id,
                        &doc_id,
                        chunk.ord as i64,
                        chunk.offsets.start as i64,
                        chunk.offsets.end as i64,
                        chunk.chunk_hash,
                        chunk.blob_ref,
                    ],
                )?;
            }

            tx.commit()?;
        }
        self.insert_event(event, Some(&doc_id))?;
        Ok(())
    }

    fn upsert_chunk_ref(&self, event: &NostrEvent, chunk: ChunkRef) -> Result<(), Error> {
        let chunk_id = chunk.chunk_id.clone();
        {
            let conn = self.db.lock().map_err(|_| Error::MutexPoisoned)?;
            conn.execute(
                "INSERT INTO chunks (chunk_id, doc_id, ord, offset_start, offset_end, chunk_hash, blob_ref) \
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                ON CONFLICT(chunk_id) DO UPDATE SET \
                  doc_id = excluded.doc_id, \
                  ord = excluded.ord, \
                  offset_start = excluded.offset_start, \
                  offset_end = excluded.offset_end, \
                  chunk_hash = excluded.chunk_hash, \
                  blob_ref = excluded.blob_ref",
                params![
                    &chunk_id,
                    chunk.doc_id,
                    chunk.ord as i64,
                    chunk.offsets.start as i64,
                    chunk.offsets.end as i64,
                    chunk.chunk_hash,
                    chunk.blob_ref,
                ],
            )?;
        }
        self.insert_event(event, Some(&chunk_id))?;
        Ok(())
    }

    fn upsert_policy(&self, event: &NostrEvent, policy: AccessPolicy) -> Result<(), Error> {
        let scope_id = policy.scope_id.clone();
        let json = serde_json::to_string(&policy)?;
        {
            let conn = self.db.lock().map_err(|_| Error::MutexPoisoned)?;
            conn.execute(
                "INSERT INTO policies (scope_id, json, updated_at) VALUES (?1, ?2, ?3) \
                ON CONFLICT(scope_id) DO UPDATE SET \
                  json = excluded.json, \
                  updated_at = excluded.updated_at \
                WHERE excluded.updated_at >= policies.updated_at",
                params![&scope_id, json, policy.updated_at as i64],
            )?;
        }
        self.insert_event(event, Some(&scope_id))?;
        Ok(())
    }

    fn build_filter(&self) -> Result<Filter, Error> {
        let mut filter = Filter::new().kinds([
            self.config.kinds.doc_manifest,
            self.config.kinds.chunk_ref,
            self.config.kinds.access_policy,
        ]);

        if !self.config.authors.is_empty() {
            let authors = self.config.author_keys()?;
            filter = filter.authors(authors);
        }

        if let Some(since) = self.config.backfill_since {
            filter = filter.since(Timestamp::from(since));
        }

        if let Some(limit) = self.config.backfill_limit {
            filter = filter.limit(limit as usize);
        }

        Ok(filter)
    }
}

#[async_trait]
impl NostrIndexer for NostrIndexerImpl {
    async fn backfill(&self) -> Result<(), Error> {
        let filter = self.build_filter()?;
        let events = self
            .client
            .fetch_events(filter, self.config.timeout)
            .await?;

        for event in events.iter() {
            let parsed = NostrEvent::from_event(event);
            self.handle_event(parsed).await?;
        }

        Ok(())
    }

    async fn start(&self) -> Result<(), Error> {
        let mut filter = self.build_filter()?;
        filter = filter.since(Timestamp::now());
        self.client.subscribe(filter, None).await?;

        let indexer = self.clone();
        self.client
            .handle_notifications(move |notification| {
                let indexer = indexer.clone();
                async move {
                    if let RelayPoolNotification::Event { event, .. } = notification {
                        let parsed = NostrEvent::from_event(&event);
                        if let Err(err) = indexer.handle_event(parsed).await {
                            warn!(error = %err, "Failed to handle nostr event");
                        }
                    }
                    Ok(false)
                }
            })
            .await?;

        Ok(())
    }

    async fn handle_event(&self, event: NostrEvent) -> Result<(), Error> {
        let payload = self.decode_content(&event)?;
        let d_tag = tag_value(&event.tags, "d");

        if d_tag.is_none() {
            return Err(Error::MissingDTag);
        }

        match event.kind {
            k if k == self.config.kinds.doc_manifest.as_u16() => {
                let doc: DocManifest = serde_json::from_slice(&payload)?;
                if tag_value(&event.tags, "k") != Some(TAG_KIND_DOC_MANIFEST) {
                    warn!(event_id = %event.event_id, "Missing k=doc_manifest tag");
                }
                self.upsert_doc_manifest(&event, doc)?;
            }
            k if k == self.config.kinds.chunk_ref.as_u16() => {
                let chunk: ChunkRef = serde_json::from_slice(&payload)?;
                if tag_value(&event.tags, "k") != Some(TAG_KIND_CHUNK_REF) {
                    warn!(event_id = %event.event_id, "Missing k=chunk_ref tag");
                }
                self.upsert_chunk_ref(&event, chunk)?;
            }
            k if k == self.config.kinds.access_policy.as_u16() => {
                let policy: AccessPolicy = serde_json::from_slice(&payload)?;
                if tag_value(&event.tags, "k") != Some(TAG_KIND_POLICY) {
                    warn!(event_id = %event.event_id, "Missing k=policy tag");
                }
                self.upsert_policy(&event, policy)?;
            }
            _ => {
                warn!(event_id = %event.event_id, kind = event.kind, "Unhandled event kind");
            }
        }

        info!(event_id = %event.event_id, "Indexed nostr event");
        Ok(())
    }
}

fn init_schema(conn: &Connection) -> Result<(), Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS nostr_events (\
            event_id TEXT PRIMARY KEY,\
            kind INTEGER NOT NULL,\
            author TEXT NOT NULL,\
            d_tag TEXT,\
            created_at INTEGER NOT NULL,\
            seen_at INTEGER NOT NULL,\
            raw_json TEXT NOT NULL\
        );\
        CREATE TABLE IF NOT EXISTS docs (\
            doc_id TEXT PRIMARY KEY,\
            title TEXT NOT NULL,\
            lang TEXT NOT NULL,\
            mime TEXT NOT NULL,\
            source_type TEXT NOT NULL,\
            content_hash TEXT NOT NULL,\
            blob_ref TEXT,\
            updated_at INTEGER NOT NULL\
        );\
        CREATE TABLE IF NOT EXISTS chunks (\
            chunk_id TEXT PRIMARY KEY,\
            doc_id TEXT NOT NULL,\
            ord INTEGER NOT NULL,\
            offset_start INTEGER,\
            offset_end INTEGER,\
            chunk_hash TEXT NOT NULL,\
            blob_ref TEXT\
        );\
        CREATE INDEX IF NOT EXISTS idx_chunks_doc_id ON chunks(doc_id);\
        CREATE TABLE IF NOT EXISTS policies (\
            scope_id TEXT PRIMARY KEY,\
            json TEXT NOT NULL,\
            updated_at INTEGER NOT NULL\
        );",
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::crypto::NoopCodec;
    use crate::events::{ChunkOffsets, DocChunk, DocManifest, NostrTag};
    use rusqlite::OptionalExtension;

    #[tokio::test]
    async fn test_replaceable_doc_manifest_upsert() {
        let db_path = std::env::temp_dir().join("nostr_test_replaceable.db");
        let _ = std::fs::remove_file(&db_path);

        let config = IndexerConfig {
            relays: vec![],
            authors: vec![],
            timeout: Duration::from_secs(1),
            kinds: crate::NostrKinds::default(),
            db_path: db_path.clone(),
            backfill_since: None,
            backfill_limit: None,
            codec: Arc::new(NoopCodec),
        };

        let indexer = NostrIndexerImpl::new(config).await.unwrap();

        let chunk = DocChunk {
            chunk_id: "chunk-1".to_string(),
            ord: 0,
            offsets: ChunkOffsets { start: 0, end: 10 },
            chunk_hash: "sha256:aaa".to_string(),
            blob_ref: None,
        };

        let mut doc = DocManifest::new(
            "doc-1",
            "Title",
            "en",
            "text/plain",
            "signal_paste",
            "sha256:doc",
            vec![chunk],
        );
        doc.updated_at = 10;

        let event = NostrEvent {
            event_id: "event-1".to_string(),
            kind: crate::KIND_DOC_MANIFEST,
            pubkey: "pubkey".to_string(),
            created_at: 10,
            content: serde_json::to_string(&doc).unwrap(),
            tags: vec![NostrTag::new("d", vec!["doc-1".to_string()]), NostrTag::new("k", vec![TAG_KIND_DOC_MANIFEST.to_string()])],
            raw_json: "{}".to_string(),
        };

        indexer.handle_event(event).await.unwrap();

        let mut doc_updated = doc.clone();
        doc_updated.title = "Updated".to_string();
        doc_updated.updated_at = 20;

        let event2 = NostrEvent {
            event_id: "event-2".to_string(),
            kind: crate::KIND_DOC_MANIFEST,
            pubkey: "pubkey".to_string(),
            created_at: 20,
            content: serde_json::to_string(&doc_updated).unwrap(),
            tags: vec![NostrTag::new("d", vec!["doc-1".to_string()]), NostrTag::new("k", vec![TAG_KIND_DOC_MANIFEST.to_string()])],
            raw_json: "{}".to_string(),
        };

        indexer.handle_event(event2).await.unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let title: Option<String> = conn
            .query_row(
                "SELECT title FROM docs WHERE doc_id = ?1",
                params!["doc-1"],
                |row| row.get(0),
            )
            .optional()
            .unwrap();

        assert_eq!(title, Some("Updated".to_string()));
        let _ = std::fs::remove_file(&db_path);
    }
}
