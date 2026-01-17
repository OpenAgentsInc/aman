# Aman Signal MVP Architecture

## Goal

- Signal-native messaging experience.
- Opt-in regional alerts for activists and human-rights defenders.
- Core crates: `signal-daemon`, `message-listener`, `agent-brain`, `broadcaster`, `api`.
- Data persistence crate: `database` (SQLite via SQLx).
- Test harness crate: `mock-brain` (mock implementations for message flow testing).
- Regional event ingestion as a subsystem/service (documented under `agent_brain::regional_events`).
- For planned phases beyond the MVP, see `ROADMAP.md`.

## Components

- `signal-cli daemon` (process)
  - Runs the Signal account and exposes HTTP/SSE and JSON-RPC endpoints.
- Web UI (Next.js app in `web/`)
  - Browser-based chat surface with `/api/chat`.
  - Uses the OpenAI-compatible API directly; not yet wired to Signal services.
- `api` (crate: `crates/api`)
  - OpenAI-compatible inference gateway (`/v1/chat/completions`, `/v1/models`).
  - Currently returns stubbed echo responses for local/dev use.
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
- `database` (crate: `crates/database`)
  - SQLite persistence for users, topics, and notification subscriptions.
  - Runs migrations and exposes async CRUD helpers.
- `mock-brain` (crate: `crates/mock-brain`)
  - Mock brain implementations for testing message processing without an AI backend.
- `regional_event_listener` (subsystem)
  - Ingests regional events from external feeds or fixtures.
  - Normalizes to `RegionEvent` and hands off to `agent_brain`.
- Local storage
  - Signal account keys/credentials (managed by `signal-cli`).
  - Bot state: contacts, messages, subscriptions, dedupe.
  - Database tables: users, topics, notifications (via `database` crate).
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

### User/Topic/Notification (SQLite)

The `database` crate models subscriptions as topics:

- `User` (id is a stable Signal UUID or address, name, language)
- `Topic` (slug, e.g., `iran`, `vpn+iran`)
- `Notification` (topic_slug + user_id, created_at)

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

### Web UI flow (current)

1. Browser -> Next.js app in `web/`.
2. `/api/chat` streams responses from the OpenAI-compatible API.

### OpenAI-compatible API flow (current)

1. Web UI or client -> `api` service.
2. `api` returns OpenAI-style chat completions (stubbed echo).

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
- `AMAN_API_ADDR`: bind address for the OpenAI-compatible gateway (api crate).
- `AMAN_API_TOKEN`: bearer token for API access (optional).
- `AMAN_API_MODEL`: default model name for the gateway.
- `NOSTR_RELAYS`: comma-separated relay URLs (Phase 2).
- `NOSTR_DB_PATH`: SQLite path for Nostr indexer (Phase 2).
- `NOSTR_SECRETBOX_KEY`: optional symmetric key for payload encryption (Phase 2).

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
- **mock-brain**: test harness crate for message flow and signal-daemon integration.
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
