//! Test vision/image capabilities with Maple API.
//!
//! Run with: cargo run -p maple-brain --example test_vision
//!
//! This tests sending an image to a vision-language model.

use opensecret::{OpenSecretClient, types::{ChatCompletionRequest, ChatMessage}};
use futures::StreamExt;
use std::env;

// A small test image (1x1 red pixel PNG, base64 encoded)
const TEST_IMAGE_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file
    let _ = dotenvy::dotenv();

    let api_url = env::var("MAPLE_API_URL")
        .unwrap_or_else(|_| "https://enclave.trymaple.ai".to_string());
    let api_key = env::var("MAPLE_API_KEY")
        .expect("MAPLE_API_KEY must be set");

    println!("Connecting to: {}", api_url);

    let client = OpenSecretClient::new_with_api_key(&api_url, api_key)?;

    println!("Performing attestation handshake...");
    client.perform_attestation_handshake().await?;
    println!("Handshake complete!\n");

    // Use the vision-language model
    let model = "qwen3-vl-30b";
    println!("Using vision model: {}", model);

    // Create a multimodal message with image
    // OpenAI vision format: content is an array of parts
    let content = serde_json::json!([
        {
            "type": "text",
            "text": "What do you see in this image? Describe it briefly."
        },
        {
            "type": "image_url",
            "image_url": {
                "url": format!("data:image/png;base64,{}", TEST_IMAGE_BASE64)
            }
        }
    ]);

    let request = ChatCompletionRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content,
            tool_calls: None,
        }],
        temperature: Some(0.7),
        max_tokens: Some(256),
        stream: Some(true),
        stream_options: None,
        tools: None,
        tool_choice: None,
    };

    println!("Sending image to vision model...\n");

    let mut stream = client.create_chat_completion_stream(request).await?;

    print!("Response: ");
    let mut full_response = String::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                if !chunk.choices.is_empty() {
                    if let Some(serde_json::Value::String(s)) = &chunk.choices[0].delta.content {
                        print!("{}", s);
                        full_response.push_str(s);
                    }
                }
            }
            Err(e) => {
                eprintln!("\nStream error: {}", e);
                break;
            }
        }
    }
    println!("\n");

    if full_response.is_empty() {
        println!("No response received - vision may not be supported or image format incorrect");
    } else {
        println!("Vision test successful!");
    }

    Ok(())
}
