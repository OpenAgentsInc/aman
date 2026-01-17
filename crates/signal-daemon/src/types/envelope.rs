//! Envelope and message types from signal-cli daemon.

use serde::{Deserialize, Serialize};

/// A message envelope received from Signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    /// Source phone number (e.g., "+1234567890").
    #[serde(default)]
    pub source: String,

    /// Source phone number (same as source).
    #[serde(default)]
    pub source_number: String,

    /// Source UUID.
    #[serde(default)]
    pub source_uuid: Option<String>,

    /// Contact name if available.
    #[serde(default)]
    pub source_name: Option<String>,

    /// Source device ID.
    #[serde(default)]
    pub source_device: Option<u32>,

    /// Message timestamp (milliseconds since epoch).
    #[serde(default)]
    pub timestamp: u64,

    /// Data message content (regular message).
    #[serde(default)]
    pub data_message: Option<DataMessage>,

    /// Sync message (from linked device).
    #[serde(default)]
    pub sync_message: Option<SyncMessage>,

    /// Receipt message.
    #[serde(default)]
    pub receipt_message: Option<ReceiptMessage>,

    /// Typing indicator.
    #[serde(default)]
    pub typing_message: Option<TypingMessage>,
}

/// A data message containing the actual message content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataMessage {
    /// Message timestamp.
    #[serde(default)]
    pub timestamp: u64,

    /// The text message content.
    #[serde(default)]
    pub message: Option<String>,

    /// Expiration time in seconds (disappearing messages).
    #[serde(default)]
    pub expires_in_seconds: u32,

    /// Whether this is a view-once message.
    #[serde(default)]
    pub view_once: bool,

    /// Group information if this is a group message.
    #[serde(default)]
    pub group_info: Option<GroupInfo>,

    /// Attachments included with the message.
    #[serde(default)]
    pub attachments: Vec<Attachment>,

    /// Quote/reply to another message.
    #[serde(default)]
    pub quote: Option<Quote>,

    /// Reaction to a message.
    #[serde(default)]
    pub reaction: Option<Reaction>,

    /// Mentions in the message.
    #[serde(default)]
    pub mentions: Vec<Mention>,
}

/// Information about a group.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfo {
    /// Group ID (base64 encoded).
    #[serde(default)]
    pub group_id: String,

    /// Group type (v1 or v2).
    #[serde(default)]
    pub r#type: Option<String>,
}

/// An attachment in a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    /// Content type (MIME type).
    #[serde(default)]
    pub content_type: String,

    /// Original filename.
    #[serde(default)]
    pub filename: Option<String>,

    /// Attachment ID.
    #[serde(default)]
    pub id: Option<String>,

    /// Size in bytes.
    #[serde(default)]
    pub size: Option<u64>,

    /// Width (for images/videos).
    #[serde(default)]
    pub width: Option<u32>,

    /// Height (for images/videos).
    #[serde(default)]
    pub height: Option<u32>,

    /// Caption text.
    #[serde(default)]
    pub caption: Option<String>,
}

/// Quote/reply to another message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    /// Original message ID.
    #[serde(default)]
    pub id: u64,

    /// Author of the quoted message.
    #[serde(default)]
    pub author: Option<String>,

    /// Author UUID.
    #[serde(default)]
    pub author_uuid: Option<String>,

    /// Quoted text.
    #[serde(default)]
    pub text: Option<String>,
}

/// Reaction to a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reaction {
    /// The emoji reaction.
    #[serde(default)]
    pub emoji: String,

    /// Whether this removes a reaction.
    #[serde(default)]
    pub is_remove: bool,

    /// Target message author.
    #[serde(default)]
    pub target_author: Option<String>,

    /// Target message author UUID.
    #[serde(default)]
    pub target_author_uuid: Option<String>,

    /// Target message timestamp.
    #[serde(default)]
    pub target_sent_timestamp: u64,
}

/// A mention in a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mention {
    /// Start position in the message.
    #[serde(default)]
    pub start: u32,

    /// Length of the mention.
    #[serde(default)]
    pub length: u32,

    /// UUID of the mentioned user.
    #[serde(default)]
    pub uuid: Option<String>,

    /// Phone number of the mentioned user.
    #[serde(default)]
    pub number: Option<String>,
}

/// Sync message from a linked device.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMessage {
    /// Sent message sync.
    #[serde(default)]
    pub sent_message: Option<SentMessage>,

    /// Read receipts sync.
    #[serde(default)]
    pub read_messages: Vec<ReadMessage>,
}

/// A message sent from another linked device.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SentMessage {
    /// Destination phone number.
    #[serde(default)]
    pub destination: Option<String>,

    /// Destination UUID.
    #[serde(default)]
    pub destination_uuid: Option<String>,

    /// The message content.
    #[serde(default)]
    pub message: Option<DataMessage>,

    /// Timestamp.
    #[serde(default)]
    pub timestamp: u64,
}

/// A read message receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadMessage {
    /// Sender of the read message.
    #[serde(default)]
    pub sender: Option<String>,

    /// Sender UUID.
    #[serde(default)]
    pub sender_uuid: Option<String>,

    /// Message timestamp.
    #[serde(default)]
    pub timestamp: u64,
}

/// A receipt message (delivery, read, viewed).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptMessage {
    /// Timestamps of messages this receipt is for.
    #[serde(default)]
    pub timestamps: Vec<u64>,

    /// When the receipt was created.
    #[serde(default)]
    pub when: u64,

    /// Whether this is a delivery receipt.
    #[serde(default)]
    pub is_delivery: bool,

    /// Whether this is a read receipt.
    #[serde(default)]
    pub is_read: bool,

    /// Whether this is a viewed receipt.
    #[serde(default)]
    pub is_viewed: bool,
}

/// A typing indicator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingMessage {
    /// Action: "STARTED" or "STOPPED".
    #[serde(default)]
    pub action: String,

    /// Timestamp.
    #[serde(default)]
    pub timestamp: u64,

    /// Group ID if in a group.
    #[serde(default)]
    pub group_id: Option<String>,
}

/// Wrapper for SSE event data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiveEvent {
    /// The message envelope.
    pub envelope: Envelope,
}
