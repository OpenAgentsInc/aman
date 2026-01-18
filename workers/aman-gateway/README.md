# Aman Gateway Worker

Cloudflare Worker (workers-rs) that exposes an OpenAI-compatible API backed by OpenRouter with a
minimal Aman-like loop. It stores short memory snapshots in KV, maintains a local D1 knowledge
base synced from Nostr DocManifest/ChunkRef events, and injects KB snippets into prompts.

## Endpoints

- `GET /health`
- `GET /v1/models`
- `POST /v1/chat/completions` (streaming supported via `stream: true`)
- `GET /kb/status` (debug)
- `POST /kb/search` (debug)

## Quickstart

1) Create KV namespaces and update `wrangler.toml`:

```bash
wrangler kv namespace create AMAN_MEMORY
wrangler kv namespace create AMAN_MEMORY --preview
wrangler kv namespace create AMAN_META
wrangler kv namespace create AMAN_META --preview
```

2) Create D1 and apply migrations:

```bash
wrangler d1 create aman_kb
wrangler d1 migrations apply aman_kb
wrangler d1 migrations apply aman_kb --remote
```

3) Configure secrets and vars:

```bash
wrangler secret put OPENROUTER_API_KEY
wrangler secret put WORKER_API_TOKEN  # optional unless ALLOW_ANON=false
wrangler secret put NOSTR_SECRETBOX_KEY  # optional
```

Set `NOSTR_RELAYS` (comma-separated) and optional `NOSTR_KB_AUTHOR` in `wrangler.toml` or via
`wrangler secret put` if you want to keep the value private.

4) Run locally:

```bash
wrangler dev
```

## Example request

```bash
curl -s http://127.0.0.1:8787/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -H 'X-Aman-User: demo-user' \
  -d '{"model":"x-ai/grok-4.1-fast","messages":[{"role":"user","content":"Hello"}]}'
```

## Knowledge base ingestion

- Use `ingester --inline-text` to embed chunk text directly inside `ChunkRef` events.
- Set `NOSTR_RELAYS` to the relay(s) you publish to.
- Cron sync runs every 5 minutes (configurable in `wrangler.toml`).

## Environment variables

Required:

- `OPENROUTER_API_KEY`

Optional:

- `OPENROUTER_API_URL` (default: `https://openrouter.ai/api/v1`)
- `OPENROUTER_HTTP_REFERER` (optional header)
- `OPENROUTER_X_TITLE` (optional header)
- `DEFAULT_MODEL` (default: `x-ai/grok-4.1-fast`)
- `SUMMARY_MODEL` (default: `mistral-small`)
- `SYSTEM_PROMPT` (default: Aman identity + safety/clarity guidance)
- `MEMORY_MAX_CHARS` (default: `1200`)
- `MEMORY_SUMMARIZE_EVERY_TURNS` (default: `6`)
- `ALLOW_ANON` (default: `true`)
- `WORKER_API_TOKEN` (required only when `ALLOW_ANON=false`)
- `RATE_LIMIT_MAX` (default: `60`)
- `RATE_LIMIT_WINDOW_SECS` (default: `60`)
- `NOSTR_RELAYS` (comma-separated relay URLs)
- `NOSTR_KB_AUTHOR` (optional pubkey filter)
- `NOSTR_SECRETBOX_KEY` (optional secretbox key for encrypted payloads)
- `KB_SYNC_LOOKBACK_SECS` (default: `86400`)
- `KB_MAX_SNIPPET_CHARS` (default: `600`)
- `KB_MAX_TOTAL_CHARS` (default: `1200`)
- `KB_MAX_HITS` (default: `3`)

## Notes

- Streaming responses are supported (SSE passthrough).
- KV bindings: `AMAN_MEMORY` for chat memory, `AMAN_META` for KB sync metadata.
- D1 binding: `AMAN_KB` for KB storage and search.
- Nostr sync is best-effort and continues if a relay fails.
