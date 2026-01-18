use std::path::Path;

use crate::ProtonError;

/// An email message to send.
#[derive(Debug, Clone)]
pub struct Email {
    /// Primary recipients
    pub to: Vec<String>,
    /// CC recipients
    pub cc: Vec<String>,
    /// BCC recipients
    pub bcc: Vec<String>,
    /// Email subject
    pub subject: String,
    /// Plain text body
    pub body: String,
    /// Optional HTML body
    pub html_body: Option<String>,
    /// File attachments
    pub attachments: Vec<Attachment>,
}

impl Email {
    /// Create a new email with a single recipient.
    pub fn new(to: impl Into<String>, subject: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            to: vec![to.into()],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: subject.into(),
            body: body.into(),
            html_body: None,
            attachments: Vec::new(),
        }
    }

    /// Create a new email with multiple recipients.
    pub fn new_multi(
        to: impl IntoIterator<Item = impl Into<String>>,
        subject: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            to: to.into_iter().map(Into::into).collect(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: subject.into(),
            body: body.into(),
            html_body: None,
            attachments: Vec::new(),
        }
    }

    /// Add a recipient to the To field.
    pub fn add_to(&mut self, recipient: impl Into<String>) -> &mut Self {
        self.to.push(recipient.into());
        self
    }

    /// Add a CC recipient.
    pub fn add_cc(&mut self, recipient: impl Into<String>) -> &mut Self {
        self.cc.push(recipient.into());
        self
    }

    /// Add a BCC recipient.
    pub fn add_bcc(&mut self, recipient: impl Into<String>) -> &mut Self {
        self.bcc.push(recipient.into());
        self
    }

    /// Set the HTML body (creates multipart alternative with text fallback).
    pub fn with_html(&mut self, html: impl Into<String>) -> &mut Self {
        self.html_body = Some(html.into());
        self
    }

    /// Add an attachment.
    pub fn attach(&mut self, attachment: Attachment) -> &mut Self {
        self.attachments.push(attachment);
        self
    }
}

/// A file attachment for an email.
#[derive(Debug, Clone)]
pub struct Attachment {
    /// Filename to display
    pub filename: String,
    /// MIME content type (e.g., "application/pdf")
    pub content_type: String,
    /// Raw file data
    pub data: Vec<u8>,
}

impl Attachment {
    /// Create an attachment from raw data.
    pub fn new(filename: impl Into<String>, content_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            filename: filename.into(),
            content_type: content_type.into(),
            data,
        }
    }

    /// Create an attachment from a file path.
    ///
    /// MIME type is auto-detected from the file extension.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ProtonError> {
        let path = path.as_ref();
        let data = std::fs::read(path)?;

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| ProtonError::Attachment("Invalid filename".to_string()))?
            .to_string();

        let content_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();

        Ok(Self::new(filename, content_type, data))
    }

    /// Create an attachment from bytes with auto-detected MIME type.
    pub fn from_bytes(filename: impl Into<String>, data: Vec<u8>) -> Self {
        let filename = filename.into();
        let content_type = mime_guess::from_path(&filename)
            .first_or_octet_stream()
            .to_string();

        Self::new(filename, content_type, data)
    }
}

/// A received email message from the inbox.
#[derive(Debug, Clone)]
pub struct InboxMessage {
    /// Unique identifier (UID) of the message
    pub uid: u32,
    /// Message ID header
    pub message_id: Option<String>,
    /// Sender email address
    pub from: Option<String>,
    /// Sender display name
    pub from_name: Option<String>,
    /// Recipient email addresses
    pub to: Vec<String>,
    /// CC recipients
    pub cc: Vec<String>,
    /// Email subject
    pub subject: String,
    /// Plain text body
    pub body: Option<String>,
    /// HTML body
    pub html_body: Option<String>,
    /// Date received (RFC 2822 format)
    pub date: Option<String>,
    /// Attachments
    pub attachments: Vec<InboxAttachment>,
    /// Raw message data
    pub raw: Option<Vec<u8>>,
}

impl InboxMessage {
    /// Create a new inbox message with minimal fields.
    pub fn new(uid: u32, subject: impl Into<String>) -> Self {
        Self {
            uid,
            message_id: None,
            from: None,
            from_name: None,
            to: Vec::new(),
            cc: Vec::new(),
            subject: subject.into(),
            body: None,
            html_body: None,
            date: None,
            attachments: Vec::new(),
            raw: None,
        }
    }
}

/// An attachment from a received email.
#[derive(Debug, Clone)]
pub struct InboxAttachment {
    /// Filename
    pub filename: String,
    /// MIME content type
    pub content_type: String,
    /// Attachment data
    pub data: Vec<u8>,
}
