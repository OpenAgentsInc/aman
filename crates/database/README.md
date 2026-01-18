# database

SQLite persistence layer for Aman using async SQLx.

## Features

- Async SQLite with connection pooling
- Built-in migrations
- CRUD operations for Users, Topics, and Notification subscriptions
- Durable memory tables for preferences, summaries, tool history, and clear-context events

## Schema

```
┌─────────────────┐       ┌─────────────────┐
│     users       │       │     topics      │
├─────────────────┤       ├─────────────────┤
│ id (PK)         │       │ slug (PK)       │
│ name            │       └────────┬────────┘
│ language        │                │
└────────┬────────┘                │
         │                         │
         │    ┌────────────────────┘
         │    │
         ▼    ▼
┌─────────────────────────┐
│     notifications       │
├─────────────────────────┤
│ user_id (FK)            │
│ topic_slug (FK)         │
│ created_at              │
└─────────────────────────┘
```

### Memory tables

```
preferences (history_key, preference, updated_at)
conversation_summaries (history_key, summary, message_count, updated_at)
tool_history (history_key, tool_name, success, content, sender_id, group_id, created_at)
clear_context_events (history_key, sender_id, created_at)
```

## Usage

```rust
use database::{Database, User, user, topic, notification};

#[tokio::main]
async fn main() -> database::Result<()> {
    // Connect and run migrations
    let db = Database::connect("sqlite:aman.db?mode=rwc").await?;
    db.migrate().await?;

    // Create a user
    let user = User {
        id: "c27fb365-0c84-4cf2-8555-814bb065e448".to_string(),
        name: "Bob".to_string(),
        language: "Arabic".to_string(),
    };
    user::create_user(db.pool(), &user).await?;

    // Subscribe user to a topic
    notification::subscribe(db.pool(), &user.id, "iran").await?;

    // Get all subscribers for a topic
    let subscribers = notification::get_topic_subscribers(db.pool(), "iran").await?;

    // Get all topics a user is subscribed to
    let topics = notification::get_user_subscriptions(db.pool(), &user.id).await?;

    Ok(())
}
```

## API

### Database

| Function | Description |
|----------|-------------|
| `Database::connect(url)` | Connect to SQLite database |
| `Database::migrate()` | Run pending migrations |
| `Database::pool()` | Get the connection pool |

### User CRUD

| Function | Description |
|----------|-------------|
| `user::create_user(pool, user)` | Create a new user |
| `user::get_user(pool, id)` | Get user by ID |
| `user::get_user_by_name(pool, name)` | Get user by name |
| `user::update_user(pool, user)` | Update user |
| `user::delete_user(pool, id)` | Delete user |
| `user::list_users(pool)` | List all users |

### Topic CRUD

| Function | Description |
|----------|-------------|
| `topic::create_topic(pool, slug)` | Create a new topic |
| `topic::get_topic(pool, slug)` | Get topic by slug |
| `topic::delete_topic(pool, slug)` | Delete topic |
| `topic::list_topics(pool)` | List all topics |

### Notification CRUD

| Function | Description |
|----------|-------------|
| `notification::subscribe(pool, user_id, topic_slug)` | Subscribe user to topic |
| `notification::unsubscribe(pool, user_id, topic_slug)` | Unsubscribe user from topic |
| `notification::is_subscribed(pool, user_id, topic_slug)` | Check if user is subscribed |
| `notification::get_user_subscriptions(pool, user_id)` | Get all topics for a user |
| `notification::get_topic_subscribers(pool, topic_slug)` | Get all users for a topic |

### Preferences

| Function | Description |
|----------|-------------|
| `preference::upsert_preference(pool, history_key, preference)` | Create or update preference |
| `preference::get_preference(pool, history_key)` | Get preference by history key |
| `preference::clear_preference(pool, history_key)` | Delete preference |
| `preference::clear_all(pool)` | Delete all preferences |

### Conversation summaries

| Function | Description |
|----------|-------------|
| `conversation_summary::upsert_summary(pool, history_key, summary, message_count)` | Create or update summary |
| `conversation_summary::get_summary(pool, history_key)` | Get summary by history key |
| `conversation_summary::clear_summary(pool, history_key)` | Delete summary |

### Tool history

| Function | Description |
|----------|-------------|
| `tool_history::insert_tool_history(pool, history_key, tool_name, success, content, sender_id, group_id)` | Record tool usage |
| `tool_history::list_tool_history(pool, history_key, limit)` | List recent tool usage |

### Clear-context events

| Function | Description |
|----------|-------------|
| `clear_context_event::insert_event(pool, history_key, sender_id)` | Record clear context event |
| `clear_context_event::list_events(pool, history_key, limit)` | List recent clear context events |

## Default Topics

The initial migration seeds these topics:

- `iran`
- `syria`
- `lebanon`
- `uganda`
- `venezuela`
- `bitcoin`
- `vpn+iran`

## Testing

```bash
cargo test -p database
```
