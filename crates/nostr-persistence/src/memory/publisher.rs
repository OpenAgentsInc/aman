use std::env;
use std::time::Duration;

use async_trait::async_trait;
use nostr_sdk::prelude::*;
use sha2::{Digest, Sha256};
use tracing::info;

use crate::events::{d_tag, k_tag, unix_timestamp};
use crate::memory::crypto::encode_payload;
use crate::memory::{
    hk_tag, ts_tag, v_tag, AmanClearContextEvent, AmanPreferenceEvent, AmanSummaryEvent,
    AmanToolHistoryEvent, KIND_AMAN_CLEAR_CONTEXT, KIND_AMAN_PREFERENCE, KIND_AMAN_SUMMARY,
    KIND_AMAN_TOOL_HISTORY, MEMORY_SCHEMA_VERSION, TAG_KIND_AMAN_CLEAR_CONTEXT,
    TAG_KIND_AMAN_PREFERENCE, TAG_KIND_AMAN_SUMMARY, TAG_KIND_AMAN_TOOL_HISTORY,
};
use crate::{Error, PublishResult, SecretBoxCodec};

const DEFAULT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_MIN_ACKS: usize = 1;

#[derive(Clone)]
pub struct MemoryPublisherConfig {
    pub relays: Vec<String>,
    pub secret_key: String,
    pub min_acks: usize,
    pub timeout: Duration,
    pub secretbox_key: Option<SecretBoxCodec>,
}

impl MemoryPublisherConfig {
    pub fn from_env() -> Result<Option<Self>, Error> {
        let relays = match env::var("NOSTR_RELAYS") {
            Ok(value) => parse_relays(&value),
            Err(_) => Vec::new(),
        };
        if relays.is_empty() {
            return Ok(None);
        }

        let secret_key =
            env::var("NOSTR_SECRET_KEY").map_err(|_| Error::MissingEnv("NOSTR_SECRET_KEY"))?;
        let secretbox_key = match env::var("NOSTR_SECRETBOX_KEY") {
            Ok(value) => Some(SecretBoxCodec::from_str(&value)?),
            Err(_) => None,
        };

        Ok(Some(Self {
            relays,
            secret_key,
            min_acks: DEFAULT_MIN_ACKS,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            secretbox_key,
        }))
    }
}

#[async_trait]
pub trait NostrMemoryPublisher: Send + Sync {
    async fn publish_preference(
        &self,
        history_key: &str,
        preference: &str,
    ) -> Result<PublishResult, Error>;
    async fn publish_summary(
        &self,
        history_key: &str,
        summary: &str,
        message_count: i64,
    ) -> Result<PublishResult, Error>;
    async fn publish_tool_history(
        &self,
        entry: AmanToolHistoryEvent,
    ) -> Result<PublishResult, Error>;
    async fn publish_clear_context(
        &self,
        history_key: &str,
        sender_id: &str,
    ) -> Result<PublishResult, Error>;
}

#[derive(Clone)]
pub struct NostrMemoryPublisherImpl {
    client: Client,
    config: MemoryPublisherConfig,
}

impl NostrMemoryPublisherImpl {
    pub async fn new(config: MemoryPublisherConfig) -> Result<Self, Error> {
        let keys = Keys::parse(&config.secret_key)?;
        let client = Client::builder().signer(keys).build();

        for relay in &config.relays {
            client.add_relay(relay).await?;
        }

        client.connect().await;
        Ok(Self { client, config })
    }

