use async_trait::async_trait;
use base64::Engine;
use nostr_sdk::prelude::*;
use tracing::info;

use crate::config::PublisherConfig;
use crate::crypto::{codec_tag, PayloadCodec};
use crate::events::{
    d_tag, k_tag, AccessPolicy, ChunkRef, DocManifest, NostrTag, TAG_KIND_CHUNK_REF,
    TAG_KIND_DOC_MANIFEST, TAG_KIND_POLICY,
};
use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishResult {
    pub event_id: String,
    pub success: usize,
    pub failed: usize,
}

#[async_trait]
pub trait NostrPublisher: Send + Sync {
    async fn publish_doc_manifest(
        &self,
        doc: DocManifest,
        extra_tags: Vec<NostrTag>,
    ) -> Result<PublishResult, Error>;
    async fn publish_chunk_ref(
        &self,
        chunk: ChunkRef,
        extra_tags: Vec<NostrTag>,
    ) -> Result<PublishResult, Error>;
    async fn publish_policy(
        &self,
        policy: AccessPolicy,
        extra_tags: Vec<NostrTag>,
    ) -> Result<PublishResult, Error>;
}

#[derive(Clone)]
pub struct NostrPublisherImpl {
    client: Client,
    config: PublisherConfig,
}

impl NostrPublisherImpl {
    pub async fn new(config: PublisherConfig) -> Result<Self, Error> {
        let keys = config.keys()?;
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
        d_value: &str,
        k_value: &str,
        updated_at: u64,
        payload: &T,
        extra_tags: Vec<NostrTag>,
    ) -> Result<PublishResult, Error> {
        let json = serde_json::to_vec(payload)?;
        let codec = self.config.codec.as_ref();
        let (content, enc_tag) = encode_content(codec, &json)?;

        let mut tags = Vec::new();
        tags.push(d_tag(d_value).to_sdk_tag()?);
        tags.push(k_tag(k_value).to_sdk_tag()?);

        if let Some(tag) = enc_tag {
            tags.push(tag.to_sdk_tag()?);
        }

        for tag in extra_tags {
            tags.push(tag.to_sdk_tag()?);
        }

        let builder = EventBuilder::new(kind, content)
            .custom_created_at(Timestamp::from(updated_at))
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
        info!(event_id = %event_id, success, failed, "Published nostr event");

        Ok(PublishResult {
            event_id,
            success,
            failed,
        })
    }
}

#[async_trait]
impl NostrPublisher for NostrPublisherImpl {
    async fn publish_doc_manifest(
        &self,
        doc: DocManifest,
        extra_tags: Vec<NostrTag>,
    ) -> Result<PublishResult, Error> {
        let kind = self.config.kinds.doc_manifest;
        self.publish_payload(
            kind,
            &doc.doc_id,
            TAG_KIND_DOC_MANIFEST,
            doc.updated_at,
            &doc,
            extra_tags,
        )
            .await
    }

    async fn publish_chunk_ref(
        &self,
        chunk: ChunkRef,
        extra_tags: Vec<NostrTag>,
    ) -> Result<PublishResult, Error> {
        let kind = self.config.kinds.chunk_ref;
        self.publish_payload(
            kind,
            &chunk.chunk_id,
            TAG_KIND_CHUNK_REF,
            chunk.updated_at,
            &chunk,
            extra_tags,
        )
            .await
    }

    async fn publish_policy(
        &self,
        policy: AccessPolicy,
        extra_tags: Vec<NostrTag>,
    ) -> Result<PublishResult, Error> {
        let kind = self.config.kinds.access_policy;
        self.publish_payload(
            kind,
            &policy.scope_id,
            TAG_KIND_POLICY,
            policy.updated_at,
            &policy,
            extra_tags,
        )
            .await
    }
}

fn encode_content(
    codec: &dyn PayloadCodec,
    json: &[u8],
) -> Result<(String, Option<NostrTag>), Error> {
    if let Some(tag) = codec_tag(codec) {
        let encoded = codec.encode(json)?;
        let content = base64::engine::general_purpose::STANDARD.encode(encoded);
        Ok((content, Some(tag)))
    } else {
        Ok((String::from_utf8(json.to_vec())?, None))
    }
}
