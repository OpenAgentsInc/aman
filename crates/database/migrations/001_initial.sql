-- Users table
-- id: Signal UUID (e.g., c27fb365-0c84-4cf2-8555-814bb065e448)
-- name: Display name
-- language: Preferred language (e.g., "Arabic", "English")
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    language TEXT NOT NULL DEFAULT 'English'
);

-- Topics lookup table
-- slug: Unique identifier (e.g., "iran", "bitcoin", "vpn+iran")
CREATE TABLE IF NOT EXISTS topics (
    slug TEXT PRIMARY KEY NOT NULL
);

-- Notifications (subscriptions)
-- Links users to topics they want notifications for
CREATE TABLE IF NOT EXISTS notifications (
    topic_slug TEXT NOT NULL,
    user_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (topic_slug, user_id),
    FOREIGN KEY (topic_slug) REFERENCES topics(slug) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Index for querying subscriptions by user
CREATE INDEX IF NOT EXISTS idx_notifications_user_id ON notifications(user_id);

-- Seed initial topics
INSERT OR IGNORE INTO topics (slug) VALUES
    ('iran'),
    ('uganda'),
    ('venezuela'),
    ('bitcoin'),
    ('vpn+iran');
