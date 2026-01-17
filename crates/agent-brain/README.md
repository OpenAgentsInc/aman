# agent_brain

## Responsibility

The agent brain owns the core decision layer: onboarding state machine, subscription routing, and OpenAI-compatible API
calls. It decides whether to respond conversationally, update a subscription, or fan out a regional alert.

## Public interfaces

Consumes:

- `InboundMessage` (from `message_listener`)
- `RegionEvent` (from `regional_event_listener`)

Produces:

- `OutboundMessage` (to `broadcaster`)
- subscription state updates (to the state store)

## Onboarding and state machine

- New contacts are prompted to opt in to regional alerts.
- Regions are parsed from user input ("Iran", "Syria", etc.).
- Users can opt out at any time with "stop" or "unsubscribe".

See `docs/ARCHITECTURE.md` for the authoritative state machine.

## Subscription storage

MVP intent: store subscriptions in SQLite with fields for identity, region, topics, and timestamps.

## OpenAI call boundary

Inputs:

- minimal user text + optional short context
- system prompt defining safety posture and onboarding behavior

Outputs:

- short, actionable replies
- optional clarifying questions for ambiguous regions

Prefer `store: false` (or equivalent) with the Responses API.

## Command handling (MVP)

- `help`
- `subscribe <region>`
- `region <region>`
- `status`
- `stop` / `unsubscribe`

## How to run it

MVP target command (adjust to your runtime):

```bash
cargo run --bin agent-brain -- --db "$SQLITE_PATH" --model "$MODEL"
```

## How to test it

- `cargo test`
- Use fixture inputs for `InboundMessage` and `RegionEvent` to validate state transitions.

## Failure modes

- Region parsing fails or is ambiguous.
- OpenAI-compatible API timeouts or rate limits.
- Duplicate inbound messages cause repeated onboarding prompts.

## Future work

- RAG integration with retrieval and citations.
- Nostr-backed document manifests and chunk references.
- Query routing between chat and RAG flows.

## Security notes

- Do not log message bodies by default.
- Store only minimal context required for the state machine.
