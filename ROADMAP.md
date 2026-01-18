# Aman Roadmap

This roadmap captures what still needs to be built beyond the current Signal MVP. It separates
MVP hardening, orchestrator/brain integration, and the longer arc toward RAG + Nostr-based
persistence and rehydration.

## Current status

- signal-cli daemon workflows and helper scripts are in place.
- `signal-daemon`, `message-listener`, and `broadcaster` are working as libraries.
- `brain-core` defines shared Brain/ToolExecutor traits, routing metadata, prompt hashing, and tool request metadata.
- `maple-brain` provides an OpenSecret-backed Brain with vision + tool calling and per-request model overrides.
- `grok-brain` provides a Grok-backed Brain and ToolExecutor with per-request model overrides.
- `orchestrator` routes messages, executes action plans, attaches routing metadata, and tracks preferences.
- `agent-tools` ships a tool registry plus a ToolExecutor adapter with allowlists, rate limits, timeouts, and caching.
- SQLite persistence now covers preferences, rolling summaries, tool history, and clear-context events (when configured).
- `agent-brain` implements onboarding and subscription routing; ships `agent_brain_bot` and `region_event_send`.
- `regional_event_listener` exists as a documented subsystem; intake wiring is still pending.
- `nostr-persistence` crate is started (publisher/indexer foundation).
- `database` crate provides SQLx migrations and async CRUD helpers.
- `web/` Next.js UI exists for browser chat (not yet wired to Signal services).
- `api` crate provides an OpenAI-compatible inference gateway (stubbed echo).
- `ingester` crate exists for chunking files and publishing/indexing Nostr events.
- `admin-web` crate provides a dashboard + broadcast UI (auth still needed).

## Phase 0 - Signal MVP hardening + orchestrator adoption (in progress)

Goal: make the orchestrator the default Signal message flow and close MVP reliability gaps.

- Wire `message-listener` -> `orchestrator` as the default bot path (new service binary).
- Integrate `agent-brain` onboarding with orchestrator responses (handoff when state machine needs AI).
- Add safer routing fallbacks (fail-closed to Maple; detect image attachments without router output).
- Implement dedupe and idempotent send logic for inbound/outbound message delivery.
- Persist preferences and minimal conversation metadata in SQLite.
- Add structured health checks and minimal logging defaults for production.

## Phase 1 - Orchestrator + Brain interface alignment (complete)

Goal: unify how routing signals, task hints, and model selection flow across brains.

- Per-request model override support in `maple-brain` and `grok-brain`.
- Routing metadata (sensitivity, task_hint, attachment info) promoted into `brain-core` message types.
- Router/system prompt hashing for reproducibility.
- Unit/integration tests for routing plans, preferences, and vision routing.

## Phase 2 - Tooling and action framework expansion (complete)

Goal: consolidate tool calling across orchestrator and LLM tool calls.

- `agent-tools` registry bridged into `brain-core::ToolExecutor`.
- Policy controls: allowlist per sender/group, rate limits, and timeouts.
- Tool caching and structured tool result formatting.
- Tool surface expanded for alerts, knowledge lookup, and admin workflows (partial).

## Phase 3 - Durable memory and preference persistence (complete)

Goal: move beyond in-memory context and align memory across Maple/Grok.

- SQLite tables for preferences, conversation summaries, tool history, and clear-context events.
- Rolling summary jobs that compress context into short memory blobs.
- Retention policy config (per-sender TTL + global caps).
- Shared history keys across direct and group messages.

## Phase 4 - Brain memory sync + shared context (next)

Goal: make Maple/Grok consume the same durable memory view.

- Add a `brain-core` MemoryStore trait with adapters for SQLite.
- Use durable summaries as prompt context for Maple/Grok responses.
- Hydrate per-sender memory into Maple/Grok history windows on cold start.
- Respect clear-context events across brains and cached tool outputs.
- Add background compaction jobs for summaries and tool history.

## Phase 5 - Nostr persistence schema + encryption

Goal: make memory portable and cryptographically bound.

- Finalize Nostr event schema for summaries, tool history, and preference updates.
- Encrypt sensitive payloads; publish encrypted references + hashes.
- Define provenance/policy events for memory updates and deletions.
- Map SQLite memory rows to Nostr events with deterministic IDs.

## Phase 6 - Nostr sync + rehydration

Goal: rebuild local memory from Nostr events and keep nodes in sync.

- Relay publishing + local indexer rehydration for memory tables.
- Reconcile conflicts between local SQLite and relay event streams.
- Store large blobs in object storage/IPFS with Nostr refs + hashes.
- Add replay/backup tooling for disaster recovery.

## Phase 7 - RAG pipeline + retrieval

Goal: retrieval-augmented responses with citations over Signal.

- Extend `ingester` for document + YouTube ingestion with embeddings.
- Add local vector DB (Qdrant/FAISS) + reranking.
- Expose retrieval as a tool in both orchestrator and `brain-core` tool calls.
- Deliver short, Signal-friendly citations/snippets in responses.

## Phase 8 - Safety, policy, and high-risk workflows

Goal: enforce strong privacy defaults for closed-society contexts.

- Metadata minimization (EXIF stripping, redacted logs, user-agent/IP avoidance).
- Encryption boundaries for blobs and short-lived plaintext handling.
- Media sanitization helpers (face blur, audio distortion, background obfuscation).
- Policy enforcement for sensitive domains and high-risk escalation paths.

## Phase 9 - Multi-agent federation + governance

Goal: compose specialized brains and operators without losing policy control.

- Multi-brain routing with explicit policy gates per tool and domain.
- Operator review queues for high-risk messages + tool actions.
- Skill pack distribution with signed capability manifests.
- Regional deployment profiles (latency, relay selection, compliance).

## Decisions to lock early

- Router prompt versioning and how routing outputs are stored for audits.
- Unified tool interface (agent-tools vs brain-core) and permission model.
- Memory retention windows and default summarization strategy.
- What is stored on Nostr vs in object storage, plus event schema details.
- How citations/snippets are presented over Signal.

## Planned artifacts

- Orchestrator-first service binary that wires listener + routing + tools.
- Shared tool registry + policy layer for orchestrator + brain tool calls.
- DB schema for preferences, summaries, tool history, and dedupe.
- `crates/nostr-persistence` publisher/indexer with rehydration tooling.
- `crates/ingester` expansions for docs + YouTube.
- Vector DB schema and retrieval tool integration.
