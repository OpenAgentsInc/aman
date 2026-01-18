# Aman Roadmap

This roadmap captures what still needs to be built beyond the current Signal MVP. It separates MVP hardening,
orchestrator/brain integration, and the longer arc toward RAG + Nostr-based persistence.

## Current status

- signal-cli daemon workflows and helper scripts are in place.
- `signal-daemon`, `message-listener`, and `broadcaster` are working as libraries.
- `brain-core` defines the shared Brain/ToolExecutor traits and message types (incl. attachments).
- `maple-brain` provides an OpenSecret-backed Brain implementation with vision + tool calling support.
- `grok-brain` provides an xAI Grok-backed Brain and ToolExecutor for real-time search.
- `orchestrator` routes messages (Maple TEE), executes action plans, tracks preferences, and maintains typing indicators.
- `agent-tools` provides a tool registry with built-in utilities (weather, calculator, web fetch, etc.).
- `agent-brain` implements onboarding and subscription routing; ships `agent_brain_bot` and `region_event_send`.
- `regional_event_listener` exists as a documented subsystem; intake wiring is still pending.
- `nostr-persistence` crate is started (publisher/indexer foundation).
- `database` crate exists for users/topics/notifications (SQLx + migrations).
- `web/` Next.js UI exists for browser chat (not yet wired to Signal services).
- `api` crate provides an OpenAI-compatible inference gateway (stubbed echo).
- `api` can read a local knowledge base directory/file (simple keyword match).
- `ingester` crate exists for chunking files and publishing/indexing Nostr events.
- `admin-web` crate provides a dashboard + broadcast UI (auth still needed).

## Phase 0 - Signal MVP hardening + orchestrator adoption

Goal: make the orchestrator the default Signal message flow and close MVP reliability gaps.

- Wire `message-listener` -> `orchestrator` as the default bot path (new service binary).
- Integrate `agent-brain` onboarding with orchestrator responses (handoff when state machine needs AI).
- Add safer routing fallbacks (fail-closed to Maple; detect image attachments without router output).
- Implement dedupe and idempotent send logic for inbound/outbound message delivery.
- Persist preferences and minimal conversation metadata in SQLite.
- Add structured health checks and minimal logging defaults for production.

## Phase 1 - Orchestrator + Brain interface alignment

Goal: unify how routing signals, task hints, and model selection flow across brains.

- Add per-request model override support in `maple-brain` and `grok-brain` (selector already in orchestrator).
- Promote routing metadata (sensitivity, task_hint, attachment info) into `brain-core` message types.
- Version and track router/system prompts for reproducibility.
- Add unit/integration tests for routing plans, preferences, and vision routing.

## Phase 2 - Tooling and action framework expansion

Goal: consolidate tool calling across orchestrator and LLM tool calls.

- Bridge `agent-tools` registry into `brain-core::ToolExecutor` so both orchestration and LLM tools share a single tool surface.
- Add tool policy controls: allowlist per sender/group, rate limits, and timeouts.
- Introduce tool caching and structured tool result formatting for Signal output.
- Add new tools for alerts, knowledge lookup, and admin workflows.

## Phase 3 - Durable memory and preference persistence

Goal: move beyond in-memory context and align memory across Maple/Grok.

- SQLite tables for preferences, conversation summaries, tool history, and clear-context events.
- Summarization jobs that compress context into short memory blobs.
- Retention policy config (per-sender TTL, global caps).
- Shared history keys for group + direct messages across all brains.

## Phase 4 - Nostr persistence + sync

Goal: make knowledge and memory portable and resilient.

- Finalize Nostr event schema for documents, chunks, embeddings, and conversation summaries.
- Implement relay publishing + local indexer rehydration for vector DB and memory rebuilds.
- Store large blobs in object storage/IPFS with Nostr refs + hashes.
- Encrypt sensitive payloads and define provenance/policy events.

## Phase 5 - RAG pipeline + retrieval

Goal: retrieval-augmented responses with citations over Signal.

- Extend `ingester` for document + YouTube ingestion with embeddings.
- Add local vector DB (Qdrant/FAISS) + reranking.
- Expose retrieval as a tool in both orchestrator and `brain-core` tool calls.
- Deliver short, Signal-friendly citations/snippets in responses.

## Phase 6 - Safety and high-risk workflows

Goal: enforce strong privacy defaults for closed-society contexts.

- Metadata minimization (EXIF stripping, redacted logs, user-agent/IP avoidance).
- Encryption boundaries for blobs and short-lived plaintext handling.
- Media sanitization helpers (face blur, audio distortion, background obfuscation).
- Safety policies for sensitive domains and high-risk escalation paths.

## Decisions to lock early

- Router prompt versioning and how routing outputs are stored for audits.
- Unified tool interface (agent-tools vs brain-core) and permission model.
- Memory retention windows and default summarization strategy.
- What is stored on Nostr vs in object storage, plus event schema details.
- How citations/snippets are presented over Signal.

## Planned artifacts

- Orchestrator-first service binary that wires listener + routing + tools.
- Shared tool registry + policy layer for orchestrator + brain tool calls.
- DB schema for preferences, summaries, and dedupe.
- `crates/nostr-persistence` publisher/indexer with rehydration tooling.
- `crates/ingester` expansions for docs + YouTube.
- Vector DB schema and retrieval tool integration.
