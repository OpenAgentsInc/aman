# Data Retention Policy (MVP)

This document defines what Aman stores, what it does not store, and the default retention targets for the MVP.

## Principles

- Minimize stored data.
- Opt-in alerts only.
- Store only what is required for dedupe, routing, and short context.
- Avoid logging message bodies.

## What is stored

### Signal account data (signal-cli)

- Private keys, registration data, and account metadata.
- Location (typical): `$XDG_DATA_HOME/signal-cli/data/` or `$HOME/.local/share/signal-cli/data/`.

### Bot state (SQLite or equivalent)

- `contacts`: identity, last_seen timestamp.
- `messages`: message id, sender id, timestamp, direction, status.
- `subscriptions`: identity -> region/topics, created_at, updated_at.
- `users`, `topics`, `notifications`: subscription store (via `database` crate).
- Optional: short context buffer or rolling summary (if enabled).

## What is not stored

- Full transcripts unless short-context mode is enabled.
- Attachments or media (MVP is text-only).
- Precise location data or unnecessary metadata.
- Raw OpenAI requests/responses when `store: false` is used.

## Planned data categories (Phase 2+)

These items are planned for the RAG and Nostr phases and are not part of the MVP.

- Nostr event log (`nostr_events`) with raw JSON for rehydration.
- Document manifests and chunk metadata (Nostr events).
- Encrypted document blobs stored in object storage or IPFS.
- Embedding artifacts or references used to rebuild the local vector DB.
- Access policy and provenance events.

## Retention windows (defaults)

- Dedupe metadata: 7 days.
- Message bodies: only if short-context is enabled; keep last N turns per user (N <= 6).
- Conversation summaries: keep until unsubscribed or manually cleared.
- Subscriptions: keep until user opts out.

Adjust these windows based on threat model and legal constraints.

## Red lines

- Do not log message bodies by default.
- Do not store attachments in MVP.
- Do not export contact lists or message content to third parties.
- Do not retain data after an explicit opt-out request beyond required dedupe metadata.

## OpenAI-compatible API data

- Prefer `store: false` (or equivalent) to avoid server-side retention of application state.
- Only send the minimum text required for the response.

## Security notes

- Treat `signal-cli` storage paths as secret material.
- Restrict filesystem permissions and backups to trusted operators.
- Avoid dumping state databases into logs or crash reports.
