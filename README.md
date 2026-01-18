# Aman - Privacy-focused AI assistant for Signal

## What is Aman?

Aman (meaning "trust" or "safety" in several languages) is a privacy-focused AI chatbot for activists, journalists, and human rights defenders that operates entirely through Signal messenger. The system provides intelligent, privacy-respecting assistance through a dual-brain architecture: **Maple AI** (OpenSecret TEE) for sensitive requests and **Grok AI** (xAI) for speed and real-time search.

**Key Differentiators:**
- End-to-end encrypted communications via Signal
- Privacy-preserving AI processing using Trusted Execution Environments (TEE)
- Intelligent sensitivity-based routing between privacy and speed modes
- PII detection and sanitization capabilities
- Rich tool ecosystem for real-world tasks

## Current Capabilities (MVP)

- Signal-based chat via a dedicated account
- Orchestrated routing with Maple (privacy) + Grok (speed) and action plans
- Sensitivity + task-hint classification with per-request model overrides
- Router/system prompt hashing for reproducible routing decisions
- User preference commands (privacy/speed) and direct Grok/Maple overrides
- Built-in tools: calculator, weather, web fetch, dictionary, world time, crypto, currency
- ToolExecutor adapter for agent-tools with allowlists, rate limits, and caching
- Attachment-aware routing (vision tasks stay on Maple)
- Opt-in regional alerts (outages, throttling, advisories)
- Basic onboarding and subscription management
- Durable preferences + rolling conversation summaries (SQLite, if configured)
- Tool history + clear-context events for auditability (SQLite, if configured)
- Operator broadcasts and dashboards
- Optional web UI for browser chat

## What it is not yet

- Full RAG pipeline (planned)
- Nostr-backed persistence (foundation only)
- Long-term message transcript storage
- Automated event ingestion from live feeds
- End-to-end durable memory in the brain crates (Maple/Grok history is still in-memory)

## Quickstart (local dev)

See `docs/AMAN_LOCAL_DEV.md` for the shortest path to running Aman locally.

## Prompt Configuration

The bot's behavior is controlled by two prompt files at the project root:

| File | Purpose | Env Override |
|------|---------|--------------|
| `SYSTEM_PROMPT.md` | Main bot persona and response style | `MAPLE_SYSTEM_PROMPT` or `MAPLE_PROMPT_FILE` |
| `ROUTER_PROMPT.md` | Message classification and action routing | `ROUTER_SYSTEM_PROMPT` or `ROUTER_PROMPT_FILE` |

Edit these files to customize the bot's personality, tone, and routing behavior without recompiling.

## Docs and references

- `docs/ARCHITECTURE.md` - system design
- `docs/DATA_RETENTION.md` - storage and safety
- `docs/signal-cli-daemon.md` - Signal daemon details
- `ROADMAP.md` - next phases
- `crates/README.md` - crate catalog
- `crates/orchestrator/README.md` - routing + action orchestration
- `web/README.md` - browser UI

## Safety Posture

Aman is designed for high-risk contexts: opt-in alerts only, "stop" honored everywhere,
minimal retention, and no message body logging by default. Routing and classification
decisions can be made inside the Maple TEE, but the server is still a trusted boundary
because Signal E2EE terminates there.

## License

MIT
