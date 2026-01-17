# Aman (Signal AI Bot) - README

## What is Aman?

Aman is a Signal-native assistant and activist notification system.
It runs a dedicated Signal account on a server using `signal-cli`.
Inbound messages are decrypted locally and normalized by `message_listener`.
An `agent_brain` service handles onboarding and routing decisions.
It calls an OpenAI-compatible Responses API for generation (example docs: [OpenAI Platform][1]).
Replies are sent back to Signal via `broadcaster`.
Aman can also deliver opt-in regional alerts to subscribed contacts.
Alerts are driven by a regional event listener and a subscription state machine.
The MVP is text-only with minimal retention and minimal logging.
Components are decoupled so receiving never blocks on generation.

## Aman MVP

- Signal-native chat with a dedicated server-side account.
- Opt-in regional alerts with a simple state machine.
- Minimal storage for dedupe and short context.
- No attachments or document upload.

## Component overview

- `message_listener`: Signal inbound transport and message normalization.
- `agent_brain`: onboarding, subscriptions, routing, and OpenAI-compatible API calls.
- `broadcaster`: outbound Signal delivery, chunking, retries.
- `regional_event_listener`: regional event ingestion and normalization.

## Message and event flow

Message flow:

1. Signal -> `message_listener` receives inbound message.
2. `message_listener` emits normalized `InboundMessage`.
3. `agent_brain` decides: onboarding, chat response, or subscription update.
4. `broadcaster` sends reply via `signal-cli`.

Event flow:

1. `regional_event_listener` observes an event and normalizes to `RegionEvent`.
2. `agent_brain` queries subscriptions and creates alerts.
3. `broadcaster` delivers alerts to subscribed identities.

## Quickstart (dev)

See the runbook: `docs/AMAN_LOCAL_DEV.md`.

## Docs

- Architecture: `docs/ARCHITECTURE.md`
- Aman overview: `docs/AMAN.md`
- Data retention: `docs/DATA_RETENTION.md`
- Roadmap: `ROADMAP.md`

## Crates

- `crates/message-listener/README.md`
- `crates/agent-brain/README.md`
- `crates/broadcaster/README.md`

## Safety and ops

- Opt-in alerts only; honor "stop" everywhere.
- Minimal retention and minimal logging.
- Use `store: false` (or equivalent) with the OpenAI-compatible Responses API (example docs: [OpenAI Platform][2]).

## Future work

- Web UI for chat and uploads (Signal remains the trust channel).
- RAG pipeline with ingestion for documents and YouTube.
- Nostr relay persistence and local vector DB rehydration.

[1]: https://platform.openai.com/docs/api-reference/responses?utm_source=chatgpt.com "Responses | OpenAI API Reference"
[2]: https://platform.openai.com/docs/guides/your-data?utm_source=chatgpt.com "Data controls in the OpenAI platform"
