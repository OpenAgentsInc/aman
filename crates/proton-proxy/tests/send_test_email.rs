//! Integration test for sending email via Proton Mail Bridge.
//!
//! Prerequisites:
//! 1. Proton Mail Bridge running locally
//! 2. Environment variables set:
//!    - PROTON_USERNAME
//!    - PROTON_PASSWORD
//!    - PROTON_TEST_RECIPIENT (email to send test to)
//!
//! Run with:
//! ```bash
//! PROTON_TEST_RECIPIENT=recipient@proton.me cargo test -p proton-proxy --test send_test_email -- --ignored
//! ```

use proton_proxy::{Email, ProtonClient, ProtonConfig, ProtonError};

/// Send a simple test email.
///
/// This test is ignored by default because it requires Bridge to be running.
/// Run with `cargo test --ignored` to execute.
#[tokio::test]
#[ignore = "requires Proton Bridge running and valid credentials"]
async fn test_send_simple_email() -> Result<(), ProtonError> {
    // Load .env if present
    let _ = dotenvy::dotenv();

    let config = ProtonConfig::from_env()?;
    let recipient = config.username.clone();
    let client = ProtonClient::new(config)?;

    let email = Email::new(
        &recipient,
        "proton-proxy test email",
        "This is a test email sent from the proton-proxy integration test.\n\nIf you received this, the test passed!",
    );

    client.send(&email).await?;
    println!("✓ Test email sent to {}", recipient);

    Ok(())
}

/// Test sending email with HTML content.
#[tokio::test]
#[ignore = "requires Proton Bridge running and valid credentials"]
async fn test_send_html_email() -> Result<(), ProtonError> {
    let _ = dotenvy::dotenv();

    let config = ProtonConfig::from_env()?;
    let recipient = config.username.clone();
    let client = ProtonClient::new(config)?;

    let mut email = Email::new(
        &recipient,
        "proton-proxy HTML test",
        "This is the plain text fallback.",
    );
    email.with_html("<h1>HTML Test</h1><p>This email contains <strong>HTML</strong> content.</p>");

    client.send(&email).await?;
    println!("✓ HTML test email sent to {}", recipient);

    Ok(())
}

/// Test sending email with attachment.
#[tokio::test]
#[ignore = "requires Proton Bridge running and valid credentials"]
async fn test_send_email_with_attachment() -> Result<(), ProtonError> {
    use proton_proxy::Attachment;

    let _ = dotenvy::dotenv();

    let config = ProtonConfig::from_env()?;
    let recipient = config.username.clone();
    let client = ProtonClient::new(config)?;

    let mut email = Email::new(
        &recipient,
        "proton-proxy attachment test",
        "This email has an attachment.",
    );
    email.attach(Attachment::from_bytes(
        "test.txt",
        b"Hello from proton-proxy test!".to_vec(),
    ));

    client.send(&email).await?;
    println!("✓ Email with attachment sent to {}", recipient);

    Ok(())
}

/// Test config loading from environment.
#[test]
fn test_config_from_env() {
    // Set test env vars
    std::env::set_var("PROTON_USERNAME", "test@proton.me");
    std::env::set_var("PROTON_PASSWORD", "test-password");
    std::env::set_var("PROTON_SMTP_HOST", "127.0.0.1");
    std::env::set_var("PROTON_SMTP_PORT", "1025");

    let config = ProtonConfig::from_env().expect("should load config");

    assert_eq!(config.smtp_host, "127.0.0.1");
    assert_eq!(config.smtp_port, 1025);
    assert_eq!(config.username, "test@proton.me");
}

/// Test config with default values.
#[test]
fn test_config_defaults() {
    std::env::set_var("PROTON_USERNAME", "test@proton.me");
    std::env::set_var("PROTON_PASSWORD", "test-password");
    std::env::remove_var("PROTON_SMTP_HOST");
    std::env::remove_var("PROTON_SMTP_PORT");

    let config = ProtonConfig::from_env().expect("should load config");

    assert_eq!(config.smtp_host, "127.0.0.1");
    assert_eq!(config.smtp_port, 1025);
}

/// Test missing required env vars.
#[test]
fn test_config_missing_username() {
    std::env::remove_var("PROTON_USERNAME");
    std::env::set_var("PROTON_PASSWORD", "test-password");

    let result = ProtonConfig::from_env();
    assert!(result.is_err());
}

/// Test Email builder methods.
#[test]
fn test_email_builder() {
    let mut email = Email::new("to@proton.me", "Subject", "Body");
    email
        .add_to("to2@proton.me")
        .add_cc("cc@proton.me")
        .add_bcc("bcc@proton.me")
        .with_html("<p>HTML</p>");

    assert_eq!(email.to.len(), 2);
    assert_eq!(email.cc.len(), 1);
    assert_eq!(email.bcc.len(), 1);
    assert!(email.html_body.is_some());
}
