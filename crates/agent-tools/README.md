# agent-tools

Tool registry and built-in tools for the Aman Signal bot orchestrator.

## Overview

This crate provides a `ToolRegistry` for registering and executing tools that the chatbot can use. Tools are external capabilities (web fetch, calculator, weather, etc.) that take input and return output.

Unlike brain-core's `ToolExecutor` trait which handles LLM tool calls (when a model like Maple calls tools), the `Tool` trait here is for orchestrator-level actions dispatched based on routing decisions. The `RegistryToolExecutor` adapter lets you expose the same registry as a `ToolExecutor` for LLM tool calls with optional policy controls.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
agent-tools = { path = "../agent-tools" }
```

## Built-in Tools

### Utility Tools

| Tool | Name | Description | Parameters |
|------|------|-------------|------------|
| Calculator | `calculator` | Safe math evaluation using `meval` | `expression` (string) |
| Weather | `weather` | Weather via wttr.in (no API key) | `location` (string) |
| WebFetch | `web_fetch` | Fetch URL, convert HTML to text | `url`, optional `summarize` |
| Dictionary | `dictionary` | Word definitions via Free Dictionary API | `word` (string) |
| WorldTime | `world_time` | Current time via WorldTimeAPI | `timezone` (string) |
| UnitConverter | `unit_converter` | Convert between units | `value`, `from`, `to` |
| RandomNumber | `random_number` | Random numbers, dice, coin flips | `min`, `max` or `dice`, `count` |

### Financial Tools

| Tool | Name | Description | Parameters |
|------|------|-------------|------------|
| BitcoinPrice | `bitcoin_price` | BTC price via mempool.space | none |
| CryptoPrice | `crypto_price` | Any crypto via CoinGecko | `coin` (string) |
| CurrencyConverter | `currency_converter` | Fiat conversion via exchangerate.host | `amount`, `from`, `to` |

### AI-Powered Tools

| Tool | Name | Description | Requirements |
|------|------|-------------|--------------|
| Sanitize | `sanitize` | PII detection and redaction | Requires brain to be set |

## Usage

### Basic Usage

```rust
use agent_tools::{default_registry, ToolRegistry};
use std::collections::HashMap;
use serde_json::Value;

#[tokio::main]
async fn main() {
    // Create registry with all default tools
    let registry = default_registry();

    // Execute a calculation
    let mut params = HashMap::new();
    params.insert("expression".to_string(), Value::String("2 + 2 * 3".to_string()));

    let result = registry.execute("calculator", params).await.unwrap();
    println!("{}", result.content); // "2 + 2 * 3 = 8"
}
```

### With JSON Arguments

```rust
let result = registry
    .execute_json("weather", r#"{"location": "London"}"#)
    .await?;
println!("{}", result.content);
```

### List Available Tools

```rust
let registry = default_registry();

// Get tool names
for name in registry.list_tools() {
    println!("Tool: {}", name);
}

// Get names and descriptions
for (name, desc) in registry.get_descriptions() {
    println!("{}: {}", name, desc);
}
```

### With Brain for AI Tools

```rust
use std::sync::Arc;

let brain = MapleBrain::from_env().await?;
let mut registry = default_registry();
registry.set_brain(Arc::new(brain));

// Now AI-powered tools like `sanitize` can work
let result = registry
    .execute_json("sanitize", r#"{"text": "Call me at 555-1234"}"#)
    .await?;
```

## Implementing Custom Tools

```rust
use agent_tools::{async_trait, Tool, ToolArgs, ToolOutput, ToolError};

struct GreetTool;

#[async_trait]
impl Tool for GreetTool {
    fn name(&self) -> &str {
        "greet"
    }

    fn description(&self) -> &str {
        "Greets a person by name"
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let name = args.get_string("name")?;
        Ok(ToolOutput::success(format!("Hello, {}!", name)))
    }
}

// Register custom tool
let mut registry = default_registry();
registry.register(GreetTool);
```

## ToolArgs Helper Methods

The `ToolArgs` struct provides convenient methods for extracting parameters:

```rust
// Required parameters (return error if missing)
let s = args.get_string("key")?;
let n = args.get_number("key")?;  // f64
let b = args.get_bool("key")?;

// Optional parameters
let s = args.get_string_opt("key");  // Option<String>
let n = args.get_number_opt("key")?; // Result<Option<f64>>
let b = args.get_bool_opt("key")?;   // Result<Option<bool>>

// Optional with default
let b = args.get_bool_or("key", false);
```

## RegistryToolExecutor Adapter

Expose the registry as a brain-core `ToolExecutor` for LLM tool calls:

```rust
use agent_tools::{RegistryToolExecutor, ToolPolicy, RateLimit, default_registry};
use std::time::Duration;

// Create with default policy (all tools allowed)
let registry = default_registry();
let executor = RegistryToolExecutor::new(registry);

// Or with custom policy
let policy = ToolPolicy::new()
    .allow(&["calculator", "weather"])
    .with_rate_limit(RateLimit::new(10, Duration::from_secs(60)));
let executor = RegistryToolExecutor::with_policy(registry, policy);

// Use with MapleBrain
let brain = MapleBrain::with_tools(config, executor).await?;
```

## Tool Output

Tools return `ToolOutput` with content and success status:

```rust
// Success
ToolOutput::success("Result text")

// Failure
ToolOutput::failure("Error message")

// Check result
if result.success {
    println!("Output: {}", result.content);
} else {
    eprintln!("Error: {}", result.content);
}
```

## Error Handling

```rust
use agent_tools::ToolError;

match registry.execute("calculator", params).await {
    Ok(output) => {
        if output.success {
            println!("{}", output.content);
        } else {
            eprintln!("Tool failed: {}", output.content);
        }
    }
    Err(ToolError::NotFound(name)) => {
        eprintln!("Unknown tool: {}", name);
    }
    Err(ToolError::MissingParameter(param)) => {
        eprintln!("Missing parameter: {}", param);
    }
    Err(ToolError::InvalidParameter { name, reason }) => {
        eprintln!("Invalid parameter {}: {}", name, reason);
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

## Examples

Run the included example:

```bash
cargo run -p agent-tools --example tools_demo
```

## Testing

```bash
cargo test -p agent-tools
```

## Adding New Tools

For a detailed guide on adding new tools, see [docs/ADDING_TOOLS.md](../../docs/ADDING_TOOLS.md).

Quick checklist:
1. Create `crates/agent-tools/src/tools/your_tool.rs`
2. Export in `crates/agent-tools/src/tools/mod.rs`
3. Re-export in `crates/agent-tools/src/lib.rs`
4. Register in `default_registry()` in `lib.rs`
5. Update `ROUTER_PROMPT.md` if the tool should be directly routable

## Security Notes

- Calculator uses `meval` for safe expression evaluation (no arbitrary code execution)
- WebFetch respects timeouts and size limits
- Tools receive sanitized inputs from the orchestrator
- AI-powered tools require explicit brain configuration
