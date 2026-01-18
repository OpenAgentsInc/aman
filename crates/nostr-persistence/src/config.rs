use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nostr_sdk::prelude::*;

use crate::crypto::PayloadCodec;
use crate::events::{KIND_ACCESS_POLICY, KIND_CHUNK_REF, KIND_DOC_MANIFEST};
use crate::memory::{
    KIND_AMAN_CLEAR_CONTEXT, KIND_AMAN_PREFERENCE, KIND_AMAN_SUBSCRIPTION_STATE,
    KIND_AMAN_SUMMARY, KIND_AMAN_TOOL_HISTORY,
};
use crate::Error;

#[derive(Clone)]
pub struct NostrKinds {
    pub doc_manifest: Kind,
    pub chunk_ref: Kind,
    pub access_policy: Kind,
    pub memory_preference: Kind,
    pub memory_summary: Kind,
    pub memory_tool_history: Kind,
    pub memory_clear_context: Kind,
    pub memory_subscription_state: Kind,
}

impl Default for NostrKinds {
    fn default() -> Self {
        Self {
            doc_manifest: Kind::Custom(KIND_DOC_MANIFEST),
            chunk_ref: Kind::Custom(KIND_CHUNK_REF),
            access_policy: Kind::Custom(KIND_ACCESS_POLICY),
            memory_preference: Kind::Custom(KIND_AMAN_PREFERENCE),
            memory_summary: Kind::Custom(KIND_AMAN_SUMMARY),
            memory_tool_history: Kind::Custom(KIND_AMAN_TOOL_HISTORY),
            memory_clear_context: Kind::Custom(KIND_AMAN_CLEAR_CONTEXT),
            memory_subscription_state: Kind::Custom(KIND_AMAN_SUBSCRIPTION_STATE),
        }
    }
}

#[derive(Clone)]
pub struct PublisherConfig {
    pub relays: Vec<String>,
    pub secret_key: String,
    pub min_acks: usize,
    pub timeout: Duration,
    pub kinds: NostrKinds,
    pub codec: Arc<dyn PayloadCodec>,
}

impl PublisherConfig {
    pub fn keys(&self) -> Result<Keys, Error> {
        Ok(Keys::parse(&self.secret_key)?)
    }
}

#[derive(Clone)]
pub struct IndexerConfig {
    pub relays: Vec<String>,
    pub authors: Vec<String>,
    pub timeout: Duration,
    pub kinds: NostrKinds,
    pub db_path: PathBuf,
    pub backfill_since: Option<u64>,
    pub backfill_limit: Option<u64>,
    pub codec: Arc<dyn PayloadCodec>,
}

impl IndexerConfig {
    pub fn author_keys(&self) -> Result<Vec<PublicKey>, Error> {
        self.authors
            .iter()
            .map(|value| PublicKey::parse(value).map_err(Error::from))
            .collect()
    }
}
