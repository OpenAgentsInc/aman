# Aman - Signal-native assistant and alert system

## What is Aman?

Aman is a Signal-native assistant and activist notification system built for organizers, journalists,
and human-rights defenders. It lets people ask questions, get guidance, and opt into regional alerts
without leaving Signal. The focus is on minimal retention and operational safety.

## What it enables today (MVP)

- Signal-based chat via a dedicated account
- Opt-in regional alerts (outages, throttling, advisories)
- Basic onboarding and subscription management
- Minimal state storage (short context and dedupe)
- Operator broadcasts and dashboards
- Optional web UI for browser chat

## What it is not yet

- Full RAG pipeline (planned)
- Nostr-backed persistence (foundation only)
- Long-term message transcript storage
- Automated event ingestion from live feeds

## Quickstart (local dev)

See `docs/AMAN_LOCAL_DEV.md` for the shortest path to running Aman locally.

## Prompt Configuration

The bot's behavior is controlled by two prompt files at the project root:

| File | Purpose | Env Override |
|------|---------|--------------|
| `SYSTEM_PROMPT.md` | Main bot persona and response style | `MAPLE_SYSTEM_PROMPT` or `MAPLE_PROMPT_FILE` |
| `ROUTER_PROMPT.md` | Message classification and action routing | `ROUTER_SYSTEM_PROMPT` or `ROUTER_PROMPT_FILE` |

Edit these files to customize the bot's personality, tone, and routing behavior without recompiling.

## Docs and references

- `docs/ARCHITECTURE.md` - system design
- `docs/DATA_RETENTION.md` - storage and safety
- `docs/signal-cli-daemon.md` - Signal daemon details
- `ROADMAP.md` - next phases
- `crates/README.md` - crate catalog
- `web/README.md` - browser UI

## Safety posture

Aman is designed for high-risk contexts: opt-in alerts only, "stop" honored everywhere,
minimal retention, and no message body logging by default. Treat the server as a trusted
boundary because Signal E2EE terminates there.

## License

MIT
