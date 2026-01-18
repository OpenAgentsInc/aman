//! Example: Send an email via Proton Mail Bridge
//!
//! Prerequisites:
//! 1. Proton Mail Bridge running locally
//! 2. Environment variables set (or .env file):
//!    - PROTON_USERNAME=your@proton.me
//!    - PROTON_PASSWORD=bridge-password
//!
//! Run with:
//! ```bash
//! cargo run --example send_email
//! ```

use proton_proxy::{Attachment, Email, ProtonClient, ProtonConfig, ProtonError};

#[tokio::main]
async fn main() -> Result<(), ProtonError> {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    // Initialize tracing for logs
    tracing_subscriber::fmt::init();

    // Create client from environment
    let config = ProtonConfig::from_env()?;
    let client = ProtonClient::new(config)?;

    // Example 1: Simple text email
    let email = Email::new(
        "recipient@proton.me",
        "Hello from proton-proxy!",
        "This is a test email sent via Proton Mail Bridge.\n\nIt's end-to-end encrypted!",
    );
    client.send(&email).await?;
    println!("✓ Sent simple email");

    // Example 2: Email with HTML
    let mut html_email = Email::new(
        "recipient@proton.me",
        "HTML Email Test",
        "This is the plain text fallback.",
    );
    html_email.with_html("<h1>Hello!</h1><p>This is <strong>HTML</strong> content.</p>");
    client.send(&html_email).await?;
    println!("✓ Sent HTML email");

    // Example 3: Email with attachment (uncomment to test)
    // let mut email_with_attachment = Email::new(
    //     "recipient@proton.me",
    //     "Document Attached",
    //     "Please find the document attached.",
    // );
    // email_with_attachment.attach(Attachment::from_file("document.pdf")?);
    // client.send(&email_with_attachment).await?;
    // println!("✓ Sent email with attachment");

    // Example 4: Email with inline attachment from bytes
    let mut email_with_data = Email::new("recipient@proton.me", "Data Attachment", "Here's some data.");
    email_with_data.attach(Attachment::from_bytes("data.txt", b"Hello, World!".to_vec()));
    client.send(&email_with_data).await?;
    println!("✓ Sent email with data attachment");

    // Example 5: Multiple recipients
    let multi_email = Email::new_multi(
        ["alice@proton.me", "bob@proton.me"],
        "Group Email",
        "This goes to multiple recipients.",
    );
    client.send(&multi_email).await?;
    println!("✓ Sent multi-recipient email");

    Ok(())
}
