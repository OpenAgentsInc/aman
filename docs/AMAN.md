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
- `agent_brain` owns the state machine, routing, and OpenAI-compatible API calls.
- `broadcaster` owns outbound delivery, retries, and chunking.
- `regional_event_listener` ingests regional events and emits `RegionEvent` records.
- `signal-daemon` is the HTTP/SSE client used by `message_listener` and `broadcaster`.
- `database` provides SQLite persistence for users, topics, and subscriptions.
- Web UI in `web/` provides browser-based chat via `/api/chat` (separate from Signal flow).
- `api` provides an OpenAI-compatible inference endpoint for the web UI.
- `api` can optionally read a local knowledge base via `AMAN_KB_PATH`.

For the authoritative architecture spec, see `docs/ARCHITECTURE.md`.

## Ops and safety notes

- Opt-in alerts only; "stop" must always be honored.
- Minimal retention and minimal logging.
- Treat the server as a trusted endpoint (Signal E2EE terminates at the server).
- Prefer `store: false` (or equivalent) with the OpenAI-compatible Responses API.

## Future phases (planned)

- RAG pipeline for documents and YouTube transcripts.
- Nostr relay persistence and local vector DB rehydration.

## Security notes

- `signal-cli` stores keys/credentials on disk; protect the storage path.
- Never log message bodies by default.

## Links

- Local dev runbook: `docs/AMAN_LOCAL_DEV.md`
- Data retention policy: `docs/DATA_RETENTION.md`
- signal-cli daemon guide: `docs/signal-cli-daemon.md`
- Roadmap: `ROADMAP.md`
