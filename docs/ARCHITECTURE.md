# Aman Signal MVP Architecture

## Table of Contents

- [Overview](#overview)
- [System Architecture](#system-architecture)
- [Message Flow](#message-flow)
- [Crate Dependency Graph](#crate-dependency-graph)
- [Quick Start](#quick-start)
- [Goal](#goal)
- [Components](#components)
- [Data Model](#data-model-mvp-intent)
- [State Machine](#state-machine-onboarding--subscriptions)
- [Flows](#flows)
- [Reliability](#reliability)
- [Configuration](#configuration)
- [Privacy Architecture](#privacy-architecture)
- [Safety Posture](#safety-posture)
- [Future Architecture](#future-architecture-rag-and-nostr)
- [Glossary](#glossary)
- [Security Notes](#security-notes)

---

## Overview

Aman is a Signal-native chatbot that provides AI-powered conversations with privacy-preserving routing and optional regional alerts for activists and human-rights defenders.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SIGNAL LAYER                                    │
│  ┌─────────────────┐     ┌─────────────────────────────────────────────┐   │
│  │   Signal App    │◀───▶│            signal-cli daemon                │   │
│  │  (User's Phone) │     │  (JSON-RPC + SSE on HTTP_ADDR:8080)         │   │
│  └─────────────────┘     └────────────────────┬────────────────────────┘   │
└───────────────────────────────────────────────│─────────────────────────────┘
                                                │
                                                ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           TRANSPORT LAYER                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        signal-daemon                                 │   │
│  │  SignalClient (HTTP) + DaemonProcess (JAR spawn) + SSE Stream       │   │
│  │  Auto-reconnection • Styled text • Multi-account support            │   │
│  └────────────────────────────────────┬────────────────────────────────┘   │
│                                       │                                     │
│  ┌────────────────────────────────────┼────────────────────────────────┐   │
│  │              message-listener      │         broadcaster            │   │
│  │  (inbound messages + attachments)  │    (outbound + chunking)       │   │
│  └────────────────────────────────────┴────────────────────────────────┘   │
└───────────────────────────────────────│─────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          ORCHESTRATION LAYER                                 │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                          orchestrator                                │   │
│  │                                                                      │   │
│  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────────┐  │   │
│  │  │   Router    │───▶│RoutingPlan │───▶│   Action Executor       │  │   │
│  │  │(maple TEE)  │    │  (actions)  │    │ (search,tool,respond..) │  │   │
│  │  └─────────────┘    └─────────────┘    └─────────────────────────┘  │   │
│  │                                                                      │   │
│  │  Sensitivity routing • User preferences • Task hints • Memory       │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                        │                                     │
│  ┌─────────────────────────────────────┴───────────────────────────────┐   │
│  │                        agent-tools                                   │   │
│  │  Calculator • Weather • WebFetch • Dictionary • WorldTime           │   │
│  │  BitcoinPrice • CryptoPrice • CurrencyConverter • RandomNumber      │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
└───────────────────────────────────────│─────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                             BRAIN LAYER                                      │
│                                                                              │
│  ┌──────────────────────────────┐     ┌──────────────────────────────┐     │
│  │         maple-brain          │     │         grok-brain           │     │
│  │      (OpenSecret TEE)        │◀────│        (xAI Grok)            │     │
│  │                              │     │                              │     │
│  │  • Privacy-first processing  │     │  • Real-time search          │     │
│  │  • Vision support            │     │  • Web + X/Twitter search    │     │
│  │  • Tool calling              │     │  • Fast responses            │     │
│  │  • Attestation handshake     │     │  • GrokToolExecutor          │     │
│  └──────────────────────────────┘     └──────────────────────────────┘     │
│                    │                               │                        │
│                    └───────────────┬───────────────┘                        │
│                                    ▼                                        │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                          brain-core                                   │  │
│  │  Brain trait • ToolExecutor • ConversationHistory • Message types    │  │
│  │  MemorySnapshot • MemoryStore • TextStyle • RoutingInfo              │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           PERSISTENCE LAYER                                  │
│                                                                              │
│  ┌──────────────────────────────┐     ┌──────────────────────────────┐     │
│  │          database            │     │     nostr-persistence        │     │
│  │        (SQLite/SQLx)         │     │    (Nostr events + relay)    │     │
│  │                              │     │                              │     │
│  │  • Users, topics, subs       │     │  • DocManifest events        │     │
│  │  • Preferences               │     │  • ChunkRef events           │     │
│  │  • Conversation summaries    │     │  • AccessPolicy events       │     │
│  │  • Tool history              │     │  • Encrypted payloads        │     │
│  └──────────────────────────────┘     └──────────────────────────────┘     │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Message Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         INBOUND MESSAGE FLOW                                 │
│                                                                              │
│  Signal App ──▶ signal-cli daemon ──▶ SSE Stream ──▶ message-listener       │
│                                                              │               │
│                                                              ▼               │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                        ORCHESTRATOR PIPELINE                           │  │
│  │                                                                        │  │
│  │  1. Start typing indicator                                             │  │
│  │  2. Load durable memory (summary, tool history, clear-context)        │  │
│  │  3. Router classifies message (in Maple TEE):                         │  │
│  │     • Sensitivity: sensitive / insensitive / uncertain                │  │
│  │     • Task hint: general / coding / math / creative / vision / quick  │  │
│  │     • PII detection: name, phone, email, ssn, card, address, etc.     │  │
│  │  4. Execute action plan:                                               │  │
│  │     • search      → Grok real-time search                             │  │
│  │     • use_tool    → agent-tools (calculator, weather, etc.)           │  │
│  │     • respond     → Generate response via Maple or Grok               │  │
│  │     • clear_ctx   → Clear conversation history                        │  │
│  │     • set_pref    → Update user preference                            │  │
│  │  5. Format response with markdown-to-Signal styles                    │  │
│  │  6. Stop typing indicator                                              │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                              │                               │
│                                              ▼                               │
│  broadcaster ──▶ signal-daemon ──▶ signal-cli daemon ──▶ Signal App         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Crate Dependency Graph

```
                              ┌─────────────────┐
                              │   brain-core    │
                              │  (traits+types) │
                              └────────┬────────┘
                    ┌─────────────────┼─────────────────┐
                    │                 │                 │
                    ▼                 ▼                 ▼
           ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
           │  maple-brain │  │  grok-brain  │  │  mock-brain  │
           └──────┬───────┘  └──────┬───────┘  └──────────────┘
                  │                 │
                  └────────┬────────┘
                           │
                           ▼
              ┌────────────────────────┐     ┌─────────────┐
              │      orchestrator      │────▶│ agent-tools │
              └───────────┬────────────┘     └─────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
          ▼               ▼               ▼
┌─────────────────┐ ┌──────────┐ ┌────────────────────┐
│ message-listener│ │ database │ │ nostr-persistence  │
└────────┬────────┘ └──────────┘ └────────────────────┘
         │
         ▼
┌─────────────────┐     ┌─────────────────┐
│  signal-daemon  │◀────│   broadcaster   │
└─────────────────┘     └─────────────────┘
```

Other workspace crates: `donation-wallet` (receive-only Lightning wallet built on the `repos/lni` submodule).

## Quick Start

To understand the architecture, start with these key concepts:

1. **Signal as the UI**: Users interact via Signal messenger; there's no web UI for chat (yet)
2. **Two-brain system**: Maple (privacy-first TEE) and Grok (fast search) work together
3. **Privacy routing**: Sensitive messages always stay in Maple; insensitive can use Grok
4. **Tool augmentation**: Built-in tools (weather, calculator, etc.) extend capabilities
5. **Durable memory**: SQLite stores conversation summaries and preferences across restarts

**Key entry points for developers:**
- Message processing starts in `message-listener` → `orchestrator`
- Brain implementations live in `maple-brain` and `grok-brain`
- All shared types are in `brain-core`
- Run `./scripts/dev.sh --build` to start the bot

---

## Goal

- Signal-native messaging experience.
- Opt-in regional alerts for activists and human-rights defenders.
- Core crates: `signal-daemon`, `message-listener`, `agent-brain`, `broadcaster`, `api`, `ingester`, `admin-web`, `donation-wallet`, `grok-brain`, `orchestrator`, `agent-tools`.
- Cloudflare Worker gateway: `workers/aman-gateway` (OpenAI-compatible endpoint, OpenRouter-backed, KV memory + D1 KB synced from Nostr).
- Data persistence crate: `database` (SQLite via SQLx).
- Brain interface crate: `brain-core` (shared Brain trait, message types, and ConversationHistory).
- Optional brain crate: `maple-brain` (OpenSecret-based AI backend).
- Orchestration crate: `orchestrator` (routes messages, executes action plans, coordinates brains).
- Tooling crate: `agent-tools` (tool registry and built-in capabilities for the orchestrator).
- Test harness crate: `mock-brain` (mock implementations for message flow testing, built on `brain-core`).
- Regional event ingestion as a subsystem/service (documented under `agent_brain::regional_events`).
- For planned phases beyond the MVP, see `ROADMAP.md`.

## Components

- `signal-cli daemon` (process)
  - Runs the Signal account and exposes HTTP/SSE and JSON-RPC endpoints.
- Web UI (Next.js app in `web/`)
  - Browser-based chat surface with `/api/chat`.
  - Uses the OpenAI-compatible API directly; not yet wired to Signal services.
- Admin web UI (crate: `crates/admin-web`)
  - Dashboard + broadcast tooling for operators.
  - Reads from SQLite and sends broadcasts via `broadcaster`.
- `donation-wallet` (crate: `crates/donation-wallet`)
  - Receive-only Lightning wallet wrapper (no send/pay functions exposed).
  - Uses the LNI submodule (`repos/lni`) for backend support.
  - Intended for donation flows; not yet wired into Signal responses.
- `api` (crate: `crates/api`)
  - OpenAI-compatible inference gateway (`/v1/chat/completions`, `/v1/models`).
  - Uses a local knowledge base (if configured), orchestrator brain, or OpenRouter inference mode.
- `aman-gateway-worker` (Cloudflare Worker in `workers/aman-gateway`)
  - OpenAI-compatible endpoint for web clients (no Signal dependency).
  - Uses OpenRouter for inference, KV for memory snapshots, and D1 for KB storage.
  - Scheduled sync pulls DocManifest/ChunkRef events from Nostr relays into D1.
  - Injects KB snippets into prompts and skips memory injection when KB context is present.
  - Exposes `/kb/status`, `/kb/search`, and `/kb/sync` debug endpoints.
- `ingester` (crate: `crates/ingester`)
  - Chunks local files into blob refs and publishes DocManifest + ChunkRef events.
  - Can index directly into a local Nostr SQLite DB for testing.
- `signal-daemon` (crate: `crates/signal-daemon`)
  - HTTP/SSE client for signal-cli daemon.
  - Shared dependency for inbound and outbound transport.
  - SSE auto-reconnection with configurable exponential backoff.
  - Supports styled text via `textStyle` ranges for Signal formatting.
- `message_listener` (crate: `crates/message-listener`)
  - Owns Signal inbound transport via `signal-daemon` (HTTP/SSE).
  - Normalizes inbound messages into `InboundMessage` records (including attachments metadata).
  - Emits normalized events into the local queue/state store.
  - Forwards `OutboundMessage.styles` to Signal as styled text when present.
  - Configurable brain processing timeout (default: 60s) prevents pipeline hangs.
  - Graceful shutdown via `run_with_shutdown()` or `run_until_stopped()`.
  - Supports attachment-only messages (configurable).
- `brain-core` (crate: `crates/brain-core`)
  - Shared Brain trait, ToolExecutor trait, and message types for AI backends.
  - Defines attachments metadata (`InboundAttachment`) for inbound processing.
  - Provides `TextStyle` ranges and `OutboundMessage.styles` for formatted replies.
  - Provides `ConversationHistory` for per-sender message history with auto-trimming.
  - Defines MemorySnapshot/MemoryStore contract and memory prompt formatter.
  - Routing metadata includes memory provenance (prompt hashes + source).
  - Exposes routing metadata and prompt hashing helpers for reproducibility.
- `maple-brain` (crate: `crates/maple-brain`)
  - OpenSecret-based Brain implementation with attestation handshake, per-sender history, and vision support.
  - Optional tool calling via `ToolExecutor` (e.g., Grok real-time search).
  - Optional integration via `message-listener` (feature `maple`).
- `grok-brain` (crate: `crates/grok-brain`)
  - xAI Grok Brain implementation with optional X/Web search.
  - Provides `GrokToolExecutor` for MapleBrain tool calls.
- `orchestrator` (crate: `crates/orchestrator`)
  - Message routing and action plan execution.
  - Routes messages through a privacy-preserving classifier (maple-brain TEE).
  - Executes multi-step actions: search, clear context, respond, help, tool usage, privacy choice prompts.
  - Supports direct Grok/Maple commands and per-sender preference switching.
  - Attaches task hints for model selection and enforces vision-only routing to Maple.
  - Coordinates maple-brain (for routing and responses) and grok-brain (for search).
  - Maintains typing indicators and sends interim status messages.
  - Formats responses with markdown-to-Signal styles and metadata footers.
  - Persists preferences, rolling summaries, and tool history in SQLite (when configured).
  - Hydrates durable memory snapshots into prompts with policy controls.
- `agent-tools` (crate: `crates/agent-tools`)
  - Tool registry and implementations for orchestrator-level capabilities.
  - Built-in tools: calculator, weather, web fetch + summarize, dictionary, world time, bitcoin price, crypto price, currency converter, unit converter, random number, sanitize.
  - Registry adapter for `brain-core::ToolExecutor` with policy controls.
- `agent_brain` (crate: `crates/agent-brain`)
  - Owns message handling, onboarding state machine, and routing decisions.
  - Implements the `Brain` trait for use with `message_listener`.
  - Decides when to respond vs. when to update subscription state.
  - Provides a simple `agent_brain_bot` binary for local Signal MVP use.
- `broadcaster` (crate: `crates/broadcaster`)
  - Owns outbound delivery via `signal-daemon` (HTTP to signal-cli daemon).
  - Handles chunking, retries, and throttling.
- `database` (crate: `crates/database`)
  - SQLite persistence for users, topics, notifications, and durable memory tables.
  - Runs migrations and exposes async CRUD helpers.
- `mock-brain` (crate: `crates/mock-brain`)
  - Mock brain implementations for testing message processing without an AI backend.
  - Built on the `brain-core` trait and message types.
- `regional_event_listener` (subsystem)
  - Ingests regional events from external feeds or fixtures.
  - Normalizes to `RegionEvent` and hands off to `agent_brain`.
  - MVP helper: `region_event_send` reads a JSON event file and fans out alerts.
- Local storage
  - Signal account keys/credentials (managed by `signal-cli`).
  - Bot state: contacts, messages, subscriptions, dedupe.
  - Database tables: users, topics, notifications, preferences, summaries, tool history.

## Data model (MVP intent)

### SignalIdentity

Stable identifier used to route messages.

Fields (conceptual):

- `id` (Signal address or stable hash)
- `display_name` (optional)
- `created_at`

### Subscription

Maps an identity to a region (and optional topics).

Example:

- `identity`: SignalIdentity
- `region`: "Iran"
- `topics`: ["censorship", "shutdowns"]
- `created_at`

### User/Topic/Notification (SQLite)

The `database` crate models subscriptions as topics:

- `User` (id is a stable Signal UUID or address, name, language)
- `Topic` (slug, e.g., `iran`, `vpn+iran`)
- `Notification` (topic_slug + user_id, created_at)

### Conversation memory (SQLite)

Durable memory tables used by the orchestrator:

- `Preference` (history_key, preference, updated_at)
- `ConversationSummary` (history_key, summary, message_count, updated_at)
- `ToolHistoryEntry` (history_key, tool_name, success, content, sender_id, group_id, created_at)
- `ClearContextEvent` (history_key, sender_id, created_at)

The orchestrator builds a `MemorySnapshot` from these tables (summary + tool history + clear-context
events) and attaches a standardized memory prompt to routing metadata. Clear-context events gate
summaries and tool history so old context is not rehydrated after a reset. Maple/Grok inject the
memory prompt as a system message and refresh it on each request (capped by provider limits).
Optional compaction runs on a timer (`AMAN_MEMORY_COMPACT_INTERVAL_SECS`) to prune TTLs and row caps.

### RegionEvent

Minimal schema for alerts.

- `region`
- `kind` (vpn_block, throttling, outage, advisory)
- `severity` (info, warn, urgent)
- `confidence` (low, med, high)
- `summary`
- `source_refs` (optional)
- `ts`

### InboundAttachment (Signal)

Metadata for attachments captured from signal-cli.

- `content_type`
- `filename` (optional)
- `file_path` (optional, local path to attachment file)
- `size` (optional)
- `width` / `height` (optional, for images/videos)

### ToolRequest / ToolResult (Brain tools)

When tool calling is enabled, brains can request external actions via a
`ToolExecutor` implementation.

- `ToolRequest`: `id`, `name`, `arguments` (JSON object)
- `ToolRequestMeta`: optional sender/group metadata for policy and logging
- `ToolResult`: `tool_call_id`, `content`, `success`

## State machine (onboarding + subscriptions)

States:

- `NewContact`
- `AwaitingRegionOptIn`
- `Subscribed(region)`
- `OptedOut`

Transitions:

- `NewContact` -> `AwaitingRegionOptIn` after first inbound message.
- `AwaitingRegionOptIn`:
  - User says "no" or "stop" -> `OptedOut`.
  - User provides a region -> `Subscribed(region)`.
- `Subscribed(region)`:
  - User says "stop" or "unsubscribe" -> `OptedOut`.
  - User provides a new region -> `Subscribed(new_region)`.

Region parsing:

- Accept common region names (Iran, Syria, Lebanon, etc.).
- Normalize basic aliases ("IR" -> "Iran") when known.
- Unknown or ambiguous inputs should trigger a clarifying question (TODO if not yet implemented).

## Flows

### Message flow (chat)

1. Signal -> signal-cli daemon receives inbound message.
2. `message_listener` subscribes to SSE via `signal-daemon`.
3. `message_listener` emits normalized `InboundMessage`.
4. `agent_brain` decides:
   - onboarding step, or
   - normal chat response.
5. `agent_brain` produces `OutboundMessage`.
6. `broadcaster` sends via `signal-daemon` to signal-cli daemon.

### MessageProcessor flow (optional)

1. Signal -> `message_listener` receives inbound envelope.
2. `MessageProcessor` converts to `InboundMessage` (including attachments metadata).
3. `MessageProcessor` calls a `Brain` implementation (mock or MapleBrain) with timeout.
4. `MessageProcessor` sends `OutboundMessage` via `signal-daemon`.

Note: Attachment-only messages are processed by default (`process_attachment_only: true`).
If `OutboundMessage.styles` is set, the listener sends styled text ranges to Signal.

### Orchestrator flow (recommended)

1. Signal -> `message_listener` receives inbound envelope.
2. `Orchestrator` starts typing indicator.
3. `Router` (using maple-brain in TEE) classifies the message and returns a `RoutingPlan`.
4. `Orchestrator` executes actions in sequence:
   - `Search`: Calls grok-brain for real-time search, sends status message.
   - `ClearContext`: Clears conversation history for sender.
   - `UseTool`: Runs an `agent-tools` capability (weather, calculator, etc.) and adds results to context.
   - `SetPreference`: Updates sender preference for privacy vs speed.
   - `AskPrivacyChoice`: Prompts the user when PII is detected (sanitize vs private vs cancel).
   - `PrivacyChoiceResponse`: Handles the user's PII choice and routes accordingly.
   - `Grok`/`Maple`: Direct query routing when explicitly requested.
   - `Respond`: Generates final response using Maple or Grok with accumulated context, then formats with a footer.
   - `Help`: Displays help text.
5. `Orchestrator` stops typing indicator.
6. `Orchestrator` returns final response for delivery (optionally with text styles).

**Sensitivity Classification:**
- **Sensitive** → Routes to Maple (health, finances, legal, personal, controversial)
- **Insensitive** → Can use Grok (weather, news, coding, entertainment, how-to)
- **Uncertain** → Follows user preference or defaults to Maple

If PII is detected, the router can request an explicit privacy choice before responding.

**Task Hints for Model Selection:**
- `general` - Standard conversations (llama-3.3-70b)
- `coding` - Programming and technical (deepseek-r1-0528)
- `math` - Mathematical reasoning (deepseek-r1-0528)
- `creative` - Stories, poems, brainstorming (gpt-oss-120b)
- `multilingual` - Non-English or translation (qwen2-5-72b)
- `quick` - Yes/no, simple lookups (mistral-small-3-1-24b)
- `vision` - Image analysis (qwen3-vl-30b, forces Maple)
- `about_bot` - Questions about Aman itself

### Tool execution flow (optional)

1. MapleBrain receives a request that needs real-time information.
2. MapleBrain calls `realtime_search` with a sanitized query.
3. `GrokToolExecutor` fetches results from xAI and returns them.
4. MapleBrain synthesizes the final response for Signal.

### Tool execution flow (orchestrator)

1. Router emits a `use_tool` action with name + arguments.
2. `Orchestrator` executes the tool via `agent-tools` registry.
3. Tool output is appended to context (`[TOOL RESULTS]`).
4. `Respond` action uses the augmented message for the final reply.
5. Response footer lists tools used and the selected model.

### Event flow (notifications)

1. `regional_event_listener` observes an event for a region.
2. Normalizes to `RegionEvent`.
3. `agent_brain` queries subscription store.
4. `agent_brain` creates outbound alert messages.
5. `broadcaster` delivers to subscribed identities.

### Web UI flow (current)

1. Browser -> Next.js app in `web/`.
2. `/api/chat` streams responses from the OpenAI-compatible API (local `api` service or the hosted worker via `AMAN_API_BASE_URL`).

### OpenAI-compatible API flow (current)

1. Web UI or client -> `api` service.
2. `api` selects a mode (`echo`, `orchestrator`, or `openrouter`) via `AMAN_API_MODE`.
3. Optional headers `X-Aman-User` / `X-Aman-Group` scope memory in orchestrator mode;
   `X-Aman-User` is forwarded as the OpenRouter `user` identifier in openrouter mode.
4. If `AMAN_KB_PATH` or `NOSTR_DB_PATH` is set, the API injects a KB snippet into the system context.
5. `api` returns OpenAI-style chat completions (streaming or non-streaming).

### Worker gateway flow (current)

1. Web client -> `workers/aman-gateway` `/v1/chat/completions`.
2. Worker loads KV memory (`AMAN_MEMORY`) and D1 KB (`AMAN_KB`).
3. Scheduled cron sync pulls Nostr DocManifest/ChunkRef events into D1.
4. Worker injects KB context into the system prompt; if KB is present, memory injection is skipped.
5. Worker returns OpenAI-style chat completions (streaming or non-streaming).

### Admin web flow

1. Operator opens `admin-web` UI.
2. Dashboard reads stats from SQLite via `database` queries.
3. Broadcast sends a message to topic subscribers via `broadcaster`.

### Nostr ingestion flow (current)

1. `ingester` chunks files and writes chunk blobs to disk.
2. `ingester` publishes DocManifest + ChunkRef events (or indexes directly into SQLite).
3. `nostr-indexer` stores Nostr events (docs + memory) in `NOSTR_DB_PATH`.
4. `nostr-rehydrate-memory` projects memory events into the runtime SQLite DB.
5. `api` reads from `NOSTR_DB_PATH` for knowledge base answers.
6. `aman-gateway-worker` cron sync pulls DocManifest/ChunkRef events into D1 for web retrieval.

Note: when built with `--features nostr`, the orchestrator also publishes memory
events (preferences, summaries, tool history, clear-context) to the configured relays.

## Reliability

- Deduplicate inbound messages using (message_id, timestamp window).
- Store inbound before processing to avoid double replies after restarts.
- Alerts are at-least-once; de-dupe per (event_id, identity).
- Retry send failures with exponential backoff.
- Use a queue so inference latency never blocks receiving.

## Configuration

Environment variables (names may be implementation-specific):

- `AMAN_NUMBER`: Signal account in E.164 format.
- `SIGNAL_CLI_JAR`: path to `signal-cli.jar`.
- `HTTP_ADDR`: HTTP bind address for signal-cli daemon.
- `SQLITE_PATH`: bot state database path.
- `AMAN_MEMORY_SUMMARY_MAX_ENTRIES`: max exchanges in rolling summary (default: 8).
- `AMAN_MEMORY_SUMMARY_MAX_ENTRY_CHARS`: max chars per summary line (default: 160).
- `AMAN_MEMORY_SUMMARY_MAX_CHARS`: max summary length (default: 1200).
- `AMAN_MEMORY_TOOL_OUTPUT_MAX_CHARS`: max stored tool output length (default: 2000).
- `AMAN_MEMORY_SUMMARY_TTL_DAYS`: summary TTL in days (0 disables).
- `AMAN_MEMORY_TOOL_TTL_DAYS`: tool history TTL in days (0 disables).
- `AMAN_MEMORY_CLEAR_TTL_DAYS`: clear-context TTL in days (0 disables).
- `AMAN_MEMORY_MAX_SUMMARIES`: max summary rows (0 disables).
- `AMAN_MEMORY_MAX_TOOL_HISTORY`: max tool history rows (0 disables).
- `AMAN_MEMORY_MAX_TOOL_HISTORY_PER_KEY`: max tool rows per sender/group (0 disables).
- `AMAN_MEMORY_MAX_CLEAR_EVENTS`: max clear-context rows (0 disables).
- `AMAN_MEMORY_COMPACT_INTERVAL_SECS`: background compaction interval in seconds (0 disables).
- `AMAN_MEMORY_PROMPT_MAX_CHARS`: max characters for injected memory prompt (0 disables).
- `AMAN_MEMORY_PROMPT_MAX_TOKENS`: approximate token cap for memory prompt (converted to chars).
- `AMAN_MEMORY_PROMPT_MAX_SUMMARY_CHARS`: max summary chars included in memory prompt.
- `AMAN_MEMORY_PROMPT_MAX_TOOL_ENTRIES`: max tool history entries included.
- `AMAN_MEMORY_PROMPT_MAX_TOOL_ENTRY_CHARS`: max characters per tool entry.
- `AMAN_MEMORY_PROMPT_MAX_CLEAR_EVENTS`: max clear-context events included.
- `AMAN_MEMORY_PROMPT_INCLUDE_SUMMARY`: include summary section in memory prompt.
- `AMAN_MEMORY_PROMPT_INCLUDE_TOOL_HISTORY`: include tool history section.
- `AMAN_MEMORY_PROMPT_INCLUDE_CLEAR_CONTEXT`: include clear-context section.
- `AMAN_MEMORY_PROMPT_PII_POLICY`: PII handling (`allow`, `redact`, `skip`).
- `AMAN_MEMORY_PROMPT_OVERRIDES`: JSON map of per-history prompt policy overrides.
- `AMAN_DEFAULT_LANGUAGE`: default language label for new contacts.
- `SIGNAL_DAEMON_URL`: base URL for signal-cli daemon (optional override).
- `SIGNAL_DAEMON_ACCOUNT`: account selector for daemon multi-account mode (optional).
- `OPENAI_API_KEY`: API key for an OpenAI-compatible provider (if used by `agent_brain`).
- `MODEL`: model name (example: `gpt-5`).
- `STORE_OPENAI_RESPONSES`: `true` or `false`.
- `MAPLE_API_KEY`: OpenSecret API key for `maple-brain`.
- `MAPLE_API_URL`: optional API URL override (default: `https://enclave.trymaple.ai`).
- `MAPLE_MODEL`: text model name for MapleBrain.
- `MAPLE_VISION_MODEL`: vision model name for MapleBrain.
- `MAPLE_SYSTEM_PROMPT`: optional system prompt override.
- `MAPLE_PROMPT_FILE`: path to prompt file (default: `SYSTEM_PROMPT.md`).
- `MAPLE_MAX_TOKENS`: max tokens for MapleBrain responses.
- `MAPLE_TEMPERATURE`: temperature for MapleBrain responses.
- `MAPLE_MAX_HISTORY_TURNS`: per-sender history length.
- `MAPLE_MAX_TOOL_ROUNDS`: max tool execution rounds per request (default: 2).
- `MAPLE_MEMORY_PROMPT_MAX_CHARS`: max memory prompt characters (0 disables).
- `MAPLE_MEMORY_PROMPT_MAX_TOKENS`: approximate token cap for memory prompt.
- `ROUTER_SYSTEM_PROMPT`: optional router prompt override for the orchestrator.
- `ROUTER_PROMPT_FILE`: router prompt file path (default: `ROUTER_PROMPT.md`).
- `GROK_API_KEY`: xAI API key for `grok-brain` / `GrokToolExecutor`.
- `GROK_API_URL`: optional API URL override (default: `https://api.x.ai`).
- `GROK_MODEL`: Grok model name (default: `grok-4-1-fast`).
- `GROK_SYSTEM_PROMPT`: optional system prompt override.
- `GROK_PROMPT_FILE`: path to prompt file (default: `SYSTEM_PROMPT.md`).
- `GROK_MAX_TOKENS`: max tokens for GrokBrain responses.
- `GROK_TEMPERATURE`: temperature for GrokBrain responses.
- `GROK_MAX_HISTORY_TURNS`: per-sender history length.
- `GROK_ENABLE_X_SEARCH`: enable X Search tool.
- `GROK_ENABLE_WEB_SEARCH`: enable Web Search tool.
- `GROK_MEMORY_PROMPT_MAX_CHARS`: max memory prompt characters (0 disables).
- `GROK_MEMORY_PROMPT_MAX_TOKENS`: approximate token cap for memory prompt.
- `REGION_POLL_INTERVAL_SECONDS`: event ingester cadence.
- `LOG_LEVEL`: log verbosity.
- `AMAN_API_ADDR`: bind address for the OpenAI-compatible gateway (api crate).
- `AMAN_API_TOKEN`: bearer token for API access (optional).
- `AMAN_API_MODEL`: default model name for the gateway.
- `AMAN_API_MODE`: API mode (`echo`, `orchestrator`, `openrouter`).
- `AMAN_KB_PATH`: optional path to a local knowledge base directory/file for the gateway.
- `ADMIN_ADDR`: bind address for the admin web UI (admin-web crate).
- `OPENROUTER_API_KEY`: API key for OpenRouter (optional API gateway mode).
- `OPENROUTER_API_URL`: OpenRouter API base URL (default: `https://openrouter.ai/api/v1`).
- `OPENROUTER_MODEL`: default OpenRouter model if the request omits `model`.
- `OPENROUTER_HTTP_REFERER`: optional app URL header for OpenRouter.
- `OPENROUTER_X_TITLE`: optional app title header for OpenRouter.
- `PHOENIXD_URL`: Phoenixd base URL for donation wallet (optional).
- `PHOENIXD_PASSWORD`: Phoenixd password for donation wallet (optional).
- `NWC_URI`: Nostr Wallet Connect URI for donation wallet (optional).
- `STRIKE_API_KEY`: Strike API key for donation wallet (optional).
- `NOSTR_RELAYS`: comma-separated relay URLs (memory publishing + indexer; worker default uses damus + nos.lol + nexus).
- `NOSTR_KB_AUTHOR`: optional pubkey filter for worker KB sync.
- `NOSTR_DB_PATH`: SQLite path for Nostr indexer and memory rehydration.
- `NOSTR_SECRETBOX_KEY`: optional symmetric key for payload encryption.
- `NOSTR_SECRET_KEY`: secret key used by publishers (`ingester`, memory events).
- `DEFAULT_MODEL`: worker default OpenRouter model (default: `x-ai/grok-4.1-fast`).
- `SUMMARY_MODEL`: worker summary model (default: `openai/gpt-5-nano`).
- `SYSTEM_PROMPT`: worker system prompt (includes KB-only guidance).
- `MEMORY_MAX_CHARS`: worker memory prompt cap.
- `MEMORY_SUMMARIZE_EVERY_TURNS`: worker summary cadence.
- `ALLOW_ANON`: allow unauthenticated worker requests (`true`/`false`).
- `WORKER_API_TOKEN`: bearer token when `ALLOW_ANON=false`.
- `RATE_LIMIT_MAX`: worker fixed-window request cap.
- `RATE_LIMIT_WINDOW_SECS`: worker rate-limit window (seconds).
- `KB_SYNC_LOOKBACK_SECS`: worker KB sync lookback window (seconds).
- `KB_MAX_SNIPPET_CHARS`: max chars per KB snippet (worker).
- `KB_MAX_TOTAL_CHARS`: max chars for total KB injection (worker).
- `KB_MAX_HITS`: max KB hits injected per request (worker).

For daemon setup details, see `docs/signal-cli-daemon.md`.

Attachment paths are resolved relative to the signal-cli data directory
(default `$XDG_DATA_HOME/signal-cli` or `$HOME/.local/share/signal-cli`). If you
run signal-cli with a custom `--config` path, ensure your services configure
the matching data directory (e.g., via `DaemonConfig::with_data_dir`).

## Privacy architecture

Aman implements a three-layer privacy model:

### Transport Layer (Signal)
- End-to-end encryption for all messages
- Trusted contact verification
- Disappearing messages support

### Processing Layer (Maple TEE)
- Messages processed in secure enclave (OpenSecret)
- Attestation handshake verifies enclave integrity
- Original content never leaves TEE for tool queries
- Per-sender conversation isolation

### Tool Layer (Privacy Boundary)
- Tool executors receive only AI-crafted sanitized queries
- Grok (for search) sees AI-generated queries, not user messages
- PII detection before any external processing

### Privacy Decision Flow

```
User Message
     ↓
Router (in Maple TEE)
     ├─ Detects PII? → AskPrivacyChoice prompt
     │      ├─ Sanitize → sanitize tool (Maple) → Grok (fast mode)
     │      ├─ Private → Maple only
     │      └─ Cancel → stop processing
     ├─ Sensitive topic? → Route to Maple only
     └─ Insensitive? → May use Grok (per preference)
     ↓
If tool needed (search):
     ├─ AI crafts sanitized query (in TEE)
     ├─ Only query sent to Grok
     └─ Results returned to TEE for response
```

### PII Detection

The router flags PII via `has_pii` and `pii_types` in `respond` actions, or emits
an explicit `ask_privacy_choice` action. PII types include:

- name, phone, email, ssn
- card, account, address, dob
- medical, income, financial, id

Note: the sanitize tool exists but full orchestration wiring is still pending.

## Safety posture

- Opt-in notifications only.
- Support "stop" / "unsubscribe" everywhere.
- Minimal retention: store only what is needed for dedupe and context.
- Do not log message bodies by default.
- Prefer retention-disabled settings (e.g., `store: false`) when supported by your provider.
- Admin web should be bound to localhost or placed behind authentication.
- Tool executors should receive only sanitized queries (no raw user text).

## Future architecture (RAG and Nostr)

Planned additions beyond the Signal MVP:

- RAG pipeline integrated into `agent_brain`.
- Extend `ingester` crate for documents and YouTube transcripts.
- Local vector DB (Qdrant, FAISS, or equivalent) rebuilt from Nostr events.

Already implemented:

- Nostr relay integration for durable doc/chunk metadata and memory events.
- `nostr-persistence` crate to publish/index Nostr events into SQLite, plus `nostr-rehydrate-memory`.
- `aman-gateway-worker` scheduled sync of doc/chunk events into D1 with KB prompt injection.

### Data model (Nostr durability + planned RAG)

- DocManifest event (implemented)
  - `doc_id`, `title`, `lang`, `mime`, `source_type`
  - `content_hash`, `blob_ref`, timestamps
  - inline `chunks` list (id, ord, offsets, chunk_hash, blob_ref)
- ChunkRef event (implemented)
  - `chunk_id`, `doc_id`, `ord`, offsets
  - `chunk_hash`, `blob_ref`, optional inline `text`, timestamps
- Memory events (implemented)
  - Preferences, summaries, tool history, clear-context (see `docs/NOSTR_MEMORY_SCHEMA.md`)
- Embedding artifact
  - model name/version
  - vector bytes (compressed) or reference
  - checksum
- Access policy and provenance events
  - who can read/share/export
  - audit history and signatures

### Storage split (RAG)

- Nostr stores metadata, hashes, and access policy events.
- Large blobs live in object storage or IPFS with references in Nostr.
- Vector search happens locally; indexes are rebuilt from the relay log.

### Nostr persistence implementation (current)

- Event kinds (parameterized replaceable, 30000-39999):
  - DocManifest: 30090 (d=doc_id)
  - ChunkRef: 30091 (d=chunk_id)
  - AccessPolicy: 30092 (d=scope_id)
  - AmanPreference: 30093 (d=history_key:preference)
  - AmanSummary: 30094 (d=history_key:summary)
  - AmanToolHistoryEntry: 30095 (d=history_key:hash)
  - AmanClearContextEvent: 30096 (d=history_key:hash)
- Required tags:
  - d tag for addressability
  - k tag with semantic label (doc_manifest, chunk_ref, policy)
  - enc tag when content is encrypted (secretbox-v1)
- Content format:
  - JSON when unencrypted
  - base64 ciphertext when encrypted
- ChunkRef payloads can include inline `text` for worker ingestion (`ingester --inline-text`).
- Relay retention varies by operator (see NIP-11). Choose relays that retain custom kinds.
- Implementation uses rust-nostr (`nostr-sdk`).

### JSON schema (authoritative)

DocManifest content:

```json
{
  "schema_version": 1,
  "created_at": 1735689600,
  "updated_at": 1735689600,
  "doc_id": "doc_iran_connectivity_001",
  "title": "Connectivity Disruption Summary",
  "lang": "en",
  "mime": "text/plain",
  "source_type": "signal_paste",
  "content_hash": "sha256:...",
  "blob_ref": "s3://...",
  "chunks": [
    {
      "chunk_id": "chunk_iran_001",
      "ord": 0,
      "offsets": { "start": 0, "end": 512 },
      "chunk_hash": "sha256:...",
      "blob_ref": "s3://..."
    }
  ]
}
```

ChunkRef content:

```json
{
  "schema_version": 1,
  "created_at": 1735689600,
  "updated_at": 1735689600,
  "chunk_id": "chunk_iran_001",
  "doc_id": "doc_iran_connectivity_001",
  "ord": 0,
  "offsets": { "start": 0, "end": 512 },
  "chunk_hash": "sha256:...",
  "blob_ref": "s3://...",
  "text": "Optional inline snippet text (worker-friendly)"
}
```

AccessPolicy content:

```json
{
  "schema_version": 1,
  "created_at": 1735689600,
  "updated_at": 1735689600,
  "scope_id": "workspace_01",
  "readers": ["npub..."],
  "notes": "Optional human-readable policy notes"
}
```

## Glossary

- **SignalIdentity**: stable identifier for a Signal contact.
- **Region**: geopolitical region label used for subscriptions.
- **RegionEvent**: normalized alert event for a region.
- **Subscription**: mapping from identity to region/topics.
- **Broadcaster**: component that sends outbound Signal messages.
- **signal-cli daemon**: signal-cli process exposing HTTP/SSE and JSON-RPC.
- **signal-daemon**: Rust client for the signal-cli daemon.
- **api**: OpenAI-compatible inference gateway (echo/orchestrator/OpenRouter + KB injection).
- **database**: SQLite persistence crate for users, topics, and notifications.
- **nostr-persistence**: crate that publishes and indexes Nostr metadata into SQLite.
- **ingester**: chunks documents and publishes/indexes Nostr events.
- **mock-brain**: test harness crate for message flow and signal-daemon integration.
- **brain-core**: shared Brain trait and message types for AI backends.
- **maple-brain**: OpenSecret-backed Brain implementation.
- **admin-web**: admin dashboard and broadcast UI for operators.
- **grok-brain**: xAI Grok Brain and GrokToolExecutor implementations.
- **orchestrator**: message routing and action plan execution coordinator.
- **ToolExecutor**: interface for executing external tools (e.g., real-time search).
- **RoutingPlan**: list of actions (search, use_tool, clear_context, respond, grok, maple, help, skip, ignore, ask_privacy_choice, privacy_choice_response, set_preference) to execute.
- **agent-tools**: extensible tool registry with 11 built-in tools (Calculator, Weather, WebFetch, Dictionary, WorldTime, BitcoinPrice, CryptoPrice, CurrencyConverter, UnitConverter, RandomNumber, Sanitize).
- **TextStyle**: formatting range for Signal messages (bold, italic, monospace, strikethrough).
- **FormattedMessage**: response payload with plain text plus optional `TextStyle` ranges.
- **ModelSelector**: component that chooses optimal model based on task hints (general, coding, math, creative, multilingual, quick, vision, about_bot).
- **PreferenceStore**: per-user storage for agent preferences (prefer_speed, prefer_privacy, default).
- **ConversationHistory**: per-sender message history with auto-trimming (in brain-core).
- **MemorySnapshot**: durable memory payload (summary, tool history, clear-context events).
- **MemoryPromptPolicy**: controls how memory is formatted and injected into prompts.
- **DocManifest**: planned event describing a document and its chunks.
- **Chunk**: planned unit of text for retrieval and citations.
- **Embedding artifact**: planned vector or reference for retrieval.

## Security notes

- `signal-cli` stores account keys and credentials on disk (typically under
  `$XDG_DATA_HOME/signal-cli/data/` or `$HOME/.local/share/signal-cli/data/`).
  Protect this path with strict permissions and backups.
- Signal is end-to-end encrypted to the server; the server is the endpoint.
  Treat it as a trusted boundary and minimize stored data.

---

## See Also

- [Main README](../README.md) - Project overview and quick start
- [CLAUDE.md](../CLAUDE.md) - Developer guidance and environment setup
- [Crates Index](../crates/README.md) - Overview of all Rust crates
- [Adding Tools](ADDING_TOOLS.md) - How to add new agent-tools capabilities
- [Adding Actions](ADDING_ACTIONS.md) - How to add new orchestrator actions
- [signal-cli Daemon](signal-cli-daemon.md) - Daemon API documentation
- [ROADMAP.md](../ROADMAP.md) - Planned features and phases
