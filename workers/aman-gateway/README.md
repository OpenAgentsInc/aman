# Aman Gateway Worker

Cloudflare Worker (workers-rs) that exposes an OpenAI-compatible API backed by OpenRouter with a
minimal Aman-like memory loop. It stores short memory snapshots in KV and can optionally publish
summary events to Nostr (stubbed for now).

## Endpoints

- `GET /health`
- `GET /v1/models`
- `POST /v1/chat/completions` (non-streaming)

## Quickstart

1) Create a KV namespace and update `wrangler.toml`:

```bash
wrangler kv:namespace create "AMAN_MEMORY"
wrangler kv:namespace create "AMAN_MEMORY" --preview
```

2) Configure secrets and vars:

```bash
wrangler secret put OPENROUTER_API_KEY
wrangler secret put WORKER_API_TOKEN  # optional unless ALLOW_ANON=true
```

3) Run locally:

```bash
wrangler dev
```

## Example request

```bash
curl -s http://127.0.0.1:8787/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -H 'X-Aman-User: demo-user' \
  -d '{"model":"openai/gpt-4o-mini","messages":[{"role":"user","content":"Hello"}]}'
```

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
- `NOSTR_RELAYS` (comma-separated; stubbed publish)
- `NOSTR_SECRET_KEY` (hex or nsec; stubbed publish)

## Notes

- Streaming responses are not supported yet.
- The worker ignores tool calls and function calling in Phase 1.
- Nostr publishing is logged but not yet implemented in the worker runtime.
