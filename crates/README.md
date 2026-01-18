# Aman Crates

This directory contains all Rust crates that make up the Aman Signal chatbot. This document provides an overview of each crate and how they fit together.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         SIGNAL-DAEMON                                │
│  SignalClient (JSON-RPC) + DaemonProcess (JAR spawn) + SSE Stream   │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       MESSAGE-LISTENER                               │
│  MessageListener (event stream) + MessageProcessor (Brain adapter)  │
│  Features: timeout, graceful shutdown, attachment-only support      │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         ORCHESTRATOR                                 │
│  Router (classify) → RoutingPlan [actions] → Orchestrator (execute) │
│  Actions: search, use_tool, clear_context, respond, show_help       │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        AGENT-TOOLS                                   │
│  ToolRegistry → Calculator, Weather, WebFetch, Dictionary, etc.     │
└─────────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌─────────────────────────┐     ┌─────────────────────────┐
│      MAPLE-BRAIN        │     │      GROK-BRAIN         │
│  OpenSecret TEE         │     │  GrokToolExecutor       │
│  + tool support         │◀────│  (realtime_search)      │
│  + vision support       │     │  + GrokBrain (chat)     │
│  + ConversationHistory  │     │  + ConversationHistory  │
└─────────────────────────┘     └─────────────────────────┘
              │                               │
              └───────────────┬───────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         BRAIN-CORE                                   │
│  Brain trait + ToolExecutor + ConversationHistory + Message types   │
└─────────────────────────────────────────────────────────────────────┘
```

## Crate Categories

### Core Infrastructure

| Crate | Description |
| --- | --- |
| `signal-daemon` | Core client for signal-cli daemon (HTTP/SSE) |
| `message-listener` | Signal inbound transport using signal-daemon |
| `broadcaster` | Signal outbound delivery using signal-daemon |
| `agent-brain` | Onboarding, routing, and subscription handling |
| `mock-brain` | Mock brain implementations for testing message flows |
| `brain-core` | Shared Brain trait and message types for AI backends |
| `maple-brain` | OpenSecret-backed Brain implementation |
| `grok-brain` | xAI Grok Brain and tool executor |
| `orchestrator` | Message routing, action planning, and tool orchestration |
| `agent-tools` | Tool registry, built-in tools, and ToolExecutor adapter with policy controls |
| `database` | SQLite persistence (users/topics/notifications + preferences/summaries/tool history) |
| `api` | OpenAI-compatible chat API (local inference gateway) |
| `ingester` | Document chunking and Nostr publishing/indexing |
| `nostr-persistence` | Nostr publisher/indexer for durable doc/chunk metadata |
| `admin-web` | Admin dashboard and broadcast UI |
