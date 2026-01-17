# Architecture

## Overview

Aman is a Signal-native bot. A dedicated Signal account runs on a server via `signal-cli`, which decrypts inbound
messages locally. A bot worker turns each incoming message into a prompt for an OpenAI-compatible Responses API, then
delivers the model's text back to the sender through `signal-cli`.

This keeps the MVP small: one Signal identity, text-only, and a minimal state store for dedupe and short context.

## Components

- Signal user (mobile app)
- Aman Signal account (server-side, `signal-cli`)
- Bot worker (queue + state + OpenAI-compatible API client)
- Local storage (Signal keys + bot state)
- OpenAI-compatible API endpoint (Responses API)

## Signal integration

`signal-cli` is a server-focused CLI for Signal that supports registering, verifying, sending, and receiving messages.
It exposes two integration styles:

- CLI receive/send for simple polling pipelines.
- Daemon mode with JSON-RPC (stdin/stdout or socket/HTTP) and optional D-Bus.

HTTP daemon mode can emit incoming messages as a Server-Sent Events (SSE) stream, which is a natural fit for a worker
that subscribes to events. Whatever integration is used, Signal expects incoming messages to be received regularly
(via daemon or receive) to keep encryption and group state up to date.

Operational note: Signal clients expire on a short cadence. `signal-cli` should be kept current to remain compatible
with Signal server changes.

## Data flow (message lifecycle)

1. `signal-cli` yields an inbound message event (sender, timestamp, body, metadata).
2. Bot worker normalizes content and deduplicates it.
3. Optional: load short context for the sender (last N turns or a rolling summary).
4. Build a prompt and call the OpenAI-compatible Responses API.
5. Split long responses to fit Signal message limits and send replies via `signal-cli`.
6. Persist inbound/outbound entries for reliability, throttling, and context.

## Process shape

Two long-lived processes are expected:

- `signal-cli` runtime (account session + message receive stream)
- Bot worker (queueing + OpenAI-compatible API calls + sending)

Decouple receive from generation so a slow API call does not block inbound message handling. A lightweight queue
(even in SQLite) is enough for the MVP.

## Minimal data model (MVP)

Suggested tables:

- `contacts(sender_id, last_seen_at)`
- `messages(id, sender_id, ts, direction, body, status)`
- `conversations(sender_id, summary, last_n_turns_json)` (optional)

These support deduplication, rate limiting, and short context.

## Security and privacy

`signal-cli` stores account keys and credentials on disk (typically under
`$XDG_DATA_HOME/signal-cli/data/` or `$HOME/.local/share/signal-cli/data/`). Treat this path as secret material and
protect it with strict filesystem permissions and backups.

Signal is end-to-end encrypted between the user's device and the server, but the server is the endpoint. If the server
is compromised, message content is exposed. Operate with minimal logs, short retention, and strict access controls.

Anything sent to an OpenAI-compatible endpoint leaves the server. Use provider data controls where available and prefer
`store: false` (or equivalent) when you do not want application state retained. The Responses API is the recommended
primitive for new builds (see provider docs for retention behavior).

## Reliability and limits

- Deduplicate messages to prevent double replies after restarts or delivery quirks.
- Backoff on API rate limits and retry transient errors.
- Chunk long model outputs to fit Signal's practical message limits.
- Add per-sender throttles to avoid spam loops.
- Remember the bot is a single Signal identity; scale-out means multiple accounts or multiplexing.

## Prompting policy (MVP)

A baseline system instruction should keep responses concise, ask clarifying questions when needed, avoid requesting
identifying details unless required, and steer risky requests toward safer guidance. This can be tuned per workflow.

## Scope decisions

Supported in MVP:

- Text-only chat
- Stateless or short-context memory per sender

Explicitly out of scope for now:

- Attachments and document upload
- Retrieval-augmented generation (RAG)
- Web UI

## References (example docs)

[1]: https://platform.openai.com/docs/api-reference/responses?utm_source=chatgpt.com "Responses | OpenAI API Reference"
[2]: https://platform.openai.com/docs/guides/your-data?utm_source=chatgpt.com "Data controls in the OpenAI platform"
[3]: https://platform.openai.com/docs/guides/rate-limits?utm_source=chatgpt.com "Rate limits | OpenAI API"
