-- Preference storage for routing decisions.
CREATE TABLE IF NOT EXISTS preferences (
    history_key TEXT PRIMARY KEY NOT NULL,
    preference TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Conversation summaries for routing context.
CREATE TABLE IF NOT EXISTS conversation_summaries (
    history_key TEXT PRIMARY KEY NOT NULL,
    summary TEXT NOT NULL,
    message_count INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Tool execution history.
CREATE TABLE IF NOT EXISTS tool_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    history_key TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    success INTEGER NOT NULL,
    content TEXT NOT NULL,
    sender_id TEXT,
    group_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_tool_history_history_key ON tool_history(history_key);
CREATE INDEX IF NOT EXISTS idx_tool_history_created_at ON tool_history(created_at);

-- Clear context events for audit and retention.
CREATE TABLE IF NOT EXISTS clear_context_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    history_key TEXT NOT NULL,
    sender_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_clear_context_history_key ON clear_context_events(history_key);
CREATE INDEX IF NOT EXISTS idx_clear_context_created_at ON clear_context_events(created_at);
