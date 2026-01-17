# Aman Roadmap

This roadmap captures what still needs to be built beyond the current Signal MVP. It separates MVP hardening from the
next phases: RAG and Nostr-based persistence.

## Current status

- signal-cli daemon workflows and helper scripts are in place.
- `signal-daemon`, `message-listener`, and `broadcaster` are working as libraries.
- `agent-brain` is a stub and needs core logic implementation.
- `regional_event_listener` exists as a documented subsystem; intake wiring is still pending.
- `nostr-persistence` crate is started (publisher/indexer foundation).
- `database` crate exists for users/topics/notifications (SQLx + migrations).
- `web/` Next.js UI exists for browser chat (not yet wired to Signal services).
- `api` crate provides an OpenAI-compatible inference gateway (stubbed echo).
- `api` can read a local knowledge base directory/file (simple keyword match).
- `ingester` crate exists for chunking files and publishing/indexing Nostr events.

## Phase 0 - Signal MVP hardening

Goal: complete and stabilize the Signal-native assistant with opt-in regional alerts.

- Implement core services
  - `agent_brain` onboarding state machine and routing
  - subscription storage (SQLite) and dedupe logic
  - regional event intake (fixture endpoint or file-based ingest)
  - service binaries that wire `message-listener`, `agent-brain`, and `broadcaster`
  - optional: wire `web/` UI to `agent_brain` instead of direct OpenAI calls
  - replace `api` echo stub with real `agent_brain` inference
- Persistence
  - Wire `database` crate into services for user/topic/subscription persistence
  - SQLite schema for contacts, messages, subscriptions, dedupe
  - at-least-once delivery with idempotent sends
- Ops and safety
  - minimal logging defaults
  - retention windows and opt-out handling
  - structured config and health checks

## Phase 1 - RAG pipeline and ingester crate

Goal: support document and YouTube ingestion with retrieval and citations.

- Expand `ingester`
  - document ingestion (txt, md) with chunking (baseline)
  - YouTube ingestion (transcripts + metadata)
  - text extraction and normalization
  - chunking and embedding
- Retrieval pipeline
  - local vector DB (Qdrant, FAISS, or equivalent)
  - top-K retrieval + reranking
  - citations and snippets in responses
- Agent brain integration
  - retrieval-augmented prompts
  - query routing between chat and RAG

## Phase 2 - Nostr persistence and sync

Goal: make the knowledge base portable and resilient.

- `nostr-persistence` crate (publisher + indexer) is in progress.
- Nostr event schema
  - DocManifest events
  - Chunk events
  - Embedding artifact events (or references)
  - Access policy and provenance events
- Relay integration
  - publish events to relays
  - local indexer to rebuild vector DB from relay log
- Storage split
  - Nostr stores metadata, hashes, and policies
  - large blobs stored in object storage or IPFS with references in Nostr

## Phase 3 - Safety and high-risk workflows

Goal: enforce strong privacy defaults for closed-society contexts.

- Metadata minimization
  - strip EXIF and timestamps by default
  - avoid IP and user-agent logging
- Encryption boundaries
  - encrypt blobs at rest
  - short-lived plaintext during processing
- Media sanitization helpers
  - face blur presets
  - audio distortion
  - background obfuscation

## Decisions to lock early

- Location of vector search (local DB vs remote)
- What is stored on Nostr vs in object storage
- Event schema for documents and embeddings
- How citations/snippets are presented over Signal
- Default retention and logging policies

## Planned artifacts

- service binaries for listener/brain/broadcaster
- `crates/nostr-persistence` (publisher + indexer)
- `crates/ingester` (docs + YouTube ingestion)
- Nostr publisher/indexer module (provided by `nostr-persistence`)
- Vector DB schema and rehydration tooling
