# maple-brain

## Responsibility

OpenSecret-backed Brain implementation for Aman. MapleBrain performs an attestation handshake on startup and uses the
OpenSecret API to generate responses with per-sender conversation history.

## Configuration

Environment variables:

- `MAPLE_API_KEY` (required)
- `MAPLE_API_URL` (optional, default: `https://api.opensecret.cloud`)
- `MAPLE_MODEL` (optional)
- `MAPLE_SYSTEM_PROMPT` (optional)
- `MAPLE_MAX_TOKENS` (optional)
- `MAPLE_TEMPERATURE` (optional)
- `MAPLE_MAX_HISTORY_TURNS` (optional)

## Usage

```rust
use maple_brain::MapleBrain;

#[tokio::main]
async fn main() -> Result<(), brain_core::BrainError> {
    let brain = MapleBrain::from_env().await?;
    println!("Brain name: {}", brain.name());
    Ok(())
}
```

## Run with message-listener

```bash
export MAPLE_API_KEY="..."
cargo run -p message-listener --example maple_bot --features maple
```

## Failure modes

- Missing or invalid `MAPLE_API_KEY`.
- Attestation handshake failure.
- Network/API errors during chat completion.

## Notes

- MapleBrain stores in-memory history per sender (bounded by `MAPLE_MAX_HISTORY_TURNS`).
- For OpenSecret API details, see `docs/OPENSECRET_API.md`.
