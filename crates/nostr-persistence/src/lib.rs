//! Nostr-backed persistence layer for document metadata and chunk references.
//!
//! This crate provides a publisher and indexer for Nostr events that store
//! document manifests, chunk references, and access policies. It's designed
//! for decentralized, censorship-resistant document storage metadata.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    NOSTR-PERSISTENCE                             │
//! │                                                                  │
//! │  ┌─────────────────────┐       ┌─────────────────────┐          │
//! │  │   NostrPublisher    │       │   NostrIndexer      │          │
//! │  │   (write path)      │       │   (read path)       │          │
//! │  │                     │       │                     │          │
//! │  │ - publish_doc       │       │ - subscribe relays  │          │
//! │  │ - publish_chunk     │       │ - index events      │          │
//! │  │ - publish_policy    │       │ - query SQLite      │          │
//! │  └──────────┬──────────┘       └──────────┬──────────┘          │
//! │             │                             │                      │
//! │             ▼                             ▼                      │
//! │       Nostr Relays                   SQLite DB                   │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Event Kinds
//!
//! | Kind | Constant | Purpose | Tag |
//! |------|----------|---------|-----|
//! | 30090 | `KIND_DOC_MANIFEST` | Document metadata | `d=doc_id` |
//! | 30091 | `KIND_CHUNK_REF` | Chunk reference | `d=chunk_id` |
//! | 30092 | `KIND_ACCESS_POLICY` | Access control | `d=scope_id` |
//!
//! All kinds are parameterized replaceable events (NIP-33).
//!
//! # Example: Publishing
//!
//! ```rust,ignore
//! use nostr_persistence::{NostrPublisherImpl, PublisherConfig, DocManifest, NoopCodec};
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! let config = PublisherConfig {
//!     relays: vec!["wss://relay.damus.io".to_string()],
//!     secret_key: "hex:...".to_string(),
//!     min_acks: 1,
//!     timeout: Duration::from_secs(30),
//!     kinds: Default::default(),
//!     codec: Arc::new(NoopCodec),
//! };
//!
//! let publisher = NostrPublisherImpl::new(config).await?;
//! let manifest = DocManifest::new("doc-123", "Title", "en", "text/plain", "file", "sha256:...", vec![]);
//! publisher.publish_doc_manifest(&manifest).await?;
//! ```
//!
//! # Example: Indexing
//!
//! ```rust,ignore
//! use nostr_persistence::{NostrIndexerImpl, IndexerConfig};
//!
//! let config = IndexerConfig {
//!     relays: vec!["wss://relay.damus.io".to_string()],
//!     authors: vec!["pubkey...".to_string()],
//!     timeout: Duration::from_secs(30),
//!     kinds: Default::default(),
//!     db_path: "./data/nostr.db".into(),
//!     backfill_since: None,
//!     backfill_limit: Some(1000),
//!     codec: Arc::new(NoopCodec),
//! };
//!
//! let indexer = NostrIndexerImpl::new(config).await?;
//! indexer.start().await?;
//! ```
//!
//! # Encryption
//!
//! Payloads can be encrypted using [`SecretBoxCodec`] with a symmetric key:
//!
//! ```rust,ignore
//! use nostr_persistence::SecretBoxCodec;
//!
//! // Set NOSTR_SECRETBOX_KEY environment variable (32-byte hex)
//! let codec = SecretBoxCodec::from_env()?;
//! ```
//!
//! # Traits
//!
//! - [`NostrPublisher`] - Publish document manifests, chunk refs, and policies
//! - [`NostrIndexer`] - Subscribe to relays and materialize events into SQLite

mod config;
mod crypto;
mod error;
mod events;
mod indexer;
mod memory;
mod publish;

pub use config::{IndexerConfig, NostrKinds, PublisherConfig};
pub use crypto::{codec_tag, CryptoError, NoopCodec, PayloadCodec, SecretBoxCodec};
pub use error::Error;
pub use events::{
    enc_tag, k_tag, tag_value, unix_timestamp, AccessPolicy, ChunkOffsets, ChunkRef, DocChunk,
    DocManifest, NostrEvent, NostrTag, KIND_ACCESS_POLICY, KIND_CHUNK_REF, KIND_DOC_MANIFEST,
    SCHEMA_VERSION, TAG_KIND_CHUNK_REF, TAG_KIND_DOC_MANIFEST, TAG_KIND_POLICY,
};
pub use indexer::{NostrIndexer, NostrIndexerImpl};
pub use memory::{
    decode_payload, encode_payload, hk_tag, project_memory, ts_tag, v_tag, AmanClearContextEvent,
    AmanPreferenceEvent, AmanSummaryEvent, AmanToolHistoryEvent, KIND_AMAN_CLEAR_CONTEXT,
    KIND_AMAN_PREFERENCE, KIND_AMAN_SUBSCRIPTION_STATE, KIND_AMAN_SUMMARY, KIND_AMAN_TOOL_HISTORY,
    MEMORY_SCHEMA_VERSION, MemoryProjectionStats, MemoryPublisherConfig, NostrMemoryPublisher,
    NostrMemoryPublisherImpl, TAG_KIND_AMAN_CLEAR_CONTEXT, TAG_KIND_AMAN_PREFERENCE,
    TAG_KIND_AMAN_SUBSCRIPTION_STATE, TAG_KIND_AMAN_SUMMARY, TAG_KIND_AMAN_TOOL_HISTORY,
};
pub use publish::{NostrPublisher, NostrPublisherImpl, PublishResult};

/// Crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
