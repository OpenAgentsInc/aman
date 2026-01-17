//! Integration with signal-daemon types.
//!
//! This module provides conversion utilities between signal-daemon's
//! Envelope types and brain-core's message types.
//!
//! Enable with the `signal-daemon` feature:
//! ```toml
//! mock-brain = { path = "../mock-brain", features = ["signal-daemon"] }
//! ```

use brain_core::{Brain, BrainError, InboundAttachment, InboundMessage, OutboundMessage};
use signal_daemon::{Attachment, Envelope, SendResult};

/// Extension trait for converting signal-daemon Envelope to InboundMessage.
pub trait EnvelopeExt {
    /// Convert to an InboundMessage if this envelope contains a text message.
    ///
    /// Returns `None` if:
    /// - No data_message is present
    /// - The data_message has no text content (unless it has attachments)
    fn to_inbound_message(&self) -> Option<InboundMessage>;
}

/// Convert a signal-daemon Attachment to an InboundAttachment.
fn convert_attachment(att: &Attachment) -> InboundAttachment {
    InboundAttachment {
        content_type: att.content_type.clone(),
        filename: att.filename.clone(),
        file_path: att.id.clone(), // signal-cli uses 'id' as the file path
        size: att.size,
        width: att.width,
        height: att.height,
        caption: att.caption.clone(),
    }
}

impl EnvelopeExt for Envelope {
    fn to_inbound_message(&self) -> Option<InboundMessage> {
        let data_message = self.data_message.as_ref()?;

        // Convert attachments
        let attachments: Vec<InboundAttachment> = data_message
            .attachments
            .iter()
            .map(convert_attachment)
            .collect();

        // Get text content - allow empty string if there are attachments
        let text = data_message
            .message
            .clone()
            .or_else(|| {
                if !attachments.is_empty() {
                    Some(String::new())
                } else {
                    None
                }
            })?;

        let group_id = data_message
            .group_info
            .as_ref()
            .map(|g| g.group_id.clone());

        Some(InboundMessage {
            sender: self.source.clone(),
            text,
            timestamp: self.timestamp,
            group_id,
            attachments,
        })
    }
}

/// Extension trait for OutboundMessage to prepare for signal-daemon sending.
pub trait OutboundMessageExt {
    /// Get the recipient phone number (for direct messages).
    fn recipient_number(&self) -> Option<&str>;

    /// Get the group ID (for group messages).
    fn group_id(&self) -> Option<&str>;
}

impl OutboundMessageExt for OutboundMessage {
    fn recipient_number(&self) -> Option<&str> {
        if self.is_group {
            None
        } else {
            Some(&self.recipient)
        }
    }

    fn group_id(&self) -> Option<&str> {
        if self.is_group {
            Some(&self.recipient)
        } else {
            None
        }
    }
}

/// Send an outbound message using a signal-daemon client.
///
/// This is a convenience function that handles both direct and group messages.
pub async fn send_response(
    client: &signal_daemon::SignalClient,
    response: &OutboundMessage,
) -> Result<SendResult, signal_daemon::DaemonError> {
    if response.is_group {
        client.send_to_group(&response.recipient, &response.text).await
    } else {
        client.send_text(&response.recipient, &response.text).await
    }
}

/// Process an envelope using a brain and send the response.
///
/// Returns `Ok(Some(result))` if a message was processed and sent,
/// `Ok(None)` if the envelope didn't contain a processable message,
/// or `Err` if processing or sending failed.
pub async fn process_and_respond<B: Brain>(
    client: &signal_daemon::SignalClient,
    brain: &B,
    envelope: &Envelope,
) -> Result<Option<SendResult>, ProcessError> {
    let inbound = match envelope.to_inbound_message() {
        Some(msg) => msg,
        None => return Ok(None),
    };

    let response = brain.process(inbound).await.map_err(ProcessError::Brain)?;
    let result = send_response(client, &response)
        .await
        .map_err(ProcessError::Send)?;

    Ok(Some(result))
}

