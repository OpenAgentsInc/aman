# Nostr Memory Schema

Version: 1

This document defines Nostr event kinds, tags, and payload schemas for Aman
memory durability (preferences, summaries, tool history, and clear-context
events). SQLite remains the fast local runtime view; Nostr is the append-only
source of truth.

## Event kinds

| Event | Kind | `k` tag | Replaceable |
| --- | --- | --- | --- |
| AmanPreference | 30093 | `aman_preference` | Parameterized replaceable |
| AmanConversationSummary | 30094 | `aman_summary` | Parameterized replaceable |
| AmanToolHistoryEntry | 30095 | `aman_tool_history` | Append-only |
| AmanClearContextEvent | 30096 | `aman_clear_context` | Append-only |
| AmanSubscriptionState (optional) | 30097 | `aman_subscription_state` | Parameterized replaceable |

## Required tags

All memory events MUST include:

- `d`: deterministic identifier for replaceable kinds or dedupe keys.
- `k`: semantic label (values listed above).
- `hk`: history key (stable sender/group key).
- `v`: schema version integer.
- `ts`: created_at unix seconds (in addition to nostr `created_at`).
- `enc`: only when encrypted, with `secretbox-v1`.

## Content schemas

All payloads are JSON objects.

### AmanPreference (kind 30093)

```json
{
  "history_key": "string",
  "preference": "string",
  "updated_at": 1700000000
}
```

### AmanConversationSummary (kind 30094)

```json
{
  "history_key": "string",
  "summary": "string",
  "message_count": 42,
  "updated_at": 1700000000
}
```

### AmanToolHistoryEntry (kind 30095)

```json
{
  "history_key": "string",
  "tool_name": "string",
  "success": true,
  "content": "string",
  "sender_id": "optional string",
  "group_id": "optional string",
  "created_at": 1700000000
}
```

### AmanClearContextEvent (kind 30096)

```json
{
  "history_key": "string",
  "sender_id": "optional string",
  "created_at": 1700000000
}
```

### AmanSubscriptionState (optional, kind 30097)

```json
{
  "history_key": "string",
  "topics": ["iran", "bitcoin"],
  "updated_at": 1700000000
}
```

## Deterministic IDs (`d` tag)

- Preferences + summaries (latest state):
  - `d = <history_key>:preference`
  - `d = <history_key>:summary`
- Tool history + clear context (append-only):
  - `d = <history_key>:<sha256(payload)>` (stable dedupe)

## Conflict resolution

- Preferences + summaries: last-write-wins by tag `ts`, tie-break by event id
  lexicographic order.
- Tool history + clear events: append-only; duplicates deduped by `d`.

## Encryption

If `NOSTR_SECRETBOX_KEY` is configured, the payload is encrypted using
`secretbox-v1` (XSalsa20-Poly1305). The `enc=secretbox-v1` tag is required.
The ciphertext payload includes the nonce prefix.
