-- User profile settings (individual users only, keyed by sender_id)
CREATE TABLE IF NOT EXISTS user_profiles (
    sender_id TEXT PRIMARY KEY NOT NULL,
    default_model TEXT,
    email TEXT,
    bolt12_offer TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_user_profiles_email ON user_profiles(email);
