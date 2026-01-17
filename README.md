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

- `signal-daemon`: HTTP/SSE client for signal-cli daemon.
- `message_listener`: Signal inbound transport and message normalization.
- `agent_brain`: onboarding, subscriptions, routing, and OpenAI-compatible API calls.
- `broadcaster`: outbound Signal delivery, chunking, retries.
- `regional_event_listener`: regional event ingestion and normalization.

## Message and event flow

Message flow:

1. Signal -> signal-cli daemon receives inbound message.
2. `message_listener` reads SSE events via `signal-daemon`.
3. `message_listener` emits normalized `InboundMessage`.
4. `agent_brain` decides: onboarding, chat response, or subscription update.
5. `broadcaster` sends reply via `signal-daemon` to signal-cli daemon.

Event flow:

1. `regional_event_listener` observes an event and normalizes to `RegionEvent`.
2. `agent_brain` queries subscriptions and creates alerts.
3. `broadcaster` delivers alerts to subscribed identities.

## Quickstart (dev)

See the runbook: `docs/AMAN_LOCAL_DEV.md`.

## Setup

### Prerequisites

- Java 21+ (for signal-cli)
- Rust toolchain (for crates)
- A phone number for the bot's Signal account

Copy the example env file and edit values as needed:

```bash
cp .env.example .env
```

### 1. Build signal-cli

```bash
./scripts/build-signal-cli.sh
```

This builds a fat JAR at `build/signal-cli.jar`.
If the build script reports a missing submodule, run `git submodule update --init --recursive`.

### 2. Register Signal account

First-time registration:

```bash
# Request SMS verification code
./scripts/register-signal.sh +1234567890

# If captcha is required
./scripts/register-signal.sh +1234567890 --captcha

# For voice call instead of SMS
./scripts/register-signal.sh +1234567890 --voice
```

After receiving the code:

```bash
./scripts/signal-cli.sh -a +1234567890 verify <CODE>
```

### 3. Run the daemon

```bash
./scripts/run-signal-daemon.sh +1234567890

# Or with environment variable
AMAN_NUMBER=+1234567890 ./scripts/run-signal-daemon.sh
```

Test endpoints:

```bash
# Health check
curl http://127.0.0.1:8080/api/v1/check

# Get version
curl -X POST http://127.0.0.1:8080/api/v1/rpc \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"version","id":1}'

# Subscribe to incoming messages
curl -N http://127.0.0.1:8080/api/v1/events
```

## Scripts

| Script | Description |
|--------|-------------|
| `scripts/build-signal-cli.sh` | Build signal-cli fat JAR to `build/signal-cli.jar` |
| `scripts/signal-cli.sh` | General wrapper - pass any args to signal-cli |
| `scripts/register-signal.sh` | Register/re-register a Signal account |
| `scripts/run-signal-daemon.sh` | Run signal-cli daemon for development |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AMAN_NUMBER` | - | Bot's Signal phone number |
| `SIGNAL_CLI_JAR` | `build/signal-cli.jar` | Path to signal-cli JAR |
| `HTTP_ADDR` | `127.0.0.1:8080` | Daemon HTTP bind address |

## Docs

- Architecture: `docs/ARCHITECTURE.md`
- Aman overview: `docs/AMAN.md`
- Data retention: `docs/DATA_RETENTION.md`
- signal-cli daemon guide: `docs/signal-cli-daemon.md`
- Roadmap: `ROADMAP.md`

## Crates

| Crate | Description |
|-------|-------------|
| `signal-daemon` | Core client for signal-cli daemon (HTTP/SSE) |
| `message-listener` | Signal inbound transport using signal-daemon |
| `broadcaster` | Signal outbound delivery using signal-daemon |
| `agent-brain` | Onboarding, routing, and API calls |

See individual READMEs in `crates/*/README.md` for API documentation.

## Safety and ops

- Opt-in alerts only; honor "stop" everywhere.
- Minimal retention and minimal logging.
- Use `store: false` (or equivalent) with the OpenAI-compatible Responses API (example docs: [OpenAI Platform][2]).

## Future work

- RAG pipeline with ingestion for documents and YouTube.
- Nostr relay persistence and local vector DB rehydration.

[1]: https://platform.openai.com/docs/api-reference/responses?utm_source=chatgpt.com "Responses | OpenAI API Reference"
[2]: https://platform.openai.com/docs/guides/your-data?utm_source=chatgpt.com "Data controls in the OpenAI platform"
