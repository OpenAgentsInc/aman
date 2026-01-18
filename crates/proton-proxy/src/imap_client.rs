use async_imap::Session;
use async_native_tls::TlsStream;
use async_std::net::TcpStream;
use futures::TryStreamExt;
use mail_parser::{MessageParser, MimeHeaders};
use tracing::{debug, info, instrument};

use crate::types::InboxAttachment;
use crate::{InboxMessage, ProtonConfig, ProtonError};

type ImapSession = Session<TlsStream<TcpStream>>;

/// Low-level IMAP client for Proton Mail Bridge.
pub struct ImapClient {
    session: ImapSession,
}

impl ImapClient {
    /// Connect and authenticate to Proton Mail Bridge IMAP server.
    #[instrument(skip(config), fields(host = %config.imap_host, port = config.imap_port))]
    pub async fn connect(config: &ProtonConfig) -> Result<Self, ProtonError> {
        let addr = format!("{}:{}", config.imap_host, config.imap_port);
        debug!("Connecting to IMAP server at {}", addr);

        // Step 1: Connect plain TCP
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| ProtonError::ImapConnection(format!("Failed to connect: {}", e)))?;

        // Step 2: Create IMAP client on plain connection
        let mut client = async_imap::Client::new(stream);

        // Step 3: Read server greeting (returns Option<Result<ResponseData, io::Error>>)
        let greeting_opt = client.read_response().await;
        let _greeting = greeting_opt
            .ok_or_else(|| ProtonError::Imap("No greeting from server".to_string()))?
            .map_err(|e| ProtonError::Imap(format!("IO error reading greeting: {}", e)))?;

        debug!("Received server greeting, initiating STARTTLS");

        // Step 4: Send STARTTLS command
        client
            .run_command_and_check_ok("STARTTLS", None)
            .await
            .map_err(|e| ProtonError::Tls(format!("STARTTLS command failed: {}", e)))?;

        // Step 5: Extract the raw stream
        let stream = client.into_inner();

