use thiserror::Error;

use crate::crypto::CryptoError;

#[derive(Debug, Error)]
pub enum Error {
    #[error("nostr client error: {0}")]
    NostrClient(#[from] nostr_sdk::client::Error),
    #[error("nostr key error: {0}")]
    NostrKey(#[from] nostr_sdk::nostr::key::Error),
    #[error("nostr tag error: {0}")]
    NostrTag(#[from] nostr_sdk::nostr::event::tag::Error),
    #[error("serde json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("base64 error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("crypto error: {0}")]
    Crypto(#[from] CryptoError),
    #[error("invalid utf8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("missing env var: {0}")]
    MissingEnv(&'static str),
    #[error("missing d tag")]
    MissingDTag,
    #[error("unknown event kind: {0}")]
    UnknownKind(u16),
    #[error("publish quorum failed: required {required}, got {actual}")]
    Quorum { required: usize, actual: usize },
    #[error("encoding tag mismatch: expected {expected}, got {actual}")]
    EncodingMismatch { expected: String, actual: String },
    #[error("mutex poisoned")]
    MutexPoisoned,
    #[error("operation timed out")]
    Timeout,
}
