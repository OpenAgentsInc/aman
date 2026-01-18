# Debugging Guide

This guide covers how to debug issues with the Aman bot using detailed logging and real-time log monitoring.

## Quick Start

```bash
# Terminal 1 - Run the bot with logging enabled
AMAN_LOG_FILE=logs/aman.log ./scripts/dev.sh --build

# Terminal 2 - Monitor logs in real-time
tail -f logs/aman.log | jq
```

## Logging Configuration

### Environment Variables

Add these to your `.env` file:

| Variable | Default | Description |
|----------|---------|-------------|
| `AMAN_LOG_FILE` | `logs/aman.log` | Path to detailed log file (JSON format) |
| `AMAN_LOG_LEVEL` | `debug` | Log level for file: `trace`, `debug`, `info`, `warn`, `error` |
| `RUST_LOG` | `info` | Console log level (standard format) |

### Log Levels by Component

```bash
# Debug routing decisions
RUST_LOG=orchestrator=debug,router=debug

# Debug AI requests/responses
RUST_LOG=maple_brain=debug,grok_brain=debug

# Debug tool execution
RUST_LOG=agent_tools=debug

# Full verbose (warning: very noisy)
RUST_LOG=trace,hyper=warn,reqwest=warn
```

### Recommended Debug Configuration

```bash
# In .env file
AMAN_LOG_FILE=logs/aman.log
AMAN_LOG_LEVEL=trace
RUST_LOG=info,orchestrator=debug
```

## Log Output

The bot writes logs to two destinations:

1. **Console** - Human-readable format, respects `RUST_LOG`
2. **File** - JSON format, includes full payloads for debugging

### Log File Format

Each line is a JSON object:
```json
{
  "timestamp": "2026-01-18T17:49:46.174566Z",
  "level": "INFO",
  "target": "orchestrator::orchestrator",
  "fields": {
    "message": "Processing message from user-uuid",
    "sender": "user-uuid",
    "text": "What's the bitcoin price?"
  }
}
```

## Key Log Events

These are the most important events to watch for when debugging:

| Event | Level | What It Shows |
|-------|-------|---------------|
| `INBOUND_MESSAGE` | TRACE | Full incoming message (sender, text, attachments) |
| `ROUTER_INPUT` | TRACE | Formatted input sent to router classifier |
| `ROUTER_RAW_RESPONSE` | TRACE | Raw JSON response from router LLM |
| `ROUTER_PARSED_PLAN` | TRACE | Successfully parsed routing plan |
| `ROUTER_PARSE_FAILED` | WARN | Router JSON parse error (includes raw response) |
| `ROUTING_PLAN` | TRACE | Action plan summary |
| `ROUTING_ACTION` | DEBUG | Each action in the plan |
| `BRAIN_REQUEST` | TRACE | Request being sent to Maple/Grok |
| `BRAIN_RESPONSE` | TRACE | Full response from AI |

## Real-Time Log Monitoring

### Basic Monitoring

```bash
# Raw JSON (all events)
tail -f logs/aman.log | jq

# Compact format
tail -f logs/aman.log | jq -c
```

### Filtered Monitoring

```bash
# Only key events (recommended)
tail -f logs/aman.log | jq -c 'select(.fields.message | test("INBOUND|ROUTER|BRAIN"; "i"))'

# Only errors and warnings
tail -f logs/aman.log | jq 'select(.level | test("WARN|ERROR"))'

# Only routing decisions
tail -f logs/aman.log | jq 'select(.fields.message | test("ROUTER"; "i"))'

# Only brain requests/responses
tail -f logs/aman.log | jq 'select(.fields.message | test("BRAIN"; "i"))'
```

### Human-Readable Format

```bash
# Condensed readable output
tail -f logs/aman.log | jq -r '
  "\(.timestamp | split(".")[0]) [\(.level)] \(.fields.message)"
'

# With key fields
tail -f logs/aman.log | jq -r '
  select(.fields.message | test("INBOUND|ROUTER|BRAIN"; "i")) |
  "\(.timestamp | split("T")[1] | split(".")[0]) [\(.level[0:1])] \(.fields.message[0:60])"
'
```

## Common Issues and How to Debug Them

### Issue: Router JSON Parse Errors

**Symptom:** `ROUTER_PARSE_FAILED` warnings in logs

