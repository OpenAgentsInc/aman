# Adding Tools to Aman

This guide explains how to add new tools to the `agent-tools` crate. Tools are external capabilities (calculators, weather APIs, web fetchers, etc.) that the orchestrator can invoke based on user requests.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        ORCHESTRATOR                              │
│  Router classifies message → RoutingPlan includes "use_tool"    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                       TOOL REGISTRY                              │
│  ToolRegistry.execute(name, args) → dispatches to Tool.execute  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                         YOUR TOOL                                │
│  impl Tool: name(), description(), execute(args) → ToolOutput   │
└─────────────────────────────────────────────────────────────────┘
```

**Key components:**

| Component | Location | Purpose |
|-----------|----------|---------|
| `Tool` trait | `crates/agent-tools/src/tool.rs:147-162` | Interface all tools implement |
| `ToolArgs` | `crates/agent-tools/src/tool.rs:13-118` | Input parameters + optional brain |
| `ToolOutput` | `crates/agent-tools/src/tool.rs:120-145` | Success/failure response |
| `ToolError` | `crates/agent-tools/src/error.rs` | Error types for tool failures |
| `ToolRegistry` | `crates/agent-tools/src/registry.rs` | Tool lookup and dispatch |
| `default_registry()` | `crates/agent-tools/src/lib.rs:74-95` | Creates registry with all tools |

## The Tool Trait

Every tool implements this trait:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    /// The tool's unique name (used for dispatch).
    fn name(&self) -> &str;

    /// Human-readable description of what the tool does.
    fn description(&self) -> &str;

    /// Execute the tool with the given arguments.
    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError>;
}
```

## ToolArgs Helper Methods

`ToolArgs` provides convenient parameter extraction:

```rust
// Required parameters (return Err if missing)
args.get_string("location")?           // String parameter
args.get_number("amount")?             // f64 parameter (alias: get_f64)
args.get_bool("include_details")?      // Boolean parameter

// Optional parameters (return None/default if missing)
args.get_string_opt("format")          // Option<String>
args.get_number_opt("limit")?          // Result<Option<f64>, ToolError>
args.get_bool_opt("verbose")?          // Result<Option<bool>, ToolError>
args.get_bool_or("summarize", false)   // bool with default

// Access to brain (for AI-powered tools)
if let Some(brain) = &args.brain {
    // Use brain for summarization, etc.
}
```

## ToolOutput Patterns

```rust
// Success response
ToolOutput::success("The result is 42")
ToolOutput::success(format!("Weather in {}: {}", location, data))

// Failure response (tool ran but couldn't complete)
ToolOutput::failure("Location not found")
ToolOutput::failure(format!("API error: {}", e))
```

**When to use which:**
- `Ok(ToolOutput::success(...))` - Tool completed successfully
- `Ok(ToolOutput::failure(...))` - Tool ran but couldn't fulfill request (e.g., city not found)
- `Err(ToolError::...)` - Tool couldn't run at all (e.g., missing parameter, network error)

## ToolError Variants

```rust
pub enum ToolError {
    NotFound(String),           // Tool not in registry
    MissingParameter(String),   // Required param missing
    InvalidParameter { name, reason },  // Param has wrong type/format
    HttpError(reqwest::Error),  // Network request failed
    JsonError(serde_json::Error), // JSON parsing failed
    EvalError(String),          // Expression evaluation failed
    ExecutionFailed(String),    // General execution error
    BrainError(String),         // AI processing error
}
```

## Step-by-Step Guide

### 1. Create Tool Module

Create a new file in `crates/agent-tools/src/tools/`:

```rust
// crates/agent-tools/src/tools/stock_price.rs

//! Stock price tool using a financial API.

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// Stock price tool that fetches current stock prices.
///
/// # Parameters
///
/// - `symbol` (required): Stock ticker symbol (e.g., "AAPL", "GOOGL").
/// - `currency` (optional): Currency for price (default: "USD").
///
/// # Examples
///
/// ```json
/// {"symbol": "AAPL"}
/// {"symbol": "TSLA", "currency": "EUR"}
/// ```
pub struct StockPrice {
    client: reqwest::Client,
}

impl StockPrice {
    /// Create a new stock price tool.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }
}

impl Default for StockPrice {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for StockPrice {
    fn name(&self) -> &str {
        "stock_price"
    }

