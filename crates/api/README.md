# api

OpenAI-compatible API gateway for local Aman inference.

## Responsibilities

- Serve `/v1/chat/completions` (OpenAI-style).
- Serve `/v1/models` (model list).
- Return stubbed echo responses for now.

## Run

```bash
export AMAN_API_ADDR="127.0.0.1:8787"
export AMAN_API_TOKEN="aman-local"   # optional
export AMAN_API_MODEL="aman-chat"
export AMAN_KB_PATH="./knowledge"    # optional (txt/md/jsonl files)
cargo run -p api
```

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

- Echoes the last user message with `Echo: <text>`.
- Streams if `stream: true` is provided.
- If `AMAN_KB_PATH` is set and a match is found, returns a KB snippet instead of echo.
