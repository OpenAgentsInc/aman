# api

OpenAI-compatible API gateway for local Aman inference.

## Responsibilities

- Serve `/v1/chat/completions` (OpenAI-style).
- Serve `/v1/models` (model list).
- Return stubbed echo responses by default, or run the full orchestrator when enabled.

## Run

```bash
export AMAN_API_ADDR="127.0.0.1:8787"
export AMAN_API_TOKEN="aman-local"   # optional
export AMAN_API_MODEL="aman-chat"
export AMAN_KB_PATH="./knowledge"    # optional (txt/md/jsonl files)
export AMAN_API_MODE="orchestrator"  # "echo" (default), "orchestrator", or "openrouter"
cargo run -p api
```

If `NOSTR_DB_PATH` is set, the API reads from the Nostr indexer DB instead of `AMAN_KB_PATH`.

## Request example

```bash
curl -s http://127.0.0.1:8787/v1/chat/completions \
  -H "content-type: application/json" \
  -H "authorization: Bearer aman-local" \
  -d '{
    "model": "aman-chat",
    "messages": [{"role":"user","content":"hello"}]
  }'
```

## Response behavior

### Echo mode (default)

- Echoes the last user message with `Echo: <text>`.
- Streams if `stream: true` is provided.
- If `AMAN_KB_PATH` is set and a match is found, returns a KB snippet instead of echo.
- If `NOSTR_DB_PATH` is set and chunk blobs are file-based, returns the best matching chunk.

### Orchestrator mode

Set `AMAN_API_MODE=orchestrator` to run the full Aman brain stack via the orchestrator.
This requires Maple/Grok environment variables (e.g., `MAPLE_API_KEY`, `GROK_API_KEY`)
and uses the same routing + tool behavior as the Signal bot.

Optional headers:

- `X-Aman-User`: Stable sender ID for memory and preferences (default: `api-user`)
- `X-Aman-Group`: Group ID to scope history (optional)

### OpenRouter mode

Set `AMAN_API_MODE=openrouter` to proxy requests to OpenRouter's OpenAI-compatible endpoint.
Required env vars:

- `OPENROUTER_API_KEY` (required)
- `OPENROUTER_API_URL` (default: `https://openrouter.ai/api/v1`)
- `OPENROUTER_MODEL` (optional default if the request omits `model`)
- `OPENROUTER_HTTP_REFERER` and `OPENROUTER_X_TITLE` (optional, used for OpenRouter rankings)

If `AMAN_KB_PATH` is set and a match is found, the API injects a system message with
the KB snippet before sending the request to OpenRouter.
