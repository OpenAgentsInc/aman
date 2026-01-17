# Aman Signal MVP Architecture

## Goal

- Signal-native messaging experience.
- Opt-in regional alerts for activists and human-rights defenders.
- Core crates: `signal-daemon`, `message-listener`, `agent-brain`, `broadcaster`.
- Regional event ingestion as a subsystem/service (documented under `agent_brain::regional_events`).
- For planned phases beyond the MVP, see `ROADMAP.md`.

## Components

- `signal-cli daemon` (process)
  - Runs the Signal account and exposes HTTP/SSE and JSON-RPC endpoints.
- `signal-daemon` (crate: `crates/signal-daemon`)
  - HTTP/SSE client for signal-cli daemon.
  - Shared dependency for inbound and outbound transport.
- `message_listener` (crate: `crates/message-listener`)
  - Owns Signal inbound transport via `signal-daemon` (HTTP/SSE).
  - Normalizes inbound messages into `InboundMessage` records.
  - Emits normalized events into the local queue/state store.
- `agent_brain` (crate: `crates/agent-brain`)
  - Owns message handling, onboarding state machine, and routing decisions.
  - Calls the OpenAI-compatible Responses API.
  - Decides when to respond vs. when to update subscription state.
- `broadcaster` (crate: `crates/broadcaster`)
  - Owns outbound delivery via `signal-daemon` (HTTP to signal-cli daemon).
  - Handles chunking, retries, and throttling.
- `regional_event_listener` (subsystem)
  - Ingests regional events from external feeds or fixtures.
  - Normalizes to `RegionEvent` and hands off to `agent_brain`.
- Local storage
  - Signal account keys/credentials (managed by `signal-cli`).
  - Bot state: contacts, messages, subscriptions, dedupe.
- OpenAI-compatible API endpoint
  - Responses API for text generation.

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

### RegionEvent

Minimal schema for alerts.

- `region`
- `kind` (vpn_block, throttling, outage, advisory)
- `severity` (info, warn, urgent)
- `confidence` (low, med, high)
- `summary`
- `source_refs` (optional)
- `ts`

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

### Event flow (notifications)

1. `regional_event_listener` observes an event for a region.
2. Normalizes to `RegionEvent`.
3. `agent_brain` queries subscription store.
4. `agent_brain` creates outbound alert messages.
5. `broadcaster` delivers to subscribed identities.

## Reliability

- Deduplicate inbound messages using (message_id, timestamp window).
- Store inbound before processing to avoid double replies after restarts.
- Alerts are at-least-once; de-dupe per (event_id, identity).
- Retry send failures with exponential backoff.
- Use a queue so OpenAI latency never blocks receiving.

## Configuration

Environment variables (names may be implementation-specific):

- `AMAN_NUMBER`: Signal account in E.164 format.
- `SIGNAL_CLI_JAR`: path to `signal-cli.jar`.
- `HTTP_ADDR`: HTTP bind address for signal-cli daemon.
- `SQLITE_PATH`: bot state database path.
- `OPENAI_API_KEY`: API key for OpenAI-compatible provider.
- `MODEL`: model name (example: `gpt-5`).
- `STORE_OPENAI_RESPONSES`: `true` or `false`.
- `REGION_POLL_INTERVAL_SECONDS`: event ingester cadence.
- `LOG_LEVEL`: log verbosity.

For daemon setup details, see `docs/signal-cli-daemon.md`.

## Safety posture

- Opt-in notifications only.
- Support "stop" / "unsubscribe" everywhere.
- Minimal retention: store only what is needed for dedupe and context.
- Do not log message bodies by default.
- Prefer `store: false` (or equivalent) for the OpenAI-compatible Responses API.

## Future architecture (RAG and Nostr)

Planned additions beyond the Signal MVP:

- RAG pipeline integrated into `agent_brain`.
- New `ingester` crate for documents and YouTube transcripts.
- Nostr relay integration for durable, syncable knowledge state.
- Local vector DB (Qdrant, FAISS, or equivalent) rebuilt from Nostr events.

### Planned data model (RAG + Nostr)

- DocManifest event
  - `doc_id`, `owner_id`, `created_at`, `language`, `mime`, `title`
  - `source_hash`, encryption metadata
  - list of chunk IDs
- Chunk event
  - `chunk_id`, `doc_id`, offsets
  - encrypted chunk text or pointer to encrypted blob
  - optional embedding reference
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

## Glossary

- **SignalIdentity**: stable identifier for a Signal contact.
- **Region**: geopolitical region label used for subscriptions.
- **RegionEvent**: normalized alert event for a region.
- **Subscription**: mapping from identity to region/topics.
- **Broadcaster**: component that sends outbound Signal messages.
- **signal-cli daemon**: signal-cli process exposing HTTP/SSE and JSON-RPC.
- **signal-daemon**: Rust client for the signal-cli daemon.
- **DocManifest**: planned event describing a document and its chunks.
- **Chunk**: planned unit of text for retrieval and citations.
- **Embedding artifact**: planned vector or reference for retrieval.

## Security notes

- `signal-cli` stores account keys and credentials on disk (typically under
  `$XDG_DATA_HOME/signal-cli/data/` or `$HOME/.local/share/signal-cli/data/`).
  Protect this path with strict permissions and backups.
- Signal is end-to-end encrypted to the server; the server is the endpoint.
  Treat it as a trusted boundary and minimize stored data.

## References (example docs)

[1]: https://platform.openai.com/docs/api-reference/responses?utm_source=chatgpt.com "Responses | OpenAI API Reference"
[2]: https://platform.openai.com/docs/guides/your-data?utm_source=chatgpt.com "Data controls in the OpenAI platform"
[3]: https://platform.openai.com/docs/guides/rate-limits?utm_source=chatgpt.com "Rate limits | OpenAI API"
