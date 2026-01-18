# Aman - Privacy-focused AI assistant for Signal

## What is Aman?

Aman (meaning "trust" or "safety" in several languages) is a privacy-focused AI chatbot for activists, journalists, and human rights defenders that operates entirely through Signal messenger. The system provides intelligent, privacy-respecting assistance through a dual-brain architecture: **Maple AI** (OpenSecret TEE) for sensitive requests and **Grok AI** (xAI) for speed and real-time search.

**Key Differentiators:**
- End-to-end encrypted communications via Signal
- Privacy-preserving AI processing using Trusted Execution Environments (TEE)
- Intelligent sensitivity-based routing between privacy and speed modes
- PII detection with privacy-choice prompts (sanitization tool available)
- Rich tool ecosystem for real-world tasks

## Current Capabilities (MVP)

- Signal-based chat via a dedicated account
- Orchestrated routing with Maple (privacy) + Grok (speed) and action plans
- Sensitivity + task-hint classification with per-request model overrides
- Router/system prompt hashing for reproducible routing decisions
- User preference commands (privacy/speed) and direct Grok/Maple overrides
- Response formatting: markdown-to-Signal styles with mode/model/tool footer
- PII detection with user choice (sanitize vs private vs cancel)
- Built-in tools: calculator, weather, web fetch, dictionary, world time, bitcoin price, crypto price, currency conversion, unit conversion, random numbers, sanitize
- ToolExecutor adapter for agent-tools with allowlists, rate limits, and caching
- Attachment-aware routing (vision tasks stay on Maple)
- Opt-in regional alerts (outages, throttling, advisories)
- Basic onboarding and subscription management
- Durable preferences + rolling conversation summaries (SQLite) with memory snapshot prompt hydration
- Tool history + clear-context events feed memory snapshots (SQLite, if configured)
- Memory prompt policy controls (PII redaction, per-sender overrides)
- Memory prompt provenance hashes + optional background compaction
- Optional Nostr-backed memory durability (publish + rehydrate preferences, summaries, tool history, clear-context)
- Operator broadcasts and dashboards
- Optional web UI for browser chat
- OpenAI-compatible API gateway (echo, orchestrator, or OpenRouter proxy modes) with local/Nostr KB injection
- Receive-only Lightning donation wallet crate (LNI-backed; no send/pay functions exposed)
- Cloudflare Worker OpenAI-compatible gateway (OpenRouter + KV memory + D1 KB synced from Nostr, no Signal dependency)
- Nostr knowledge ingestion pipeline (DocManifest/ChunkRef events with optional inline chunk text)

## What it is not yet

- Full RAG pipeline (planned)
- Vector search + citations for Nostr-backed retrieval (worker currently uses keyword/FTS search)
- Long-term message transcript storage
- Automated event ingestion from live feeds
- End-to-end durable memory in the brain crates (Maple/Grok history is still in-memory)
- End-to-end PII sanitization flow (tool exists, orchestration wiring pending)
- Donation flows wired into Signal responses (donation-wallet is currently standalone)

## Knowledge Base Flow (Nostr -> Worker D1)

1) Ingest markdown with `crates/ingester` and publish DocManifest + ChunkRef events to Nostr
   (`--inline-text` embeds a short snippet directly in the events for worker sync).
2) The Cloudflare Worker cron sync pulls Nostr events, upserts docs/chunks into D1, and keeps a
   sync cursor in KV.
3) `/v1/chat/completions` runs keyword/FTS search in D1 and injects a small, capped snippet bundle
   into the system prompt before calling OpenRouter. When KB context is present, the worker skips
   memory injection to avoid mixing sources.
4) `/kb/status`, `/kb/search`, and `/kb/sync` are available for debugging the KB state and forcing
   a backfill (`/kb/sync?full=1`).

Key knobs: `NOSTR_RELAYS`, `NOSTR_KB_AUTHOR`, `NOSTR_SECRETBOX_KEY`, `KB_*` limits, and the
worker KV/D1 bindings. See `workers/aman-gateway/README.md` for the full setup.

## Quickstart (local dev)

See `docs/AMAN_LOCAL_DEV.md` for the shortest path to running Aman locally.

## Prompt Configuration

The bot's behavior is controlled by two prompt files at the project root:

| File | Purpose | Env Override |
|------|---------|--------------|
| `SYSTEM_PROMPT.md` | Main bot persona and response style (Maple + Grok) | `MAPLE_SYSTEM_PROMPT` / `MAPLE_PROMPT_FILE`, or `GROK_SYSTEM_PROMPT` / `GROK_PROMPT_FILE` |
| `ROUTER_PROMPT.md` | Message classification and action routing | `ROUTER_SYSTEM_PROMPT` / `ROUTER_PROMPT_FILE` |

Edit these files to customize the bot's personality, tone, and routing behavior without recompiling.

## Docs and references

- `docs/ARCHITECTURE.md` - system design
- `docs/DATA_RETENTION.md` - storage and safety
- `docs/signal-cli-daemon.md` - Signal daemon details
- `ROADMAP.md` - next phases
- `crates/README.md` - crate catalog
- `crates/orchestrator/README.md` - routing + action orchestration
- `crates/donation-wallet/README.md` - receive-only Lightning wallet
- `workers/aman-gateway/README.md` - Cloudflare Worker OpenAI gateway
- `web/README.md` - browser UI

## Safety Posture

Aman is designed for high-risk contexts: opt-in alerts only, "stop" honored everywhere,
minimal retention, and no message body logging by default. Routing and classification
decisions can be made inside the Maple TEE, but the server is still a trusted boundary
because Signal E2EE terminates there.

## License

Public domain
