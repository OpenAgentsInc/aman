//! IMAP integration tests for Proton Mail Bridge
//!
//! These tests require a running Proton Mail Bridge with the following env vars:
//! - PROTON_USERNAME
//! - PROTON_PASSWORD
//! - PROTON_IMAP_HOST (default: 127.0.0.1)
//! - PROTON_IMAP_PORT (default: 1143)

use proton_proxy::{ImapClient, ProtonConfig};

fn get_test_config() -> ProtonConfig {
    ProtonConfig::from_env().expect("Failed to load config from environment")
}

#[tokio::test]
async fn test_imap_connect() {
    let config = get_test_config();
    let client = ImapClient::connect(&config)
        .await
        .expect("Failed to connect to IMAP server");

    client.logout().await.expect("Failed to logout");
}

#[tokio::test]
async fn test_list_folders() {
    let config = get_test_config();
    let mut client = ImapClient::connect(&config)
        .await
        .expect("Failed to connect to IMAP server");

    let folders = client.list_folders().await.expect("Failed to list folders");

    println!("Found folders: {:?}", folders);
    assert!(!folders.is_empty(), "Should have at least one folder");
    assert!(
        folders.iter().any(|f| f == "INBOX" || f.to_uppercase() == "INBOX"),
        "Should have an INBOX folder"
    );

    client.logout().await.expect("Failed to logout");
}

#[tokio::test]
async fn test_select_inbox() {
    let config = get_test_config();
    let mut client = ImapClient::connect(&config)
        .await
        .expect("Failed to connect to IMAP server");

    let count = client
        .select_folder("INBOX")
        .await
        .expect("Failed to select INBOX");

    println!("INBOX has {} messages", count);

    client.logout().await.expect("Failed to logout");
}

#[tokio::test]
async fn test_fetch_uids() {
    let config = get_test_config();
    let mut client = ImapClient::connect(&config)
        .await
        .expect("Failed to connect to IMAP server");

    client
        .select_folder("INBOX")
        .await
        .expect("Failed to select INBOX");

    let uids = client.fetch_uids().await.expect("Failed to fetch UIDs");
    println!("Found {} message UIDs: {:?}", uids.len(), uids);

    client.logout().await.expect("Failed to logout");
}

#[tokio::test]
async fn test_fetch_message() {
    let config = get_test_config();
    let mut client = ImapClient::connect(&config)
        .await
        .expect("Failed to connect to IMAP server");

    client
        .select_folder("INBOX")
        .await
        .expect("Failed to select INBOX");

    let uids = client.fetch_uids().await.expect("Failed to fetch UIDs");

    if let Some(&uid) = uids.first() {
        let message = client
            .fetch_message(uid)
            .await
            .expect("Failed to fetch message");

        println!("Message UID: {}", message.uid);
        println!("From: {:?} ({:?})", message.from, message.from_name);
        println!("To: {:?}", message.to);
        println!("Subject: {}", message.subject);
        println!("Date: {:?}", message.date);
        
        // Safe string truncation respecting UTF-8 boundaries
        if let Some(body) = &message.body {
            let preview: String = body.chars().take(100).collect();
            println!("Body preview: {:?}", preview);
        }
        println!("Attachments: {}", message.attachments.len());
    } else {
        println!("No messages in INBOX to fetch");
    }

    client.logout().await.expect("Failed to logout");
}

#[tokio::test]
async fn test_search_unread() {
    let config = get_test_config();
    let mut client = ImapClient::connect(&config)
        .await
        .expect("Failed to connect to IMAP server");

    client
        .select_folder("INBOX")
        .await
        .expect("Failed to select INBOX");

    let unread_uids = client
        .search_unread()
        .await
        .expect("Failed to search unread");

    println!("Found {} unread messages", unread_uids.len());

    client.logout().await.expect("Failed to logout");
}
