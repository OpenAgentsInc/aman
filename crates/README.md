# Aman crates

This directory contains the Rust crates that make up Aman. Each crate has its own README
with configuration and usage details.

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
| `database` | SQLite persistence (users/topics/notifications) via SQLx |
| `api` | OpenAI-compatible chat API (local inference gateway) |
| `ingester` | Document chunking and Nostr publishing/indexing |
| `nostr-persistence` | Nostr publisher/indexer for durable doc/chunk metadata |
| `admin-web` | Admin dashboard and broadcast UI |
