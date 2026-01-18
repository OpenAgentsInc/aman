use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine;
use nostr_sdk::prelude::*;
use rusqlite::{params, Connection};
use tracing::{info, warn};

use crate::config::IndexerConfig;
use crate::events::{
    tag_value, AccessPolicy, ChunkRef, DocManifest, NostrEvent, NostrTag, TAG_KIND_POLICY,
};
use crate::events::{TAG_KIND_CHUNK_REF, TAG_KIND_DOC_MANIFEST};
use crate::memory::{
    AmanClearContextEvent, AmanPreferenceEvent, AmanSummaryEvent, AmanToolHistoryEvent,
    MEMORY_SCHEMA_VERSION, TAG_KIND_AMAN_CLEAR_CONTEXT, TAG_KIND_AMAN_PREFERENCE,
    TAG_KIND_AMAN_SUMMARY, TAG_KIND_AMAN_TOOL_HISTORY,
};
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

    fn tag_u64(tags: &[NostrTag], name: &str) -> Option<u64> {
        tag_value(tags, name).and_then(|value| value.parse().ok())
    }

    fn tag_u32(tags: &[NostrTag], name: &str) -> Option<u32> {
        tag_value(tags, name).and_then(|value| value.parse().ok())
    }

    fn warn_if_tag_mismatch(
        event: &NostrEvent,
        name: &str,
        expected: &str,
    ) {
        if let Some(actual) = tag_value(&event.tags, name) {
            if actual != expected {
                warn!(
                    event_id = %event.event_id,
                    tag = name,
                    expected,
                    actual,
                    "Mismatched tag value"
                );
            }
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
                    "INSERT INTO chunks (chunk_id, doc_id, ord, offset_start, offset_end, chunk_hash, blob_ref, text) \
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                    ON CONFLICT(chunk_id) DO UPDATE SET \
                      doc_id = excluded.doc_id, \
                      ord = excluded.ord, \
                      offset_start = excluded.offset_start, \
                      offset_end = excluded.offset_end, \
                      chunk_hash = excluded.chunk_hash, \
                      blob_ref = excluded.blob_ref, \
                      text = excluded.text",
                    params![
                        chunk.chunk_id,
                        &doc_id,
                        chunk.ord as i64,
                        chunk.offsets.start as i64,
                        chunk.offsets.end as i64,
                        chunk.chunk_hash,
                        chunk.blob_ref,
                        Option::<String>::None,
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
                "INSERT INTO chunks (chunk_id, doc_id, ord, offset_start, offset_end, chunk_hash, blob_ref, text) \
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                ON CONFLICT(chunk_id) DO UPDATE SET \
                  doc_id = excluded.doc_id, \
                  ord = excluded.ord, \
                  offset_start = excluded.offset_start, \
                  offset_end = excluded.offset_end, \
                  chunk_hash = excluded.chunk_hash, \
                  blob_ref = excluded.blob_ref, \
                  text = excluded.text",
                params![
                    &chunk_id,
                    chunk.doc_id,
                    chunk.ord as i64,
                    chunk.offsets.start as i64,
                    chunk.offsets.end as i64,
                    chunk.chunk_hash,
                    chunk.blob_ref,
                    chunk.text,
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

    fn upsert_memory_preference(
        &self,
        event: &NostrEvent,
        preference: AmanPreferenceEvent,
    ) -> Result<(), Error> {
        let history_key = preference.history_key.clone();
        let updated_at = Self::tag_u64(&event.tags, "ts").unwrap_or(preference.updated_at);
        let schema_version =
            Self::tag_u32(&event.tags, "v").unwrap_or(MEMORY_SCHEMA_VERSION) as i64;
        if let Some(tag_hk) = tag_value(&event.tags, "hk") {
            if tag_hk != history_key {
                warn!(
                    event_id = %event.event_id,
                    tag = "hk",
                    expected = %history_key,
                    actual = %tag_hk,
                    "History key mismatch"
                );
            }
        }

        {
            let conn = self.db.lock().map_err(|_| Error::MutexPoisoned)?;
            conn.execute(
                "INSERT INTO nostr_memory_preferences \
                    (history_key, preference, updated_at, nostr_event_id, nostr_created_at, nostr_relay, schema_version, d_tag) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                 ON CONFLICT(history_key) DO UPDATE SET \
                    preference = excluded.preference, \
                    updated_at = excluded.updated_at, \
                    nostr_event_id = excluded.nostr_event_id, \
                    nostr_created_at = excluded.nostr_created_at, \
                    nostr_relay = excluded.nostr_relay, \
                    schema_version = excluded.schema_version, \
                    d_tag = excluded.d_tag \
                 WHERE excluded.updated_at > nostr_memory_preferences.updated_at \
                    OR (excluded.updated_at = nostr_memory_preferences.updated_at \
                        AND excluded.nostr_event_id > nostr_memory_preferences.nostr_event_id)",
                params![
                    &history_key,
                    preference.preference,
                    updated_at as i64,
                    &event.event_id,
                    event.created_at as i64,
                    Option::<String>::None,
                    schema_version,
                    tag_value(&event.tags, "d"),
                ],
            )?;
        }

        self.insert_event(event, Some(&history_key))?;
        Ok(())
    }

    fn upsert_memory_summary(
        &self,
        event: &NostrEvent,
        summary: AmanSummaryEvent,
    ) -> Result<(), Error> {
        let history_key = summary.history_key.clone();
        let updated_at = Self::tag_u64(&event.tags, "ts").unwrap_or(summary.updated_at);
        let schema_version =
            Self::tag_u32(&event.tags, "v").unwrap_or(MEMORY_SCHEMA_VERSION) as i64;
        if let Some(tag_hk) = tag_value(&event.tags, "hk") {
            if tag_hk != history_key {
                warn!(
                    event_id = %event.event_id,
                    tag = "hk",
                    expected = %history_key,
                    actual = %tag_hk,
                    "History key mismatch"
                );
            }
        }

        {
            let conn = self.db.lock().map_err(|_| Error::MutexPoisoned)?;
            conn.execute(
                "INSERT INTO nostr_memory_summaries \
                    (history_key, summary, message_count, updated_at, nostr_event_id, nostr_created_at, nostr_relay, schema_version, d_tag) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
                 ON CONFLICT(history_key) DO UPDATE SET \
                    summary = excluded.summary, \
                    message_count = excluded.message_count, \
                    updated_at = excluded.updated_at, \
                    nostr_event_id = excluded.nostr_event_id, \
                    nostr_created_at = excluded.nostr_created_at, \
                    nostr_relay = excluded.nostr_relay, \
                    schema_version = excluded.schema_version, \
                    d_tag = excluded.d_tag \
                 WHERE excluded.updated_at > nostr_memory_summaries.updated_at \
                    OR (excluded.updated_at = nostr_memory_summaries.updated_at \
                        AND excluded.nostr_event_id > nostr_memory_summaries.nostr_event_id)",
                params![
                    &history_key,
                    summary.summary,
                    summary.message_count,
                    updated_at as i64,
                    &event.event_id,
                    event.created_at as i64,
                    Option::<String>::None,
                    schema_version,
                    tag_value(&event.tags, "d"),
                ],
            )?;
        }

        self.insert_event(event, Some(&history_key))?;
        Ok(())
    }

    fn insert_memory_tool_history(
        &self,
        event: &NostrEvent,
        entry: AmanToolHistoryEvent,
    ) -> Result<(), Error> {
        let history_key = entry.history_key.clone();
        let created_at = Self::tag_u64(&event.tags, "ts").unwrap_or(entry.created_at);
        let schema_version =
            Self::tag_u32(&event.tags, "v").unwrap_or(MEMORY_SCHEMA_VERSION) as i64;
        if let Some(tag_hk) = tag_value(&event.tags, "hk") {
            if tag_hk != history_key {
                warn!(
                    event_id = %event.event_id,
                    tag = "hk",
                    expected = %history_key,
                    actual = %tag_hk,
                    "History key mismatch"
                );
            }
        }

        {
            let conn = self.db.lock().map_err(|_| Error::MutexPoisoned)?;
            conn.execute(
                "INSERT INTO nostr_memory_tool_history \
                    (d_tag, history_key, tool_name, success, content, sender_id, group_id, created_at, nostr_event_id, nostr_created_at, nostr_relay, schema_version) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12) \
                 ON CONFLICT(d_tag) DO NOTHING",
                params![
                    tag_value(&event.tags, "d"),
                    &history_key,
                    entry.tool_name,
                    entry.success as i64,
                    entry.content,
                    entry.sender_id,
                    entry.group_id,
                    created_at as i64,
                    &event.event_id,
                    event.created_at as i64,
                    Option::<String>::None,
                    schema_version,
                ],
            )?;
        }

        self.insert_event(event, Some(&history_key))?;
        Ok(())
    }

    fn insert_memory_clear_context(
        &self,
        event: &NostrEvent,
        entry: AmanClearContextEvent,
    ) -> Result<(), Error> {
        let history_key = entry.history_key.clone();
        let created_at = Self::tag_u64(&event.tags, "ts").unwrap_or(entry.created_at);
        let schema_version =
            Self::tag_u32(&event.tags, "v").unwrap_or(MEMORY_SCHEMA_VERSION) as i64;
        if let Some(tag_hk) = tag_value(&event.tags, "hk") {
            if tag_hk != history_key {
                warn!(
                    event_id = %event.event_id,
                    tag = "hk",
                    expected = %history_key,
                    actual = %tag_hk,
                    "History key mismatch"
                );
            }
        }

        {
            let conn = self.db.lock().map_err(|_| Error::MutexPoisoned)?;
            conn.execute(
                "INSERT INTO nostr_memory_clear_events \
                    (d_tag, history_key, sender_id, created_at, nostr_event_id, nostr_created_at, nostr_relay, schema_version) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                 ON CONFLICT(d_tag) DO NOTHING",
                params![
                    tag_value(&event.tags, "d"),
                    &history_key,
                    entry.sender_id,
                    created_at as i64,
                    &event.event_id,
                    event.created_at as i64,
                    Option::<String>::None,
                    schema_version,
                ],
            )?;
        }

        self.insert_event(event, Some(&history_key))?;
        Ok(())
    }

    fn build_filter(&self) -> Result<Filter, Error> {
        let mut filter = Filter::new().kinds([
            self.config.kinds.doc_manifest,
            self.config.kinds.chunk_ref,
            self.config.kinds.access_policy,
            self.config.kinds.memory_preference,
            self.config.kinds.memory_summary,
            self.config.kinds.memory_tool_history,
            self.config.kinds.memory_clear_context,
            self.config.kinds.memory_subscription_state,
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
                Self::warn_if_tag_mismatch(&event, "k", TAG_KIND_DOC_MANIFEST);
                self.upsert_doc_manifest(&event, doc)?;
            }
            k if k == self.config.kinds.chunk_ref.as_u16() => {
                let chunk: ChunkRef = serde_json::from_slice(&payload)?;
                Self::warn_if_tag_mismatch(&event, "k", TAG_KIND_CHUNK_REF);
                self.upsert_chunk_ref(&event, chunk)?;
            }
            k if k == self.config.kinds.access_policy.as_u16() => {
                let policy: AccessPolicy = serde_json::from_slice(&payload)?;
                Self::warn_if_tag_mismatch(&event, "k", TAG_KIND_POLICY);
                self.upsert_policy(&event, policy)?;
            }
            k if k == self.config.kinds.memory_preference.as_u16() => {
                let pref: AmanPreferenceEvent = serde_json::from_slice(&payload)?;
                Self::warn_if_tag_mismatch(&event, "k", TAG_KIND_AMAN_PREFERENCE);
                self.upsert_memory_preference(&event, pref)?;
            }
            k if k == self.config.kinds.memory_summary.as_u16() => {
                let summary: AmanSummaryEvent = serde_json::from_slice(&payload)?;
                Self::warn_if_tag_mismatch(&event, "k", TAG_KIND_AMAN_SUMMARY);
                self.upsert_memory_summary(&event, summary)?;
            }
            k if k == self.config.kinds.memory_tool_history.as_u16() => {
                let entry: AmanToolHistoryEvent = serde_json::from_slice(&payload)?;
                Self::warn_if_tag_mismatch(&event, "k", TAG_KIND_AMAN_TOOL_HISTORY);
                self.insert_memory_tool_history(&event, entry)?;
            }
            k if k == self.config.kinds.memory_clear_context.as_u16() => {
                let entry: AmanClearContextEvent = serde_json::from_slice(&payload)?;
                Self::warn_if_tag_mismatch(&event, "k", TAG_KIND_AMAN_CLEAR_CONTEXT);
                self.insert_memory_clear_context(&event, entry)?;
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
            blob_ref TEXT,\
            text TEXT\
        );\
        CREATE INDEX IF NOT EXISTS idx_chunks_doc_id ON chunks(doc_id);\
        CREATE TABLE IF NOT EXISTS policies (\
            scope_id TEXT PRIMARY KEY,\
            json TEXT NOT NULL,\
            updated_at INTEGER NOT NULL\
        );\
        CREATE TABLE IF NOT EXISTS nostr_memory_preferences (\
            history_key TEXT PRIMARY KEY,\
            preference TEXT NOT NULL,\
            updated_at INTEGER NOT NULL,\
            nostr_event_id TEXT NOT NULL,\
            nostr_created_at INTEGER NOT NULL,\
            nostr_relay TEXT,\
            schema_version INTEGER NOT NULL,\
            d_tag TEXT\
        );\
        CREATE TABLE IF NOT EXISTS nostr_memory_summaries (\
            history_key TEXT PRIMARY KEY,\
            summary TEXT NOT NULL,\
            message_count INTEGER NOT NULL,\
            updated_at INTEGER NOT NULL,\
            nostr_event_id TEXT NOT NULL,\
            nostr_created_at INTEGER NOT NULL,\
            nostr_relay TEXT,\
            schema_version INTEGER NOT NULL,\
            d_tag TEXT\
        );\
        CREATE TABLE IF NOT EXISTS nostr_memory_tool_history (\
            d_tag TEXT PRIMARY KEY,\
            history_key TEXT NOT NULL,\
            tool_name TEXT NOT NULL,\
            success INTEGER NOT NULL,\
            content TEXT NOT NULL,\
            sender_id TEXT,\
            group_id TEXT,\
            created_at INTEGER NOT NULL,\
            nostr_event_id TEXT NOT NULL,\
            nostr_created_at INTEGER NOT NULL,\
            nostr_relay TEXT,\
            schema_version INTEGER NOT NULL\
        );\
        CREATE INDEX IF NOT EXISTS idx_nostr_memory_tool_history_history_key \
            ON nostr_memory_tool_history(history_key);\
        CREATE INDEX IF NOT EXISTS idx_nostr_memory_tool_history_created_at \
            ON nostr_memory_tool_history(created_at);\
        CREATE TABLE IF NOT EXISTS nostr_memory_clear_events (\
            d_tag TEXT PRIMARY KEY,\
            history_key TEXT NOT NULL,\
            sender_id TEXT,\
            created_at INTEGER NOT NULL,\
            nostr_event_id TEXT NOT NULL,\
            nostr_created_at INTEGER NOT NULL,\
            nostr_relay TEXT,\
            schema_version INTEGER NOT NULL\
        );\
        CREATE INDEX IF NOT EXISTS idx_nostr_memory_clear_events_history_key \
            ON nostr_memory_clear_events(history_key);\
        CREATE INDEX IF NOT EXISTS idx_nostr_memory_clear_events_created_at \
            ON nostr_memory_clear_events(created_at);",
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
    use crate::memory::{AmanPreferenceEvent, TAG_KIND_AMAN_PREFERENCE};
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

    #[tokio::test]
    async fn test_memory_preference_lww_tiebreak() {
        let db_path = std::env::temp_dir().join("nostr_test_memory_pref.db");
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

        let pref = AmanPreferenceEvent {
            history_key: "hk-1".to_string(),
            preference: "opt-in".to_string(),
            updated_at: 100,
        };

        let tags = vec![
            NostrTag::new("d", vec!["hk-1:preference".to_string()]),
            NostrTag::new("k", vec![TAG_KIND_AMAN_PREFERENCE.to_string()]),
            NostrTag::new("hk", vec!["hk-1".to_string()]),
            NostrTag::new("v", vec!["1".to_string()]),
            NostrTag::new("ts", vec!["100".to_string()]),
        ];

        let event_a = NostrEvent {
            event_id: "event-a".to_string(),
            kind: crate::KIND_AMAN_PREFERENCE,
            pubkey: "pubkey".to_string(),
            created_at: 100,
            content: serde_json::to_string(&pref).unwrap(),
            tags: tags.clone(),
            raw_json: "{}".to_string(),
        };

        indexer.handle_event(event_a).await.unwrap();

        let mut pref_new = pref.clone();
        pref_new.preference = "opt-out".to_string();

        let mut tags_b = tags.clone();
        tags_b.retain(|tag| tag.name != "ts");
        tags_b.push(NostrTag::new("ts", vec!["100".to_string()]));

        let event_b = NostrEvent {
            event_id: "event-b".to_string(),
            kind: crate::KIND_AMAN_PREFERENCE,
            pubkey: "pubkey".to_string(),
            created_at: 100,
            content: serde_json::to_string(&pref_new).unwrap(),
            tags: tags_b,
            raw_json: "{}".to_string(),
        };

        indexer.handle_event(event_b).await.unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let stored: Option<String> = conn
            .query_row(
                "SELECT preference FROM nostr_memory_preferences WHERE history_key = ?1",
                params!["hk-1"],
                |row| row.get(0),
            )
            .optional()
            .unwrap();

        assert_eq!(stored, Some("opt-out".to_string()));
        let _ = std::fs::remove_file(&db_path);
    }
}
