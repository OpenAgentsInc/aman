# Architecture

The authoritative architecture spec lives at `docs/architecture/aman-signal-mvp.md`.

Aman is a Signal-native system composed of:

- `message_listener` for inbound Signal transport.
- `agent_brain` for onboarding, subscriptions, and routing decisions.
- `broadcaster` for outbound Signal delivery.
- `regional_event_listener` for regional alert ingestion.

For data model details, state machine, flows, and safety posture, see the full spec.
