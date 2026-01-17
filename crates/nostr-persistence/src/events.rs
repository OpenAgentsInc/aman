use std::time::{SystemTime, UNIX_EPOCH};

use nostr_sdk::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Error;

pub const SCHEMA_VERSION: u32 = 1;

pub const KIND_DOC_MANIFEST: u16 = 30090;
pub const KIND_CHUNK_REF: u16 = 30091;
pub const KIND_ACCESS_POLICY: u16 = 30092;

pub const TAG_KIND_DOC_MANIFEST: &str = "doc_manifest";
pub const TAG_KIND_CHUNK_REF: &str = "chunk_ref";
pub const TAG_KIND_POLICY: &str = "policy";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkOffsets {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocChunk {
    pub chunk_id: String,
    pub ord: u32,
    pub offsets: ChunkOffsets,
    pub chunk_hash: String,
    pub blob_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocManifest {
    pub schema_version: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub doc_id: String,
    pub title: String,
    pub lang: String,
    pub mime: String,
    pub source_type: String,
    pub content_hash: String,
    pub blob_ref: Option<String>,
    pub chunks: Vec<DocChunk>,
}

impl DocManifest {
    pub fn new(
        doc_id: impl Into<String>,
        title: impl Into<String>,
        lang: impl Into<String>,
        mime: impl Into<String>,
        source_type: impl Into<String>,
        content_hash: impl Into<String>,
        chunks: Vec<DocChunk>,
    ) -> Self {
        let now = unix_timestamp();
        Self {
            schema_version: SCHEMA_VERSION,
            created_at: now,
            updated_at: now,
            doc_id: doc_id.into(),
            title: title.into(),
            lang: lang.into(),
            mime: mime.into(),
            source_type: source_type.into(),
            content_hash: content_hash.into(),
            blob_ref: None,
            chunks,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkRef {
    pub schema_version: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub chunk_id: String,
    pub doc_id: String,
    pub ord: u32,
    pub offsets: ChunkOffsets,
    pub chunk_hash: String,
    pub blob_ref: Option<String>,
}

impl ChunkRef {
    pub fn new(
        chunk_id: impl Into<String>,
        doc_id: impl Into<String>,
        ord: u32,
        offsets: ChunkOffsets,
        chunk_hash: impl Into<String>,
    ) -> Self {
        let now = unix_timestamp();
        Self {
            schema_version: SCHEMA_VERSION,
            created_at: now,
            updated_at: now,
            chunk_id: chunk_id.into(),
            doc_id: doc_id.into(),
            ord,
            offsets,
            chunk_hash: chunk_hash.into(),
            blob_ref: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessPolicy {
    pub schema_version: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub scope_id: String,
    pub readers: Vec<String>,
    pub notes: Option<String>,
}

impl AccessPolicy {
    pub fn new(scope_id: impl Into<String>, readers: Vec<String>) -> Self {
        let now = unix_timestamp();
        Self {
            schema_version: SCHEMA_VERSION,
            created_at: now,
            updated_at: now,
            scope_id: scope_id.into(),
            readers,
            notes: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NostrTag {
    pub name: String,
    pub values: Vec<String>,
}

impl NostrTag {
    pub fn new(name: impl Into<String>, values: Vec<String>) -> Self {
        Self {
            name: name.into(),
            values,
        }
    }

    pub fn to_sdk_tag(&self) -> Result<Tag, Error> {
        let mut parts = Vec::with_capacity(1 + self.values.len());
        parts.push(self.name.clone());
        parts.extend(self.values.clone());
        Ok(Tag::parse(parts)?)
    }

    pub fn from_sdk_tag(tag: &Tag) -> Self {
        let parts = tag.clone().to_vec();
        let name = parts.first().cloned().unwrap_or_default();
        let values = parts.into_iter().skip(1).collect();
        Self { name, values }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NostrEvent {
    pub event_id: String,
    pub kind: u16,
    pub pubkey: String,
    pub created_at: u64,
    pub content: String,
    pub tags: Vec<NostrTag>,
    pub raw_json: String,
}

impl NostrEvent {
    pub fn from_event(event: &Event) -> Self {
        let tags = event
            .tags
            .iter()
            .map(NostrTag::from_sdk_tag)
            .collect();

        Self {
            event_id: event.id.to_string(),
            kind: event.kind.as_u16(),
            pubkey: event.pubkey.to_string(),
            created_at: event.created_at.as_secs(),
            content: event.content.clone(),
            tags,
            raw_json: event.as_json(),
        }
    }
}

pub fn d_tag(id: &str) -> NostrTag {
    NostrTag::new("d", vec![id.to_string()])
}

pub fn k_tag(value: &str) -> NostrTag {
    NostrTag::new("k", vec![value.to_string()])
}

pub fn enc_tag(value: &str) -> NostrTag {
    NostrTag::new("enc", vec![value.to_string()])
}

pub fn tag_value<'a>(tags: &'a [NostrTag], name: &str) -> Option<&'a str> {
    tags.iter()
        .find(|tag| tag.name == name)
        .and_then(|tag| tag.values.first().map(|s| s.as_str()))
}

pub fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doc_manifest_roundtrip() {
        let chunk = DocChunk {
            chunk_id: "chunk-1".to_string(),
            ord: 0,
            offsets: ChunkOffsets { start: 0, end: 120 },
            chunk_hash: "sha256:abc".to_string(),
            blob_ref: None,
        };
        let doc = DocManifest::new(
            "doc-1",
            "Title",
            "en",
            "text/plain",
            "signal_paste",
            "sha256:doc",
            vec![chunk],
        );
        let json = serde_json::to_vec(&doc).unwrap();
        let parsed: DocManifest = serde_json::from_slice(&json).unwrap();
        assert_eq!(doc, parsed);
    }

    #[test]
    fn test_chunk_ref_roundtrip() {
        let chunk = ChunkRef::new(
            "chunk-1",
            "doc-1",
            0,
            ChunkOffsets { start: 0, end: 10 },
            "sha256:chunk",
        );
        let json = serde_json::to_vec(&chunk).unwrap();
        let parsed: ChunkRef = serde_json::from_slice(&json).unwrap();
        assert_eq!(chunk, parsed);
    }

    #[test]
    fn test_access_policy_roundtrip() {
        let policy = AccessPolicy::new("workspace-1", vec!["npub123".to_string()]);
        let json = serde_json::to_vec(&policy).unwrap();
        let parsed: AccessPolicy = serde_json::from_slice(&json).unwrap();
        assert_eq!(policy, parsed);
    }

    #[test]
    fn test_tag_helpers() {
        let tags = vec![d_tag("doc-1"), k_tag(TAG_KIND_DOC_MANIFEST)];
        assert_eq!(tag_value(&tags, "d"), Some("doc-1"));
        assert_eq!(tag_value(&tags, "k"), Some(TAG_KIND_DOC_MANIFEST));
    }

    #[test]
    fn test_tag_conversion() {
        let tag = NostrTag::new("d", vec!["doc-1".to_string()]);
        let sdk_tag = tag.to_sdk_tag().unwrap();
        let roundtrip = NostrTag::from_sdk_tag(&sdk_tag);
        assert_eq!(tag, roundtrip);
    }
}
