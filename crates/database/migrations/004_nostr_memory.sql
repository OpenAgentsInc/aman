ALTER TABLE preferences ADD COLUMN nostr_event_id TEXT;
ALTER TABLE preferences ADD COLUMN nostr_created_at INTEGER;
ALTER TABLE preferences ADD COLUMN nostr_relay TEXT;
ALTER TABLE preferences ADD COLUMN nostr_schema_version INTEGER;

ALTER TABLE conversation_summaries ADD COLUMN nostr_event_id TEXT;
ALTER TABLE conversation_summaries ADD COLUMN nostr_created_at INTEGER;
ALTER TABLE conversation_summaries ADD COLUMN nostr_relay TEXT;
ALTER TABLE conversation_summaries ADD COLUMN nostr_schema_version INTEGER;

ALTER TABLE tool_history ADD COLUMN nostr_event_id TEXT;
ALTER TABLE tool_history ADD COLUMN nostr_created_at INTEGER;
ALTER TABLE tool_history ADD COLUMN nostr_relay TEXT;
ALTER TABLE tool_history ADD COLUMN nostr_schema_version INTEGER;

ALTER TABLE clear_context_events ADD COLUMN nostr_event_id TEXT;
ALTER TABLE clear_context_events ADD COLUMN nostr_created_at INTEGER;
ALTER TABLE clear_context_events ADD COLUMN nostr_relay TEXT;
ALTER TABLE clear_context_events ADD COLUMN nostr_schema_version INTEGER;

CREATE UNIQUE INDEX IF NOT EXISTS idx_tool_history_nostr_event_id
    ON tool_history(nostr_event_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_clear_context_events_nostr_event_id
    ON clear_context_events(nostr_event_id);
