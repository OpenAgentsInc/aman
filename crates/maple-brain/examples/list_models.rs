//! List available models from OpenSecret API.
//!
//! Run with: cargo run -p maple-brain --example list_models

use opensecret::OpenSecretClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file
    let _ = dotenvy::dotenv();

    let api_url = env::var("MAPLE_API_URL")
        .unwrap_or_else(|_| "https://api.opensecret.cloud".to_string());
    let api_key = env::var("MAPLE_API_KEY")
        .expect("MAPLE_API_KEY must be set");

    println!("Connecting to: {}", api_url);

    let client = OpenSecretClient::new_with_api_key(&api_url, api_key)?;

    println!("Performing attestation handshake...");
    client.perform_attestation_handshake().await?;
    println!("Handshake complete!\n");

    println!("Fetching available models...");
    let models = client.get_models().await?;

    println!("\n=== Available Models ===");
    for model in &models.data {
        println!("  - {} (owned by: {:?})", model.id, model.owned_by);
    }
    println!("========================\n");

    println!("Total: {} models", models.data.len());

    Ok(())
}
