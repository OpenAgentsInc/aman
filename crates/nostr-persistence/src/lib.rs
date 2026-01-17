//! Nostr-backed persistence layer for Aman.

mod config;
mod crypto;
mod error;
mod events;
mod indexer;
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
pub use publish::{NostrPublisher, NostrPublisherImpl, PublishResult};

/// Crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
