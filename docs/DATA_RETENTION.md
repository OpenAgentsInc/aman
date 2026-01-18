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
- Attachments may be written to disk by signal-cli and referenced by local file path.

### Bot state (SQLite or equivalent)

- `contacts`: identity, last_seen timestamp.
- `messages`: message id, sender id, timestamp, direction, status.
- `subscriptions`: identity -> region/topics, created_at, updated_at.
- `users`, `topics`, `notifications`: subscription store (via `database` crate).
- `preferences`: sender/group routing preferences.
- `conversation_summaries`: rolling summaries for routing context.
- `tool_history`: tool execution records (sanitized inputs/outputs; avoid raw PII), plus privacy-choice outcomes.
- `clear_context_events`: history resets for audit and retention.
- Optional: attachment metadata (filename, content type, local file path) if persisted for processing.

## What is not stored

- Full transcripts unless short-context mode is enabled.
- Attachment contents or media binaries (unless using MapleBrain vision).
- Precise location data or unnecessary metadata.
- Raw upstream requests/responses when retention is disabled (if supported by the provider).

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
- Conversation summaries: 30 days (rolling summary, capped by row count).
- Tool history: 14 days (capped by row count).
- Clear-context events: 30 days (capped by row count).
- Subscriptions: keep until user opts out.

Adjust these windows based on threat model and legal constraints.

## Red lines

- Do not log message bodies by default.
- Do not store attachments in MVP.
- Do not export contact lists or message content to third parties.
- Do not retain data after an explicit opt-out request beyond required dedupe metadata.

## OpenAI-compatible provider data

- Prefer retention-disabled settings (e.g., `store: false`) to avoid server-side retention of application state.
- Only send the minimum text required for the response.

## Maple/OpenSecret data flow

- When MapleBrain processes image attachments, attachment bytes are read from disk and sent to the Maple/OpenSecret API.
- Do not persist or log attachment contents outside of signal-cli storage.

## Tool executor data (xAI)

- GrokToolExecutor receives only sanitized search queries (no raw user text).
- Search results are returned to the brain for synthesis; tool outputs are stored in `tool_history` when durable memory is enabled.

## PII handling notes

- The router can flag PII and prompt the user for a privacy choice.
- The `sanitize` tool exists to redact PII before fast-mode requests, but full orchestration wiring is still in progress.
- Memory prompt policy can redact or skip durable memory injection when configured.

## Security notes

- Treat `signal-cli` storage paths as secret material.
- Restrict filesystem permissions and backups to trusted operators.
- Avoid dumping state databases into logs or crash reports.
