# Aman Signal MVP Architecture

## Goal

- Signal-native messaging experience.
- Opt-in regional alerts for activists and human-rights defenders.
- Core crates: `signal-daemon`, `message-listener`, `agent-brain`, `broadcaster`, `api`, `ingester`, `admin-web`, `grok-brain`, `orchestrator`, `agent-tools`.
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
- `api` (crate: `crates/api`)
  - OpenAI-compatible inference gateway (`/v1/chat/completions`, `/v1/models`).
  - Uses a local knowledge base (if configured) or stubbed echo responses for local/dev use.
- `ingester` (crate: `crates/ingester`)
  - Chunks local files into blob refs and publishes DocManifest + ChunkRef events.
  - Can index directly into a local Nostr SQLite DB for testing.
- `signal-daemon` (crate: `crates/signal-daemon`)
  - HTTP/SSE client for signal-cli daemon.
  - Shared dependency for inbound and outbound transport.
  - SSE auto-reconnection with configurable exponential backoff.
- `message_listener` (crate: `crates/message-listener`)
  - Owns Signal inbound transport via `signal-daemon` (HTTP/SSE).
  - Normalizes inbound messages into `InboundMessage` records (including attachments metadata).
  - Emits normalized events into the local queue/state store.
  - Configurable brain processing timeout (default: 60s) prevents pipeline hangs.
  - Graceful shutdown via `run_with_shutdown()` or `run_until_stopped()`.
  - Supports attachment-only messages (configurable).
- `brain-core` (crate: `crates/brain-core`)
  - Shared Brain trait, ToolExecutor trait, and message types for AI backends.
  - Defines attachments metadata (`InboundAttachment`) for inbound processing.
  - Provides `ConversationHistory` for per-sender message history with auto-trimming.
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
  - Executes multi-step actions: search, clear context, respond, help, tool usage.
  - Supports direct Grok/Maple commands and per-sender preference switching.
  - Attaches task hints for model selection and enforces vision-only routing to Maple.
  - Coordinates maple-brain (for routing and responses) and grok-brain (for search).
  - Maintains typing indicators and sends interim status messages.
  - Persists preferences, rolling summaries, and tool history in SQLite (when configured).
- `agent-tools` (crate: `crates/agent-tools`)
  - Tool registry and implementations for orchestrator-level capabilities.
  - Built-in tools: calculator, weather, web fetch + summarize, dictionary, world time, crypto, currency.
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

### Orchestrator flow (recommended)

1. Signal -> `message_listener` receives inbound envelope.
2. `Orchestrator` starts typing indicator.
3. `Router` (using maple-brain in TEE) classifies the message and returns a `RoutingPlan`.
4. `Orchestrator` executes actions in sequence:
   - `Search`: Calls grok-brain for real-time search, sends status message.
   - `ClearContext`: Clears conversation history for sender.
   - `UseTool`: Runs an `agent-tools` capability (weather, calculator, etc.) and adds results to context.
   - `SetPreference`: Updates sender preference for privacy vs speed.
   - `Grok`/`Maple`: Direct query routing when explicitly requested.
   - `Respond`: Generates final response using Maple or Grok with accumulated context.
   - `Help`: Displays help text.
5. `Orchestrator` stops typing indicator.
6. `Orchestrator` returns final response for delivery.

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

### Event flow (notifications)

1. `regional_event_listener` observes an event for a region.
2. Normalizes to `RegionEvent`.
3. `agent_brain` queries subscription store.
4. `agent_brain` creates outbound alert messages.
5. `broadcaster` delivers to subscribed identities.

### Web UI flow (current)

1. Browser -> Next.js app in `web/`.
2. `/api/chat` streams responses from the OpenAI-compatible API.

### OpenAI-compatible API flow (current)

1. Web UI or client -> `api` service.
2. `api` returns OpenAI-style chat completions (stubbed echo).

### Admin web flow

1. Operator opens `admin-web` UI.
2. Dashboard reads stats from SQLite via `database` queries.
3. Broadcast sends a message to topic subscribers via `broadcaster`.

### Nostr ingestion flow (current)