    fn description(&self) -> &str {
        "Fetches current stock prices by ticker symbol. \
         Supports major exchanges (NYSE, NASDAQ, etc.)."
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let symbol = args.get_string("symbol")?;
        let currency = args.get_string_opt("currency")
            .unwrap_or_else(|| "USD".to_string());

        debug!("Fetching stock price for {} in {}", symbol, currency);

        // Call your API here
        let url = format!(
            "https://api.example.com/stock/{}?currency={}",
            symbol.to_uppercase(),
            currency
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "API returned status {}",
                response.status()
            )));
        }

        let data: serde_json::Value = response.json().await?;

        // Extract price from response
        let price = data["price"]
            .as_f64()
            .ok_or_else(|| ToolError::ExecutionFailed("Invalid price data".to_string()))?;

        Ok(ToolOutput::success(format!(
            "{} is currently ${:.2} {}",
            symbol.to_uppercase(),
            price,
            currency
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::Value;

    fn make_args(symbol: &str) -> ToolArgs {
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), Value::String(symbol.to_string()));
        ToolArgs::new(params)
    }

    #[tokio::test]
    async fn test_missing_symbol() {
        let tool = StockPrice::new();
        let args = ToolArgs::new(HashMap::new());

        let result = tool.execute(args).await;
        assert!(matches!(result, Err(ToolError::MissingParameter(_))));
    }

    #[tokio::test]
    #[ignore] // Integration test - requires network
    async fn test_fetch_price() {
        let tool = StockPrice::new();
        let result = tool.execute(make_args("AAPL")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("AAPL"));
    }
}
```

### 2. Export from tools/mod.rs

Add your module to `crates/agent-tools/src/tools/mod.rs`:

```rust
mod bitcoin_price;
mod calculator;
// ... existing modules ...
mod stock_price;  // ADD THIS
mod weather;
mod web_fetch;
mod world_time;

pub use bitcoin_price::BitcoinPrice;
pub use calculator::Calculator;
// ... existing exports ...
pub use stock_price::StockPrice;  // ADD THIS
pub use weather::Weather;
pub use web_fetch::WebFetch;
pub use world_time::WorldTime;
```

### 3. Export from lib.rs

Add the public export in `crates/agent-tools/src/lib.rs`:

```rust
pub use tools::{
    sanitize_system_prompt, BitcoinPrice, Calculator, CryptoPrice, CurrencyConverter, Dictionary,
    RandomNumber, Sanitize, StockPrice, UnitConverter, Weather, WebFetch, WorldTime,  // ADD StockPrice
};
```

### 4. Register in default_registry()

Add your tool to the registry in `crates/agent-tools/src/lib.rs`:

```rust
pub fn default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    // Utility tools
    registry.register(Calculator::new());
    registry.register(Weather::new());
    registry.register(WebFetch::new());
    // ... existing tools ...

    // Financial tools
    registry.register(BitcoinPrice::new());
    registry.register(CryptoPrice::new());
    registry.register(CurrencyConverter::new());
    registry.register(StockPrice::new());  // ADD THIS

    // AI-powered tools
    registry.register(Sanitize::new());

    registry
}
```

### 5. Update ROUTER_PROMPT.md

Add your tool to the router's available tools list so it can route requests to it:

```markdown
### Tool Actions
- "use_tool": Execute a specific tool. Include "name" field (tool name) and "args" field (JSON object with parameters).
  - Available tools:
    - "calculator": Evaluate math expressions. Args: {"expression": "2+2*3"}
    - "weather": Get weather for a location. Args: {"location": "NYC"}
    - ...existing tools...
    - "stock_price": Get current stock price. Args: {"symbol": "AAPL", "currency": "USD"}  // ADD THIS
```

Also add examples:

```markdown
[MESSAGE: what's Apple's stock price?]
[ATTACHMENTS: none]
→ {"actions": [{"type": "use_tool", "name": "stock_price", "args": {"symbol": "AAPL"}, "message": "Checking stock..."}, {"type": "respond", "sensitivity": "insensitive", "task_hint": "quick"}]}
```

### 6. Add Unit Tests

See the example in step 1. Key test patterns:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::Value;

    // Helper to create ToolArgs
    fn make_args(params: &[(&str, Value)]) -> ToolArgs {
        let mut map = HashMap::new();
        for (k, v) in params {
            map.insert(k.to_string(), v.clone());
        }
        ToolArgs::new(map)
    }

    #[tokio::test]
    async fn test_missing_required_param() {
        let tool = YourTool::new();
        let result = tool.execute(ToolArgs::new(HashMap::new())).await;
        assert!(matches!(result, Err(ToolError::MissingParameter(_))));
    }

    #[tokio::test]
    async fn test_invalid_param_type() {
        let tool = YourTool::new();
        let args = make_args(&[("number_param", Value::String("not a number".into()))]);
        let result = tool.execute(args).await;
        assert!(matches!(result, Err(ToolError::InvalidParameter { .. })));
    }

    #[tokio::test]
    #[ignore] // Integration tests that need network
    async fn test_real_api_call() {
        // ...
    }
}
```

