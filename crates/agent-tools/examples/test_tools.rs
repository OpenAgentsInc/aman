//! Test all tools in the registry.
//!
//! Run with: cargo run -p agent-tools --example test_tools

use std::collections::HashMap;

use serde_json::Value;
use agent_tools::{default_registry, ToolRegistry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("agent_tools=debug".parse().unwrap()),
        )
        .init();

    println!("=== Agent Tools Crate Test ===\n");

    // Create registry with default tools
    let registry = default_registry();

    // List available tools
    println!("Registered tools:");
    for (name, desc) in registry.get_descriptions() {
        println!("  - {}: {}", name, desc);
    }
    println!();

    // Test Calculator
    test_calculator(&registry).await?;

    // Test Weather (requires network)
    test_weather(&registry).await?;

    // Test WebFetch (requires network)
    test_web_fetch(&registry).await?;

    println!("\n=== All tests completed ===");
    Ok(())
}

async fn test_calculator(registry: &ToolRegistry) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Testing Calculator ---");

    let test_cases = vec![
        ("2 + 2", "4"),
        ("10 * 5", "50"),
        ("sqrt(16)", "4"),
        ("2 + 3 * 4", "14"),
        ("(2 + 3) * 4", "20"),
        ("pi", "3.14"),
        ("e", "2.71"),
    ];

    for (expr, expected_substr) in test_cases {
        let mut params = HashMap::new();
        params.insert("expression".to_string(), Value::String(expr.to_string()));

        match registry.execute("calculator", params).await {
            Ok(result) => {
                if result.content.contains(expected_substr) {
                    println!("  [PASS] {} => {}", expr, result.content);
                } else {
                    println!(
                        "  [FAIL] {} => {} (expected to contain '{}')",
                        expr, result.content, expected_substr
                    );
                }
            }
            Err(e) => {
                println!("  [ERROR] {} => {}", expr, e);
            }
        }
    }

    // Test error case
    let mut params = HashMap::new();
    params.insert("expression".to_string(), Value::String("2 +".to_string()));
    match registry.execute("calculator", params).await {
        Ok(_) => println!("  [FAIL] '2 +' should have failed"),
        Err(_) => println!("  [PASS] '2 +' correctly returned error"),
    }

    println!();
    Ok(())
}

async fn test_weather(registry: &ToolRegistry) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Testing Weather ---");
    println!("  (Requires network access)");

    let mut params = HashMap::new();
    params.insert("location".to_string(), Value::String("London".to_string()));

    match registry.execute("weather", params).await {
        Ok(result) => {
            if result.success {
                println!("  [PASS] London weather: {}", result.content.lines().next().unwrap_or(""));
            } else {
                println!("  [FAIL] Request succeeded but returned failure");
            }
        }
        Err(e) => {
            println!("  [SKIP] Network error: {}", e);
        }
    }

    println!();
    Ok(())
}

async fn test_web_fetch(registry: &ToolRegistry) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Testing WebFetch ---");
    println!("  (Requires network access)");

    let mut params = HashMap::new();
    params.insert(
        "url".to_string(),
        Value::String("https://example.com".to_string()),
    );

    match registry.execute("web_fetch", params).await {
        Ok(result) => {
            if result.success && result.content.contains("Example Domain") {
                println!("  [PASS] Fetched example.com successfully");
            } else {
                println!("  [FAIL] Content doesn't contain expected text");
            }
        }
        Err(e) => {
            println!("  [SKIP] Network error: {}", e);
        }
    }

    // Test invalid URL
    let mut params = HashMap::new();
    params.insert("url".to_string(), Value::String("not-a-url".to_string()));

    match registry.execute("web_fetch", params).await {
        Ok(_) => println!("  [FAIL] Invalid URL should have failed"),
        Err(_) => println!("  [PASS] Invalid URL correctly returned error"),
    }

    println!();
    Ok(())
}