        // Step 6: Upgrade to TLS (accepts self-signed certs from Proton Bridge)
        let tls = async_native_tls::TlsConnector::new()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true);

        let tls_stream = tls
            .connect(&config.imap_host, stream)
            .await
            .map_err(|e| ProtonError::Tls(format!("TLS upgrade failed: {}", e)))?;

        // Step 7: Create new IMAP client on TLS stream (no greeting after STARTTLS)
        let client = async_imap::Client::new(tls_stream);

        // Step 8: Authenticate
        let session = client
            .login(&config.username, config.password())
            .await
            .map_err(|(e, _)| ProtonError::ImapAuth(format!("Login failed: {}", e)))?;

        info!("Connected to IMAP server via STARTTLS");
        Ok(Self { session })
    }

    /// List available folders/mailboxes.
    #[instrument(skip(self))]
    pub async fn list_folders(&mut self) -> Result<Vec<String>, ProtonError> {
        let folders: Vec<_> = self
            .session
            .list(Some(""), Some("*"))
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to list folders: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to collect folders: {}", e)))?;

        let names: Vec<String> = folders.iter().map(|f| f.name().to_string()).collect();
        debug!("Found {} folders", names.len());
        Ok(names)
    }

    /// Select a folder and return message count.
    #[instrument(skip(self))]
    pub async fn select_folder(&mut self, folder: &str) -> Result<u32, ProtonError> {
        let mailbox = self
            .session
            .select(folder)
            .await
            .map_err(|e| {
                ProtonError::Imap(format!("Failed to select folder '{}': {}", folder, e))
            })?;

        let count = mailbox.exists;
        debug!("Selected folder '{}' with {} messages", folder, count);
        Ok(count)
    }

    /// Get UIDs of all messages in the current folder.
    #[instrument(skip(self))]
    pub async fn fetch_uids(&mut self) -> Result<Vec<u32>, ProtonError> {
        let messages: Vec<_> = self
            .session
            .fetch("1:*", "UID")
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to fetch UIDs: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to collect UIDs: {}", e)))?;

        let uids: Vec<u32> = messages.iter().filter_map(|m| m.uid).collect();
        debug!("Found {} message UIDs", uids.len());
        Ok(uids)
    }

    /// Fetch a message by UID.
    #[instrument(skip(self))]
    pub async fn fetch_message(&mut self, uid: u32) -> Result<InboxMessage, ProtonError> {
        let messages: Vec<_> = self
            .session
            .uid_fetch(uid.to_string(), "(UID BODY[] ENVELOPE)")
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to fetch message {}: {}", uid, e)))?
            .try_collect()
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to collect message: {}", e)))?;

        let fetch = messages
            .first()
            .ok_or_else(|| ProtonError::Imap(format!("Message {} not found", uid)))?;

        let body = fetch
            .body()
            .ok_or_else(|| ProtonError::Imap("Message has no body".to_string()))?;

        parse_message(uid, body)
    }

    /// Fetch messages by UID range.
    #[instrument(skip(self))]
    pub async fn fetch_messages(&mut self, uids: &[u32]) -> Result<Vec<InboxMessage>, ProtonError> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }

        let uid_list = uids
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let messages: Vec<_> = self
            .session
            .uid_fetch(&uid_list, "(UID BODY[] ENVELOPE)")
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to fetch messages: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to collect messages: {}", e)))?;

        let mut result = Vec::new();
        for fetch in messages.iter() {
            if let (Some(uid), Some(body)) = (fetch.uid, fetch.body()) {
                match parse_message(uid, body) {
                    Ok(msg) => result.push(msg),
                    Err(e) => debug!("Failed to parse message {}: {}", uid, e),
                }
            }
        }

        debug!("Fetched {} messages", result.len());
        Ok(result)
    }

    /// Move a message to another folder.
    #[instrument(skip(self))]
    pub async fn move_message(&mut self, uid: u32, dest_folder: &str) -> Result<(), ProtonError> {
        // Copy to destination
        self.session
            .uid_copy(uid.to_string(), dest_folder)
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to copy message: {}", e)))?;

        // Mark as deleted in source
        let _: Vec<_> = self
            .session
            .uid_store(uid.to_string(), "+FLAGS (\\Deleted)")
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to mark deleted: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to collect store response: {}", e)))?;

        // Expunge deleted messages
        let _: Vec<_> = self
            .session
            .expunge()
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to expunge: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to collect expunge response: {}", e)))?;

        info!("Moved message {} to {}", uid, dest_folder);
        Ok(())
    }

    /// Delete a message (move to Trash).
    #[instrument(skip(self))]
    pub async fn delete_message(&mut self, uid: u32) -> Result<(), ProtonError> {
        self.move_message(uid, "Trash").await
    }

    /// Mark a message as read.
    #[instrument(skip(self))]
    pub async fn mark_read(&mut self, uid: u32) -> Result<(), ProtonError> {
        let _: Vec<_> = self
            .session
            .uid_store(uid.to_string(), "+FLAGS (\\Seen)")
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to mark read: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to collect response: {}", e)))?;
        Ok(())
    }

    /// Mark a message as unread.
    #[instrument(skip(self))]
    pub async fn mark_unread(&mut self, uid: u32) -> Result<(), ProtonError> {
        let _: Vec<_> = self
            .session
            .uid_store(uid.to_string(), "-FLAGS (\\Seen)")
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to mark unread: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| ProtonError::Imap(format!("Failed to collect response: {}", e)))?;
        Ok(())
    }

    /// Search for messages matching criteria.
    /// Returns UIDs of matching messages.
    #[instrument(skip(self))]
    pub async fn search(&mut self, query: &str) -> Result<Vec<u32>, ProtonError> {
        let uids = self
            .session
            .uid_search(query)
            .await
            .map_err(|e| ProtonError::Imap(format!("Search failed: {}", e)))?;

        let result: Vec<u32> = uids.into_iter().collect();
        debug!("Search found {} messages", result.len());
        Ok(result)
    }

    /// Search for unread messages.
    pub async fn search_unread(&mut self) -> Result<Vec<u32>, ProtonError> {
        self.search("UNSEEN").await
    }

    /// Search for messages from a specific sender.
    pub async fn search_from(&mut self, sender: &str) -> Result<Vec<u32>, ProtonError> {
        self.search(&format!("FROM \"{}\"", sender)).await
    }

    /// Logout and close connection.
    pub async fn logout(mut self) -> Result<(), ProtonError> {
        self.session
            .logout()
            .await
            .map_err(|e| ProtonError::Imap(format!("Logout failed: {}", e)))?;
        Ok(())
    }
}

/// Parse raw email bytes into InboxMessage.
fn parse_message(uid: u32, raw: &[u8]) -> Result<InboxMessage, ProtonError> {
    let parser = MessageParser::default();
    let parsed = parser
        .parse(raw)
        .ok_or_else(|| ProtonError::ParseMessage("Failed to parse message".to_string()))?;

    let subject = parsed.subject().unwrap_or("(no subject)").to_string();

    let from = parsed.from().and_then(|f| f.first()).map(|a| {
        a.address()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "(unknown)".to_string())
    });

    let from_name = parsed
        .from()
        .and_then(|f| f.first())
        .and_then(|a| a.name())
        .map(|s| s.to_string());

    let to: Vec<String> = parsed
        .to()
        .map(|addrs| {
            addrs
                .iter()
                .filter_map(|a| a.address().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let cc: Vec<String> = parsed
        .cc()
        .map(|addrs| {
            addrs
                .iter()
                .filter_map(|a| a.address().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let body = parsed.body_text(0).map(|s| s.to_string());
    let html_body = parsed.body_html(0).map(|s| s.to_string());
    let date = parsed.date().map(|d| d.to_rfc822());
    let message_id = parsed.message_id().map(|s| s.to_string());

    // Parse attachments
    let mut attachments = Vec::new();
    for part in parsed.attachments() {
        let filename = part.attachment_name().unwrap_or("attachment").to_string();
        let content_type = part
            .content_type()
            .map(|ct| ct.c_type.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let data = part.contents().to_vec();

        attachments.push(InboxAttachment {
            filename,
            content_type,
            data,
        });
    }

    Ok(InboxMessage {
        uid,
        message_id,
        from,
        from_name,
        to,
        cc,
        subject,
        body,
        html_body,
        date,
        attachments,
        raw: Some(raw.to_vec()),
    })
}
