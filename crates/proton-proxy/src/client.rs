use lettre::{
    message::{header::ContentType, Attachment as LettreAttachment, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use tracing::{debug, info, instrument};

use crate::{Email, ProtonConfig, ProtonError};

/// Client for sending emails via Proton Mail Bridge.
///
/// Uses connection pooling for efficient batch sending.
pub struct ProtonClient {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from_address: String,
}

impl ProtonClient {
    /// Create a new client with the given configuration.
    ///
    /// Establishes a pooled SMTP connection to Proton Mail Bridge.
    pub fn new(config: ProtonConfig) -> Result<Self, ProtonError> {
        let creds = Credentials::new(config.username.clone(), config.password().to_string());

        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)
            .map_err(|e| ProtonError::Transport(e.to_string()))?
            .port(config.smtp_port)
            .credentials(creds)
            .build();

        info!(
            host = %config.smtp_host,
            port = config.smtp_port,
            username = %config.username,
            "Created Proton SMTP client"
        );

        Ok(Self {
            transport,
            from_address: config.username,
        })
    }

    /// Send an email.
    #[instrument(skip(self, email), fields(to = ?email.to, subject = %email.subject))]
    pub async fn send(&self, email: &Email) -> Result<(), ProtonError> {
        let message = self.build_message(email)?;

        self.transport
            .send(message)
            .await
            .map_err(|e| ProtonError::Send(e.to_string()))?;

        info!(to = ?email.to, subject = %email.subject, "Email sent successfully");
        Ok(())
    }

    /// Build a lettre Message from our Email type.
    fn build_message(&self, email: &Email) -> Result<Message, ProtonError> {
        let from = self
            .from_address
            .parse()
            .map_err(|e| ProtonError::InvalidAddress(format!("From: {}", e)))?;

        let mut builder = Message::builder().from(from).subject(&email.subject);

        // Add To recipients
        for to in &email.to {
            let addr = to
                .parse()
                .map_err(|e| ProtonError::InvalidAddress(format!("To '{}': {}", to, e)))?;
            builder = builder.to(addr);
        }

        // Add CC recipients
        for cc in &email.cc {
            let addr = cc
                .parse()
                .map_err(|e| ProtonError::InvalidAddress(format!("CC '{}': {}", cc, e)))?;
            builder = builder.cc(addr);
        }

        // Add BCC recipients
        for bcc in &email.bcc {
            let addr = bcc
                .parse()
                .map_err(|e| ProtonError::InvalidAddress(format!("BCC '{}': {}", bcc, e)))?;
            builder = builder.bcc(addr);
        }

        // Build body based on content type and attachments
        let message = if email.attachments.is_empty() {
            // No attachments
            if let Some(html) = &email.html_body {
                // Multipart alternative: text + HTML
                builder
                    .multipart(
                        MultiPart::alternative()
                            .singlepart(SinglePart::plain(email.body.clone()))
                            .singlepart(SinglePart::html(html.clone())),
                    )
                    .map_err(|e| ProtonError::BuildEmail(e.to_string()))?
            } else {
                // Plain text only
                builder
                    .body(email.body.clone())
                    .map_err(|e| ProtonError::BuildEmail(e.to_string()))?
            }
        } else {
            // Has attachments - use multipart mixed
            let body_part = if let Some(html) = &email.html_body {
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(email.body.clone()))
                    .singlepart(SinglePart::html(html.clone()))
            } else {
                MultiPart::alternative().singlepart(SinglePart::plain(email.body.clone()))
            };

            let mut multipart = MultiPart::mixed().multipart(body_part);

            for attachment in &email.attachments {
                debug!(filename = %attachment.filename, content_type = %attachment.content_type, "Adding attachment");

                let content_type: ContentType = attachment
                    .content_type
                    .parse()
                    .map_err(|e| ProtonError::Attachment(format!("Invalid content type: {}", e)))?;

                let lettre_attachment =
                    LettreAttachment::new(attachment.filename.clone()).body(attachment.data.clone(), content_type);

                multipart = multipart.singlepart(lettre_attachment);
            }

            builder
                .multipart(multipart)
                .map_err(|e| ProtonError::BuildEmail(e.to_string()))?
        };

        Ok(message)
    }
}
