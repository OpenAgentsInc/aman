# proton-proxy

SMTP client for sending end-to-end encrypted email between Proton users via [Proton Mail Bridge](https://proton.me/mail/bridge).

## Overview

This crate provides a simple async interface for sending emails through Proton Mail Bridge's local SMTP server. When sending to other Proton users, emails are automatically end-to-end encrypted.

## Prerequisites

1. **Proton Mail paid plan** - Bridge requires Mail Plus, Unlimited, or business plan
2. **Proton Mail Bridge installed and running** - [Download here](https://proton.me/mail/bridge)
3. **Bridge password** - Found in Bridge GUI under "Mailbox details" (NOT your Proton account password)

## Setup

1. Install and launch Proton Mail Bridge
2. Log in with your Proton account
3. Copy the Bridge password from the GUI
4. Configure environment variables:

```bash
export PROTON_SMTP_HOST=127.0.0.1      # Default Bridge host
export PROTON_SMTP_PORT=1025            # Default Bridge SMTP port
export PROTON_USERNAME=you@proton.me    # Your Proton email
export PROTON_PASSWORD=bridge-password  # From Bridge GUI
```

## Usage

```rust
use proton_proxy::{ProtonClient, ProtonConfig, Email, Attachment};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create config (reads from env vars by default)
    let config = ProtonConfig::from_env()?;
    
    // Create client with connection pooling
    let client = ProtonClient::new(config)?;
    
    // Send a simple email
    let email = Email::new("recipient@proton.me", "Hello!", "This is E2E encrypted.");
    client.send(&email).await?;
    
    // Send with attachment
    let mut email = Email::new("recipient@proton.me", "Document", "See attached.");
    email.attach(Attachment::from_file("document.pdf")?);
    client.send(&email).await?;
    
    Ok(())
}
```

## Features

- **Connection pooling** - Efficient for batch sending
- **Attachment support** - Auto-detects MIME types
- **HTML emails** - Optional HTML body with text fallback
- **Multiple recipients** - To, CC, BCC support
- **Async/await** - Built on Tokio

## Security Notes

- Bridge passwords should be stored securely (uses `secrecy` crate)
- Bridge only accepts connections from localhost
- All Proton-to-Proton emails are E2E encrypted automatically
- Your Proton account password never leaves your machine

## Testing

Integration tests are ignored by default because they require a running Bridge:

```bash
PROTON_USERNAME=you@proton.me \
PROTON_PASSWORD=bridge-password \
cargo test -p proton-proxy --test send_test_email -- --ignored
```

## License

MIT
