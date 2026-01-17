# Aman Roadmap

This roadmap captures what still needs to be built beyond the current Signal MVP. It separates MVP hardening from the
next phases: RAG and Nostr-based persistence.

## Phase 0 - Signal MVP hardening

Goal: complete and stabilize the Signal-native assistant with opt-in regional alerts.

- Implement core services
  - `message_listener` inbound normalization and dedupe
  - `agent_brain` onboarding state machine and routing
  - `broadcaster` outbound delivery with retries and chunking
  - `regional_event_listener` ingestion and normalization
- Persistence
  - SQLite schema for contacts, messages, subscriptions, dedupe
  - at-least-once delivery with idempotent sends
- Ops and safety
  - minimal logging defaults
  - retention windows and opt-out handling
  - structured config and health checks

## Phase 1 - RAG pipeline and ingester crate

Goal: support document and YouTube ingestion with retrieval and citations.

- New crate: `ingester`
  - document ingestion (pdf, docx, txt)
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

- `crates/ingester` (docs + YouTube ingestion)
- Nostr publisher/indexer module (inside `ingester` or separate crate)
- Vector DB schema and rehydration tooling