## Tool Patterns

### Stateless Tools (e.g., Calculator)

Pure computation with no external dependencies:

```rust
pub struct Calculator;

impl Calculator {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Tool for Calculator {
    fn name(&self) -> &str { "calculator" }
    fn description(&self) -> &str { "Evaluates mathematical expressions" }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let expression = args.get_string("expression")?;

        match meval::eval_str(&expression) {
            Ok(result) => Ok(ToolOutput::success(format!("{} = {}", expression, result))),
            Err(e) => Err(ToolError::EvalError(e.to_string())),
        }
    }
}
```

**Characteristics:**
- No HTTP client or external state
- Unit struct (no fields)
- Fast, deterministic results
- Easy to test without mocking

### HTTP-based Tools (e.g., Weather, BitcoinPrice)

Tools that call external APIs:

```rust
pub struct Weather {
    client: reqwest::Client,
}

impl Weather {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("YourApp/1.0")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }
}

#[async_trait]
impl Tool for Weather {
    // ...
    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let location = args.get_string("location")?;

        let response = self.client
            .get(&format!("https://api.example.com/weather/{}", location))
            .send()
            .await?;  // ? converts reqwest::Error to ToolError::HttpError

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(
                format!("API returned {}", response.status())
            ));
        }

        let data: WeatherResponse = response.json().await?;
        Ok(ToolOutput::success(format!("Weather in {}: {}", location, data.summary)))
    }
}
```

**Characteristics:**
- Holds `reqwest::Client` for connection pooling
- Sets appropriate timeout and user-agent
- Handles HTTP errors gracefully
- Parses JSON responses

### Brain-powered Tools (e.g., WebFetch with summarization, Sanitize)

Tools that use AI for processing:

```rust
pub struct WebFetch {
    client: reqwest::Client,
}

#[async_trait]
impl Tool for WebFetch {
    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let url = args.get_string("url")?;
        let summarize = args.get_bool_or("summarize", false);

        // Fetch the content
        let content = self.fetch_url(&url).await?;

        if summarize {
            // Use the brain for AI summarization
            let brain = args.brain.as_ref()
                .ok_or_else(|| ToolError::BrainError("No brain available".to_string()))?;

            let summary = brain.summarize(&content).await
                .map_err(|e| ToolError::BrainError(e.to_string()))?;

            Ok(ToolOutput::success(summary))
        } else {
            Ok(ToolOutput::success(content))
        }
    }
}
```

**Characteristics:**
- Access brain via `args.brain`
- Check brain availability before using
- Convert brain errors to `ToolError::BrainError`
- The orchestrator sets the brain on the registry automatically

## Checklist

When adding a new tool, ensure you've completed:

- [ ] Created tool module in `crates/agent-tools/src/tools/`
- [ ] Implemented `Tool` trait with `name()`, `description()`, `execute()`
- [ ] Added module to `crates/agent-tools/src/tools/mod.rs`
- [ ] Exported struct from `crates/agent-tools/src/lib.rs`
- [ ] Registered in `default_registry()` in `crates/agent-tools/src/lib.rs`
- [ ] Updated `ROUTER_PROMPT.md` with tool description and examples
- [ ] Added unit tests for parameter validation
- [ ] Added integration tests (marked `#[ignore]`) for API calls
- [ ] Ran `cargo test -p agent-tools` to verify tests pass
- [ ] Ran `cargo build -p agent-tools` to verify compilation

## Testing Your Tool

```bash
# Run unit tests
cd crates/agent-tools && cargo test

# Run integration tests (requires network)
cd crates/agent-tools && cargo test -- --ignored

# Test via the bot
./scripts/dev.sh --build
# Then send a message like "what's Apple's stock price?"
```

## See Also

- [Adding Actions](./ADDING_ACTIONS.md) - How to add new orchestrator actions
- [Architecture](./ARCHITECTURE.md) - Overall system architecture
- `crates/agent-tools/src/tools/calculator.rs` - Simple stateless example
- `crates/agent-tools/src/tools/weather.rs` - HTTP-based example
- `crates/agent-tools/src/tools/web_fetch.rs` - Brain-powered example