    async fn publish_payload<T: serde::Serialize>(
        &self,
        kind: Kind,
        d_value: String,
        k_value: &str,
        history_key: &str,
        timestamp: u64,
        payload: &T,
    ) -> Result<PublishResult, Error> {
        let (content, enc_tag) = encode_payload(payload, self.config.secretbox_key.as_ref())?;

        let mut tags = Vec::new();
        tags.push(d_tag(&d_value).to_sdk_tag()?);
        tags.push(k_tag(k_value).to_sdk_tag()?);
        tags.push(hk_tag(history_key).to_sdk_tag()?);
        tags.push(v_tag(MEMORY_SCHEMA_VERSION).to_sdk_tag()?);
        tags.push(ts_tag(timestamp).to_sdk_tag()?);

        if let Some(tag) = enc_tag {
            tags.push(tag.to_sdk_tag()?);
        }

        let builder = EventBuilder::new(kind, content)
            .custom_created_at(Timestamp::from(timestamp))
            .tags(tags);
        let output = tokio::time::timeout(
            self.config.timeout,
            self.client.send_event_builder(builder),
        )
        .await
        .map_err(|_| Error::Timeout)??;

        let success = output.success.len();
        let failed = output.failed.len();
        if self.config.min_acks > 0 && success < self.config.min_acks {
            return Err(Error::Quorum {
                required: self.config.min_acks,
                actual: success,
            });
        }

        let event_id = output.id().to_string();
        info!(
            event_id = %event_id,
            kind = kind.as_u16(),
            success,
            failed,
            "Published nostr memory event"
        );

        Ok(PublishResult {
            event_id,
            success,
            failed,
        })
    }
}

#[async_trait]
impl NostrMemoryPublisher for NostrMemoryPublisherImpl {
    async fn publish_preference(
        &self,
        history_key: &str,
        preference: &str,
    ) -> Result<PublishResult, Error> {
        let updated_at = unix_timestamp();
        let event = AmanPreferenceEvent {
            history_key: history_key.to_string(),
            preference: preference.to_string(),
            updated_at,
        };
        let d_value = format!("{history_key}:preference");
        self.publish_payload(
            Kind::Custom(KIND_AMAN_PREFERENCE),
            d_value,
            TAG_KIND_AMAN_PREFERENCE,
            history_key,
            updated_at,
            &event,
        )
        .await
    }

    async fn publish_summary(
        &self,
        history_key: &str,
        summary: &str,
        message_count: i64,
    ) -> Result<PublishResult, Error> {
        let updated_at = unix_timestamp();
        let event = AmanSummaryEvent {
            history_key: history_key.to_string(),
            summary: summary.to_string(),
            message_count,
            updated_at,
        };
        let d_value = format!("{history_key}:summary");
        self.publish_payload(
            Kind::Custom(KIND_AMAN_SUMMARY),
            d_value,
            TAG_KIND_AMAN_SUMMARY,
            history_key,
            updated_at,
            &event,
        )
        .await
    }

    async fn publish_tool_history(
        &self,
        entry: AmanToolHistoryEvent,
    ) -> Result<PublishResult, Error> {
        let hash = hash_payload(&entry)?;
        let d_value = format!("{}:{}", entry.history_key, hash);
        self.publish_payload(
            Kind::Custom(KIND_AMAN_TOOL_HISTORY),
            d_value,
            TAG_KIND_AMAN_TOOL_HISTORY,
            &entry.history_key,
            entry.created_at,
            &entry,
        )
        .await
    }

    async fn publish_clear_context(
        &self,
        history_key: &str,
        sender_id: &str,
    ) -> Result<PublishResult, Error> {
        let created_at = unix_timestamp();
        let event = AmanClearContextEvent {
            history_key: history_key.to_string(),
            sender_id: Some(sender_id.to_string()),
            created_at,
        };
        let hash = hash_payload(&event)?;
        let d_value = format!("{history_key}:{hash}");
        self.publish_payload(
            Kind::Custom(KIND_AMAN_CLEAR_CONTEXT),
            d_value,
            TAG_KIND_AMAN_CLEAR_CONTEXT,
            history_key,
            created_at,
            &event,
        )
        .await
    }
}

fn hash_payload<T: serde::Serialize>(payload: &T) -> Result<String, Error> {
    let json = serde_json::to_vec(payload)?;
    let mut hasher = Sha256::new();
    hasher.update(json);
    Ok(hex::encode(hasher.finalize()))
}

fn parse_relays(value: &str) -> Vec<String> {
    value
        .split(',')
        .flat_map(|chunk| chunk.split_whitespace())
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(String::from)
        .collect()
}
