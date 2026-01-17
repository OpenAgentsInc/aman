# Aman

Aman is a **Signal-native chatbot** that runs on a server using `signal-cli`. It receives incoming Signal messages, sends them to an **OpenAI-compatible API endpoint**, and replies back to the sender on Signal.

This is the MVP phase: **no web UI, no document upload, no RAG** yet.

---

## What it does

* Runs a Signal account (phone number) on a server via `signal-cli`
* Listens for inbound messages to that account (“Aman”)
* For each message:

  * builds a prompt (optionally with short conversation context)
  * calls an OpenAI-compatible **Responses API** endpoint
  * sends the model output back as a Signal message

---

## Architecture

```
Signal User  <—E2EE—>  Aman Signal Account (signal-cli on server)
                                   |
                                   v
                           Bot Worker (queue + state)
                                   |
                                   v
                       OpenAI-compatible API (Responses)
                                   |
                                   v
                             signal-cli send reply
```

---

## Prerequisites

### Server

* Linux server with systemd (recommended) or Docker
* Reliable network connectivity

### Signal

* `signal-cli` installed on the server
* A phone number that can receive SMS/voice for registration

  * (Optional) “silent SIM / hosted number” provider if you don’t want to use a personal number

### OpenAI-compatible API

* An API key for your OpenAI-compatible provider set as an environment variable `OPENAI_API_KEY` (example docs: [OpenAI Platform][1])

---

## Install

1. Install `signal-cli` on the server
   (Use your preferred package manager / official instructions for your distro.)

2. Clone this repo and install dependencies for the implementation you’re using.

---

## Configure

Create a `.env`:

```bash
OPENAI_API_KEY="..."
AMAN_NUMBER="+15551234567"
SIGNAL_CLI_PATH="/usr/local/bin/signal-cli"
# Optional:
MODEL="gpt-5"
STORE_OPENAI_RESPONSES="false"
SQLITE_PATH="./data/aman.sqlite"
```

Notes:

* The Responses API supports `store: false` to reduce server-side “application state” storage. (example docs: [OpenAI Platform][2])
* The API key should never be placed in client-side code; keep it server-side only. (example docs: [OpenAI Platform][3])

---

## Register Aman on Signal (signal-cli)

You need to link `signal-cli` to Aman’s phone number once.

Typical flow:

1. Register the number (SMS or voice verification)
2. Verify the registration code
3. Confirm `signal-cli` can send/receive with that identity

> Exact commands vary by `signal-cli` version and how you installed it. Keep registration steps in an internal ops doc for your team.

---

## How conversation context works (MVP)

Aman can be:

* **Stateless**: each message answered independently (safest)
* **Short-context**: include last N turns from local SQLite (better UX)
* **Summarized**: keep a rolling summary per user (best balance)

For closed-society / high-risk contexts, default to:

* minimal retention
* short-context or summary
* aggressive log scrubbing

---

## Safety & privacy defaults (recommended)

* **Minimize logs**: do not log message bodies by default
* **Short retention**: store only what you need to dedupe/reply
* **Rate limits**: prevent spam loops and abuse
* **No attachments (yet)**: MVP is text-only
* **Explicit warnings**: users should assume the server is a trusted endpoint (Signal is E2EE to the server, not beyond it)

API storage note:

* Responses are stored by default unless you disable storage; use `store: false` when you don’t want application state retained. (example docs: [OpenAI Platform][2])

---

## Roadmap (next phases)

1. **Document upload + RAG**

* ingestion pipeline (chunking, embeddings)
* citations/snippets in replies
* safe storage strategy

2. **Web UI (optional)**

* richer upload/browse experience
* Signal used for login/notifications

3. **Nostr relay persistence**

* store encrypted doc manifests + metadata events
* allow rehydration/sync across agent instances
* keep vector search local for performance (materialize index from relay log)
