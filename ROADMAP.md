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
- Tool surface now includes unit conversion, random numbers, and a Maple-backed PII sanitize tool.
- SQLite persistence now covers preferences, rolling summaries, tool history, and clear-context events (when configured).
- Orchestrator can detect PII, prompt for privacy choices, and format responses with metadata footers.
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

## Phase 4 - Brain memory contract + shared context framing (next)

Goal: give Maple/Grok a consistent memory contract with explicit safety gates.

- Add `brain-core::MemoryStore` + `MemorySnapshot` types (summaries, tool history, clear-context).
- Define a standard memory prompt format (short, stable, attribution-friendly).
- Add memory policy inputs (max tokens, TTL, PII handling, per-sender overrides).
- Hydrate per-sender memory into Maple/Grok on cold start (summary-first, history-last).
- Wire `clear_context` events through memory snapshots and brain history resets.

## Phase 5 - Brain memory hydration + durability wiring

Goal: actually use durable memory in live brain calls, not just in the orchestrator.

- Map SQLite summaries + tool history into brain prompts with strict size budgets.
- Implement per-request memory hydration for Maple/Grok with configurable templates.
- Add memory compaction jobs (summary rollups, tool history pruning, clear-context honoring).
- Include privacy-choice outcomes in memory (sanitized vs private).
- Track memory provenance in routing metadata for audits.

## Phase 6 - PII sanitization workflow end-to-end

Goal: complete the privacy-choice loop with actual sanitization and fast-mode fallback.

- Implement `sanitize` tool execution (Maple-only) when user chooses sanitize.
- Persist sanitized inputs alongside tool history (never store raw PII).
- Route sanitized requests to Grok and keep private requests on Maple.
- Add policy tests for PII detection, sanitize outputs, and tool-history storage.
- Provide operator metrics for PII prompts and user choices.

## Phase 7 - Nostr persistence schema + encryption

Goal: make memory portable and cryptographically bound.

- Finalize Nostr event schema for summaries, tool history, and preference updates.
- Encrypt sensitive payloads; publish encrypted references + hashes.
- Define provenance/policy events for memory updates and deletions.
- Map SQLite memory rows to Nostr events with deterministic IDs.
- Add key rotation + migration guidance for operators.

## Phase 8 - Nostr sync + rehydration

Goal: rebuild local memory from Nostr events and keep nodes in sync.

- Relay publishing + local indexer rehydration for memory tables.
- Reconcile conflicts between local SQLite and relay event streams.
- Store large blobs in object storage/IPFS with Nostr refs + hashes.
- Add replay/backup tooling for disaster recovery.
- Add audit tooling for memory provenance and tamper checks.

## Phase 9 - Brain rehydration from Nostr + shared retrieval

Goal: make Nostr-backed memory first-class in brain inference.

- Rehydrate Maple/Grok memory snapshots from Nostr at startup.
- Maintain local memory caches with Nostr sync checkpoints.
- Use Nostr-backed knowledge as retrieval context (citations + summaries).
- Keep privacy policies attached to memory payloads across sync.

## Phase 10 - RAG pipeline + retrieval

Goal: retrieval-augmented responses with citations over Signal.

- Extend `ingester` for document + YouTube ingestion with embeddings.
- Add local vector DB (Qdrant/FAISS) + reranking.
- Expose retrieval as a tool in both orchestrator and `brain-core` tool calls.
- Deliver short, Signal-friendly citations/snippets in responses.

## Phase 11 - Safety, policy, and high-risk workflows

Goal: enforce strong privacy defaults for closed-society contexts.

- Metadata minimization (EXIF stripping, redacted logs, user-agent/IP avoidance).
- Encryption boundaries for blobs and short-lived plaintext handling.
- Media sanitization helpers (face blur, audio distortion, background obfuscation).
- Policy enforcement for sensitive domains and high-risk escalation paths.

## Phase 12 - Multi-agent federation + governance

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
- `brain-core` MemoryStore + MemorySnapshot contract for shared memory.
- PII sanitize workflow (tool wiring + policy tests).
- DB schema for preferences, summaries, tool history, and dedupe.
- `crates/nostr-persistence` publisher/indexer with rehydration tooling.
- `crates/ingester` expansions for docs + YouTube.
- Vector DB schema and retrieval tool integration.
