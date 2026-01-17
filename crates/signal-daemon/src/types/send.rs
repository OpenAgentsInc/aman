//! Types for sending messages via signal-cli daemon.

use serde::{Deserialize, Serialize};

/// Parameters for sending a message.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendParams {
    /// Recipients (phone numbers).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recipient: Vec<String>,

    /// Group IDs to send to.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub group_id: Vec<String>,

    /// The message text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// File paths to attachments.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<String>,

    /// Account to send from (multi-account mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,

    /// Quote a previous message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_timestamp: Option<u64>,

    /// Author of the quoted message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_author: Option<String>,

    /// Mentions in the message.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<MentionParam>,

    /// Text style formatting.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub text_style: Vec<TextStyleParam>,
}

impl SendParams {
    /// Create new send params for a text message to a recipient.
    pub fn text(recipient: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            recipient: vec![recipient.into()],
            message: Some(message.into()),
            ..Default::default()
        }
    }

    /// Create new send params for a text message to a group.
    pub fn group(group_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            group_id: vec![group_id.into()],
            message: Some(message.into()),
            ..Default::default()
        }
    }

    /// Add an attachment file path.
    pub fn with_attachment(mut self, path: impl Into<String>) -> Self {
        self.attachments.push(path.into());
        self
    }

    /// Set the account for multi-account mode.
    pub fn with_account(mut self, account: impl Into<String>) -> Self {
        self.account = Some(account.into());
        self
    }

    /// Add a quote/reply to another message.
    pub fn with_quote(mut self, timestamp: u64, author: impl Into<String>) -> Self {
        self.quote_timestamp = Some(timestamp);
        self.quote_author = Some(author.into());
        self
    }
}

/// A mention parameter for sending.
#[derive(Debug, Clone, Serialize)]
pub struct MentionParam {
    /// Start position in the message.
    pub start: u32,
    /// Length of the mention.
    pub length: u32,
    /// UUID of the mentioned user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Phone number of the mentioned user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
}

/// Text style parameter.
#[derive(Debug, Clone, Serialize)]
pub struct TextStyleParam {
    /// Start position.
    pub start: u32,
    /// Length.
    pub length: u32,
    /// Style type (BOLD, ITALIC, etc.).
    pub style: String,
}

/// Result of sending a message.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendResult {
    /// Timestamp of the sent message.
    pub timestamp: u64,

    /// Results per recipient (if available).
    #[serde(default)]
    pub results: Vec<RecipientResult>,
}

/// Result for a specific recipient.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipientResult {
    /// Recipient phone number.
    #[serde(default)]
    pub recipient_address: Option<RecipientAddress>,

    /// Whether the message was sent successfully.
    #[serde(default)]
    pub success: bool,

    /// Error message if failed.
    #[serde(default)]
    pub error: Option<String>,
}

/// Recipient address information.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipientAddress {
    /// UUID.
    #[serde(default)]
    pub uuid: Option<String>,

    /// Phone number.
    #[serde(default)]
    pub number: Option<String>,
}