**What to look for:**
```bash
tail -100 logs/aman.log | jq 'select(.fields.message == "ROUTER_PARSE_FAILED")'
```

**Common causes:**
- LLM outputs malformed JSON (extra braces, missing quotes)
- Response contains markdown around JSON

**Example log:**
```json
{
  "level": "WARN",
  "fields": {
    "message": "ROUTER_PARSE_FAILED",
    "error": "trailing characters at line 1 column 200",
    "raw_response": "{\"actions\": [...]}}"
  }
}
```

### Issue: Tool Not Executing

**Symptom:** Bot says "let me check" but doesn't actually fetch data

**What to look for:**
```bash
# Check if tool action was in the plan
tail -100 logs/aman.log | jq 'select(.fields.message | test("ROUTING_ACTION|use_tool"; "i"))'
```

**Common causes:**
- Router parse failure caused fallback to respond-only
- Tool action parsed but execution failed

### Issue: Wrong Brain Selected

**Symptom:** Message goes to Grok when it should go to Maple (or vice versa)

**What to look for:**
```bash
# Check routing decision
tail -100 logs/aman.log | jq 'select(.fields.message | test("BRAIN_REQUEST|Generating response"; "i"))'
```

**Key fields:**
- `sensitivity`: `Sensitive` → Maple, `Insensitive` → Grok
- `brain`: Which brain was actually used

### Issue: Slow Responses

**What to look for:**
```bash
# Check timestamps between request and response
tail -100 logs/aman.log | jq 'select(.fields.message | test("BRAIN_REQUEST|BRAIN_RESPONSE"; "i")) | {t: .timestamp, msg: .fields.message}'
```

**Common causes:**
- Slow API response from Maple/Grok
- Multiple tool execution rounds

## Debugging Session Workflow

### 1. Start the Bot with Logging

```bash
# Build and run with logging
AMAN_LOG_FILE=logs/aman.log ./scripts/dev.sh --build
```

### 2. Open Log Monitor

```bash
# In a separate terminal
tail -f logs/aman.log | jq -c 'select(.fields.message | test("INBOUND|ROUTER|BRAIN"; "i"))'
```

### 3. Send Test Message

Send a message to the bot via Signal and watch the logs flow.

### 4. Identify Issues

Look for:
- `WARN` or `ERROR` level events
- `PARSE_FAILED` messages
- Unexpected `sensitivity` or `brain` values
- Missing expected events (e.g., no `BRAIN_REQUEST` after routing)

### 5. Examine Full Payload

When you spot an issue, get the full details:
```bash
# Get last N lines with full JSON
tail -50 logs/aman.log | jq 'select(.fields.message == "ROUTER_RAW_RESPONSE")'
```

## Log Analysis Scripts

### Count Events by Type

```bash
cat logs/aman.log | jq -r '.fields.message' | sort | uniq -c | sort -rn | head -20
```

### Find All Parse Failures

```bash
grep "PARSE_FAILED" logs/aman.log | jq -r '.fields.raw_response'
```

### Get Message Flow for a Sender

```bash
SENDER="user-uuid-here"
cat logs/aman.log | jq "select(.fields.sender == \"$SENDER\" or .fields.request_text != null)"
```

### Export Debug Session

```bash
# Save filtered logs for sharing
tail -500 logs/aman.log | jq 'select(.level | test("INFO|WARN|ERROR"))' > debug-session.json
```

## Clearing Logs

```bash
# Truncate log file (bot can keep running)
> logs/aman.log

# Or remove and let it recreate
rm logs/aman.log
```

## Troubleshooting Log Setup

### Logs Not Being Written

1. Check the directory exists: `ls -la logs/`
2. Check file permissions: `touch logs/test.log`
3. Verify env var is set: `echo $AMAN_LOG_FILE`

### JSON Parse Errors in jq

If `jq` fails, there might be non-JSON output mixed in:
```bash
# Skip invalid lines
tail -f logs/aman.log | while read line; do echo "$line" | jq -c . 2>/dev/null; done
```

### Log File Growing Too Large

```bash
# Check size
du -h logs/aman.log

# Rotate manually
mv logs/aman.log logs/aman.log.bak
# Bot will create new file automatically
```

## See Also

- [Architecture](ARCHITECTURE.md) - System design and message flow
- [Adding Tools](ADDING_TOOLS.md) - How to add new tools
- [Adding Actions](ADDING_ACTIONS.md) - How to add new orchestrator actions