1. `ingester` chunks files and writes chunk blobs to disk.
2. `ingester` publishes DocManifest + ChunkRef events (or indexes directly into SQLite).
3. `nostr-indexer` stores Nostr events in `NOSTR_DB_PATH`.
4. `api` reads from `NOSTR_DB_PATH` for knowledge base answers.

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
- `ROUTER_SYSTEM_PROMPT`: optional router prompt override for the orchestrator.
- `ROUTER_PROMPT_FILE`: router prompt file path (default: `ROUTER_PROMPT.md`).
- `GROK_API_KEY`: xAI API key for `grok-brain` / `GrokToolExecutor`.
- `GROK_API_URL`: optional API URL override (default: `https://api.x.ai`).
- `GROK_MODEL`: Grok model name (default: `grok-4-1-fast`).
- `GROK_SYSTEM_PROMPT`: optional system prompt override.
- `GROK_MAX_TOKENS`: max tokens for GrokBrain responses.
- `GROK_TEMPERATURE`: temperature for GrokBrain responses.
- `GROK_MAX_HISTORY_TURNS`: per-sender history length.
- `GROK_ENABLE_X_SEARCH`: enable X Search tool.
- `GROK_ENABLE_WEB_SEARCH`: enable Web Search tool.
- `REGION_POLL_INTERVAL_SECONDS`: event ingester cadence.
- `LOG_LEVEL`: log verbosity.
- `AMAN_API_ADDR`: bind address for the OpenAI-compatible gateway (api crate).
- `AMAN_API_TOKEN`: bearer token for API access (optional).
- `AMAN_API_MODEL`: default model name for the gateway.
- `AMAN_KB_PATH`: optional path to a local knowledge base directory/file for the gateway.
- `ADMIN_ADDR`: bind address for the admin web UI (admin-web crate).
- `NOSTR_RELAYS`: comma-separated relay URLs (Phase 2).
- `NOSTR_DB_PATH`: SQLite path for Nostr indexer (Phase 2).
- `NOSTR_SECRETBOX_KEY`: optional symmetric key for payload encryption (Phase 2).
- `NOSTR_SECRET_KEY`: secret key used by `ingester` when publishing to relays.

For daemon setup details, see `docs/signal-cli-daemon.md`.

Attachment paths are resolved relative to the signal-cli data directory
(default `$XDG_DATA_HOME/signal-cli` or `$HOME/.local/share/signal-cli`). If you
run signal-cli with a custom `--config` path, ensure your services configure
the matching data directory (e.g., via `DaemonConfig::with_data_dir`).

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
- Nostr relay integration for durable, syncable knowledge state.
- Local vector DB (Qdrant, FAISS, or equivalent) rebuilt from Nostr events.
- `nostr-persistence` crate to publish and index Nostr events into SQLite.

### Planned data model (RAG + Nostr)

- DocManifest event
  - `doc_id`, `title`, `lang`, `mime`, `source_type`
  - `content_hash`, `blob_ref`, timestamps
  - inline `chunks` list (id, ord, offsets, chunk_hash, blob_ref)
- ChunkRef event
  - `chunk_id`, `doc_id`, `ord`, offsets
  - `chunk_hash`, `blob_ref`, timestamps
- Embedding artifact
  - model name/version
  - vector bytes (compressed) or reference
  - checksum
- Access policy and provenance events
  - who can read/share/export
  - audit history and signatures

### Storage split (planned)

- Nostr stores metadata, hashes, and access policy events.
- Large blobs live in object storage or IPFS with references in Nostr.
- Vector search happens locally; indexes are rebuilt from the relay log.

### Nostr persistence implementation plan

- Event kinds (parameterized replaceable, 30000-39999):
  - DocManifest: 30090 (d=doc_id)
  - ChunkRef: 30091 (d=chunk_id)
  - AccessPolicy: 30092 (d=scope_id)
- Required tags:
  - d tag for addressability
  - k tag with semantic label (doc_manifest, chunk_ref, policy)
  - enc tag when content is encrypted (secretbox-v1)
- Content format:
  - JSON when unencrypted
  - base64 ciphertext when encrypted
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
  "blob_ref": "s3://..."
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
- **api**: OpenAI-compatible inference gateway (chat completions).
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
- **RoutingPlan**: list of actions (search, clear context, respond, show help) to execute.
- **ConversationHistory**: per-sender message history with auto-trimming (in brain-core).
- **DocManifest**: planned event describing a document and its chunks.
- **Chunk**: planned unit of text for retrieval and citations.
- **Embedding artifact**: planned vector or reference for retrieval.

## Security notes

- `signal-cli` stores account keys and credentials on disk (typically under
  `$XDG_DATA_HOME/signal-cli/data/` or `$HOME/.local/share/signal-cli/data/`).
  Protect this path with strict permissions and backups.
- Signal is end-to-end encrypted to the server; the server is the endpoint.
  Treat it as a trusted boundary and minimize stored data.
