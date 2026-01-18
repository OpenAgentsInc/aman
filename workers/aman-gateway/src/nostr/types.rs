use serde::{Deserialize, Serialize};

pub const KIND_DOC_MANIFEST: u16 = 30090;
pub const KIND_CHUNK_REF: u16 = 30091;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NostrEvent {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u16,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

impl NostrEvent {
    pub fn tag_value(&self, name: &str) -> Option<&str> {
        tag_value(&self.tags, name)
    }
}

#[derive(Debug, Clone)]
pub struct NostrRawEvent {
    pub event: NostrEvent,
    pub raw_json: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NostrFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}

pub fn tag_value<'a>(tags: &'a [Vec<String>], name: &str) -> Option<&'a str> {
    tags.iter()
        .find(|tag| tag.first().map(|value| value == name).unwrap_or(false))
        .and_then(|tag| tag.get(1).map(|value| value.as_str()))
}