/// Errors that can occur during process_and_respond.
#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    /// Error from the brain during processing.
    #[error("brain error: {0}")]
    Brain(#[from] BrainError),

    /// Error from signal-daemon during sending.
    #[error("send error: {0}")]
    Send(#[from] signal_daemon::DaemonError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use signal_daemon::{Attachment, DataMessage, GroupInfo};

    #[test]
    fn test_envelope_to_inbound_direct() {
        let envelope = Envelope {
            source: "+15551234567".to_string(),
            source_number: "+15551234567".to_string(),
            timestamp: 1234567890,
            data_message: Some(DataMessage {
                message: Some("Hello!".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let inbound = envelope.to_inbound_message().unwrap();
        assert_eq!(inbound.sender, "+15551234567");
        assert_eq!(inbound.text, "Hello!");
        assert_eq!(inbound.timestamp, 1234567890);
        assert!(inbound.group_id.is_none());
        assert!(inbound.attachments.is_empty());
    }

    #[test]
    fn test_envelope_with_attachment() {
        let envelope = Envelope {
            source: "+15551234567".to_string(),
            source_number: "+15551234567".to_string(),
            timestamp: 1234567890,
            data_message: Some(DataMessage {
                message: Some("Check this out!".to_string()),
                attachments: vec![Attachment {
                    content_type: "image/jpeg".to_string(),
                    filename: Some("photo.jpg".to_string()),
                    id: Some("/tmp/signal-cli/attachments/123456".to_string()),
                    size: Some(12345),
                    width: Some(800),
                    height: Some(600),
                    caption: Some("A nice photo".to_string()),
                }],
                ..Default::default()
            }),
            ..Default::default()
        };

        let inbound = envelope.to_inbound_message().unwrap();
        assert_eq!(inbound.text, "Check this out!");
        assert_eq!(inbound.attachments.len(), 1);

        let att = &inbound.attachments[0];
        assert_eq!(att.content_type, "image/jpeg");
        assert_eq!(att.filename, Some("photo.jpg".to_string()));
        assert_eq!(att.file_path, Some("/tmp/signal-cli/attachments/123456".to_string()));
        assert_eq!(att.size, Some(12345));
        assert_eq!(att.width, Some(800));
        assert_eq!(att.height, Some(600));
        assert!(att.is_image());
        assert!(!att.is_video());
    }

    #[test]
    fn test_envelope_attachment_only() {
        // Message with attachment but no text
        let envelope = Envelope {
            source: "+15551234567".to_string(),
            source_number: "+15551234567".to_string(),
            timestamp: 1234567890,
            data_message: Some(DataMessage {
                message: None,
                attachments: vec![Attachment {
                    content_type: "image/png".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };

        let inbound = envelope.to_inbound_message().unwrap();
        assert_eq!(inbound.text, ""); // Empty text
        assert_eq!(inbound.attachments.len(), 1);
        assert!(inbound.has_attachments());
        assert!(inbound.has_images());
    }

    #[test]
    fn test_envelope_to_inbound_group() {
        let envelope = Envelope {
            source: "+15551234567".to_string(),
            source_number: "+15551234567".to_string(),
            timestamp: 1234567890,
            data_message: Some(DataMessage {
                message: Some("Hello group!".to_string()),
                group_info: Some(GroupInfo {
                    group_id: "group123".to_string(),
                    r#type: None,
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let inbound = envelope.to_inbound_message().unwrap();
        assert_eq!(inbound.sender, "+15551234567");
        assert_eq!(inbound.text, "Hello group!");
        assert_eq!(inbound.group_id, Some("group123".to_string()));
    }

    #[test]
    fn test_envelope_no_message() {
        let envelope = Envelope {
            source: "+15551234567".to_string(),
            ..Default::default()
        };

        assert!(envelope.to_inbound_message().is_none());
    }

    #[test]
    fn test_outbound_message_ext() {
        let direct = OutboundMessage::direct("+15559876543", "Hello");
        assert_eq!(direct.recipient_number(), Some("+15559876543"));
        assert!(direct.group_id().is_none());

        let group = OutboundMessage {
            recipient: "group123".to_string(),
            text: "Hello group".to_string(),
            is_group: true,
        };
        assert!(group.recipient_number().is_none());
        assert_eq!(group.group_id(), Some("group123"));
    }
}
