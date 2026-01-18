# Aman crates

This directory contains the Rust crates that make up Aman. Each crate has its own README
with configuration and usage details.

## Core Crates (Production-ready)

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
| `api` | OpenAI-compatible chat API (echo/orchestrator/OpenRouter with KB injection) |
| `ingester` | Document chunking and Nostr publishing/indexing |
| `nostr-persistence` | Nostr publisher/indexer for durable doc/chunk metadata |
| `admin-web` | Admin dashboard and broadcast UI |

## Supporting Crates

| Crate | Description |
| --- | --- |
| `proton-proxy` | SMTP client for Proton Mail Bridge (E2E encrypted email) |
