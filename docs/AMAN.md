# Aman: Signal-native agent + regional alerts

Aman is a Signal-native assistant that also delivers opt-in regional alerts. A dedicated Signal account runs on a server
via `signal-cli`, with a small set of services that receive inbound messages, maintain a subscription state machine, and
broadcast alerts to subscribed contacts.

## Onboarding UX (MVP)

1. User messages Aman.
2. Aman asks if the user wants regional alerts and which region.
3. User opts in and provides a region ("Iran", "Syria", "Lebanon", etc.).
4. Aman confirms and starts sending alerts for that region.

If the user declines or sends "stop", Aman sets the contact to OptedOut and does not send alerts.

## User commands (MVP)

- `help`: show quick usage.
- `subscribe <region>`: opt in and set region.
- `region <region>`: update region.
- `status`: show current subscription state.
- `stop` / `unsubscribe`: opt out of alerts.

## Architecture (text diagram)

```
Signal User -> signal-cli daemon -> signal-daemon -> message_listener -> agent_brain -> broadcaster -> signal-daemon -> signal-cli daemon -> Signal User
                                                           |
                                                           v
                                                regional_event_listener
```

- `message_listener` owns inbound Signal transport and normalization.
- `brain-core` defines the shared `Brain` trait, `ToolExecutor` trait, `ConversationHistory`, and message types.
- `maple-brain` provides an OpenSecret-based Brain implementation (optional).
- `grok-brain` provides an xAI Grok-based Brain and tool executor (optional).
- `orchestrator` coordinates maple-brain (routing/responses) and grok-brain (search) with action plans.
- `orchestrator` can prompt for PII handling choices and format responses with Signal styles.
- `mock-brain` provides test Brain implementations for local development.
- `agent_brain` owns the state machine, routing, and subscription updates.
- `broadcaster` owns outbound delivery, retries, and chunking.
- `regional_event_listener` ingests regional events and emits `RegionEvent` records.
- `signal-daemon` is the HTTP/SSE client (with auto-reconnection) used by `message_listener` and `broadcaster`.
- `database` provides SQLite persistence for users, topics, subscriptions, and durable memory tables.
- Web UI in `web/` provides browser-based chat via `/api/chat` (separate from Signal flow).
- `api` provides an OpenAI-compatible inference endpoint for the web UI.
- `api` supports `echo`, `orchestrator`, and `openrouter` modes.
- `api` can optionally read a local knowledge base via `AMAN_KB_PATH` (or Nostr via `NOSTR_DB_PATH`).
- `api` can also proxy to OpenRouter for OpenAI-compatible inference.
- `ingester` chunks files and publishes/indexes Nostr events for the knowledge base.
- `admin-web` provides a dashboard and broadcast tool for operators.

Optional modes:
- `orchestrator` runs the full orchestrated bot with routing and search (recommended).
- `message_listener` can also run a `MessageProcessor` that calls a `Brain`
  (mock or MapleBrain) directly and sends replies via `signal-daemon`.
- `agent-brain` ships a simple `agent_brain_bot` binary for local Signal MVP use.
- MapleBrain can optionally call `realtime_search` via a `ToolExecutor` (e.g., Grok)
  for privacy-preserving real-time lookups.
  When `SQLITE_PATH` is set, the orchestrator persists preferences, rolling summaries,
  tool history, and clear-context events, then hydrates memory snapshots into prompts
  with policy controls and provenance hashes.

For the authoritative architecture spec, see `docs/ARCHITECTURE.md`.

## Ops and safety notes

- Opt-in alerts only; "stop" must always be honored.
- Minimal retention and minimal logging.
- Treat the server as a trusted endpoint (Signal E2EE terminates at the server).
- Prefer retention-disabled settings (e.g., `store: false`) when supported by your provider.
- Bind the admin web UI to localhost or place it behind authentication.
- PII detection can prompt for sanitize/private/cancel choices (sanitization wiring is still in progress).

## Future phases (planned)

- RAG pipeline for documents and YouTube transcripts.
- Local vector DB rehydration from Nostr events (memory durability is already supported).

## Security notes

- `signal-cli` stores keys/credentials on disk; protect the storage path.
- Never log message bodies by default.

## Links

- Local dev runbook: `docs/AMAN_LOCAL_DEV.md`
- Data retention policy: `docs/DATA_RETENTION.md`
- signal-cli daemon guide: `docs/signal-cli-daemon.md`
- Roadmap: `ROADMAP.md`
