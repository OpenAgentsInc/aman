use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection};

use crate::Error;

#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryProjectionStats {
    pub preferences: usize,
    pub summaries: usize,
    pub tool_history: usize,
    pub clear_events: usize,
}

pub fn project_memory(nostr_db: &Path, aman_db: &Path) -> Result<MemoryProjectionStats, Error> {
    let nostr_conn = Connection::open(nostr_db)?;
    let aman_conn = Connection::open(aman_db)?;
    project_memory_connections(&nostr_conn, &aman_conn)
}

fn project_memory_connections(
    nostr_conn: &Connection,
    aman_conn: &Connection,
) -> Result<MemoryProjectionStats, Error> {
    let latest_clear = load_latest_clear(nostr_conn)?;
    let history_keys_tool = load_history_keys(nostr_conn, "nostr_memory_tool_history")?;
    let history_keys_summary = load_history_keys(nostr_conn, "nostr_memory_summaries")?;
    let history_keys_clear = load_history_keys(nostr_conn, "nostr_memory_clear_events")?;

    for history_key in history_keys_clear.iter() {
        aman_conn.execute(
            "DELETE FROM clear_context_events WHERE history_key = ?1",
            params![history_key],
        )?;
        aman_conn.execute(
            "DELETE FROM conversation_summaries WHERE history_key = ?1",
            params![history_key],
        )?;
        aman_conn.execute(
            "DELETE FROM tool_history WHERE history_key = ?1",
            params![history_key],
        )?;
    }

    for history_key in history_keys_tool.difference(&history_keys_clear) {
        aman_conn.execute(
            "DELETE FROM tool_history WHERE history_key = ?1",
            params![history_key],
        )?;
    }

    for history_key in history_keys_summary.difference(&history_keys_clear) {
        aman_conn.execute(
            "DELETE FROM conversation_summaries WHERE history_key = ?1",
            params![history_key],
        )?;
    }

    let mut stats = MemoryProjectionStats::default();
    stats.preferences = project_preferences(nostr_conn, aman_conn)?;
    stats.summaries = project_summaries(nostr_conn, aman_conn, &latest_clear)?;
    stats.tool_history = project_tool_history(nostr_conn, aman_conn, &latest_clear)?;
    stats.clear_events = project_clear_events(nostr_conn, aman_conn)?;

    Ok(stats)
}

fn load_latest_clear(conn: &Connection) -> Result<HashMap<String, i64>, Error> {
    let mut stmt = conn.prepare(
        "SELECT history_key, MAX(created_at) \
         FROM nostr_memory_clear_events \
         GROUP BY history_key",
    )?;
    let mut rows = stmt.query([])?;
    let mut latest = HashMap::new();
    while let Some(row) = rows.next()? {
        let history_key: String = row.get(0)?;
        let created_at: i64 = row.get(1)?;
        latest.insert(history_key, created_at);
    }
    Ok(latest)
}

fn load_history_keys(conn: &Connection, table: &str) -> Result<HashSet<String>, Error> {
    let mut stmt = conn.prepare(&format!("SELECT DISTINCT history_key FROM {table}"))?;
    let mut rows = stmt.query([])?;
    let mut keys = HashSet::new();
    while let Some(row) = rows.next()? {
        let history_key: String = row.get(0)?;
        keys.insert(history_key);
    }
    Ok(keys)
}

fn project_preferences(conn: &Connection, aman: &Connection) -> Result<usize, Error> {
    let mut stmt = conn.prepare(
        "SELECT history_key, preference, updated_at, nostr_event_id, nostr_created_at, nostr_relay, schema_version \
         FROM nostr_memory_preferences",
    )?;
    let mut rows = stmt.query([])?;
    let mut count = 0usize;
    while let Some(row) = rows.next()? {
        let history_key: String = row.get(0)?;
        let preference: String = row.get(1)?;
        let updated_at: i64 = row.get(2)?;
        let event_id: String = row.get(3)?;
        let nostr_created_at: i64 = row.get(4)?;
        let nostr_relay: Option<String> = row.get(5)?;
        let schema_version: i64 = row.get(6)?;

        aman.execute(
            "INSERT INTO preferences \
                (history_key, preference, updated_at, nostr_event_id, nostr_created_at, nostr_relay, nostr_schema_version) \
             VALUES (?1, ?2, datetime(?3, 'unixepoch'), ?4, ?5, ?6, ?7) \
             ON CONFLICT(history_key) DO UPDATE SET \
                preference = excluded.preference, \
                updated_at = excluded.updated_at, \
                nostr_event_id = excluded.nostr_event_id, \
                nostr_created_at = excluded.nostr_created_at, \
                nostr_relay = excluded.nostr_relay, \
                nostr_schema_version = excluded.nostr_schema_version",
            params![
                history_key,
                preference,
                updated_at,
                event_id,
                nostr_created_at,
                nostr_relay,
                schema_version
            ],
        )?;
        count += 1;
    }
    Ok(count)
}

fn project_summaries(
    conn: &Connection,
    aman: &Connection,
    latest_clear: &HashMap<String, i64>,
) -> Result<usize, Error> {
    let mut stmt = conn.prepare(
        "SELECT history_key, summary, message_count, updated_at, nostr_event_id, nostr_created_at, nostr_relay, schema_version \
         FROM nostr_memory_summaries",
    )?;
    let mut rows = stmt.query([])?;
    let mut count = 0usize;
    while let Some(row) = rows.next()? {
        let history_key: String = row.get(0)?;
        let summary: String = row.get(1)?;
        let message_count: i64 = row.get(2)?;
        let updated_at: i64 = row.get(3)?;
        let event_id: String = row.get(4)?;
        let nostr_created_at: i64 = row.get(5)?;
        let nostr_relay: Option<String> = row.get(6)?;
        let schema_version: i64 = row.get(7)?;

        if let Some(clear_at) = latest_clear.get(&history_key) {
            if updated_at <= *clear_at {
                continue;
            }
        }

        aman.execute(
            "INSERT INTO conversation_summaries \
                (history_key, summary, message_count, updated_at, nostr_event_id, nostr_created_at, nostr_relay, nostr_schema_version) \
             VALUES (?1, ?2, ?3, datetime(?4, 'unixepoch'), ?5, ?6, ?7, ?8) \
             ON CONFLICT(history_key) DO UPDATE SET \
                summary = excluded.summary, \
                message_count = excluded.message_count, \
                updated_at = excluded.updated_at, \
                nostr_event_id = excluded.nostr_event_id, \
                nostr_created_at = excluded.nostr_created_at, \
                nostr_relay = excluded.nostr_relay, \
                nostr_schema_version = excluded.nostr_schema_version",
            params![
                history_key,
                summary,
                message_count,
                updated_at,
                event_id,
                nostr_created_at,
                nostr_relay,
                schema_version
            ],
        )?;
        count += 1;
    }
    Ok(count)
}

fn project_tool_history(
    conn: &Connection,
    aman: &Connection,
    latest_clear: &HashMap<String, i64>,
) -> Result<usize, Error> {
    let mut stmt = conn.prepare(
        "SELECT history_key, tool_name, success, content, sender_id, group_id, created_at, nostr_event_id, nostr_created_at, nostr_relay, schema_version \
         FROM nostr_memory_tool_history \
         ORDER BY created_at ASC",
    )?;
    let mut rows = stmt.query([])?;
    let mut count = 0usize;
    while let Some(row) = rows.next()? {
        let history_key: String = row.get(0)?;
        let tool_name: String = row.get(1)?;
        let success: i64 = row.get(2)?;
        let content: String = row.get(3)?;
        let sender_id: Option<String> = row.get(4)?;
        let group_id: Option<String> = row.get(5)?;
        let created_at: i64 = row.get(6)?;
        let event_id: String = row.get(7)?;
        let nostr_created_at: i64 = row.get(8)?;
        let nostr_relay: Option<String> = row.get(9)?;
        let schema_version: i64 = row.get(10)?;

        if let Some(clear_at) = latest_clear.get(&history_key) {
            if created_at <= *clear_at {
                continue;
            }
        }

        aman.execute(
            "INSERT INTO tool_history \
                (history_key, tool_name, success, content, sender_id, group_id, created_at, nostr_event_id, nostr_created_at, nostr_relay, nostr_schema_version) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime(?7, 'unixepoch'), ?8, ?9, ?10, ?11) \
             ON CONFLICT(nostr_event_id) DO NOTHING",
            params![
                history_key,
                tool_name,
                success,
                content,
                sender_id,
                group_id,
                created_at,
                event_id,
                nostr_created_at,
                nostr_relay,
                schema_version
            ],
        )?;
        count += 1;
    }
    Ok(count)
}

fn project_clear_events(conn: &Connection, aman: &Connection) -> Result<usize, Error> {
    let mut stmt = conn.prepare(
        "SELECT history_key, sender_id, created_at, nostr_event_id, nostr_created_at, nostr_relay, schema_version \
         FROM nostr_memory_clear_events \
         ORDER BY created_at ASC",
    )?;
    let mut rows = stmt.query([])?;
    let mut count = 0usize;
    while let Some(row) = rows.next()? {
        let history_key: String = row.get(0)?;
        let sender_id: Option<String> = row.get(1)?;
        let created_at: i64 = row.get(2)?;
        let event_id: String = row.get(3)?;
        let nostr_created_at: i64 = row.get(4)?;
        let nostr_relay: Option<String> = row.get(5)?;
        let schema_version: i64 = row.get(6)?;

        aman.execute(
            "INSERT INTO clear_context_events \
                (history_key, sender_id, created_at, nostr_event_id, nostr_created_at, nostr_relay, nostr_schema_version) \
             VALUES (?1, ?2, datetime(?3, 'unixepoch'), ?4, ?5, ?6, ?7) \
             ON CONFLICT(nostr_event_id) DO NOTHING",
            params![
                history_key,
                sender_id,
                created_at,
                event_id,
                nostr_created_at,
                nostr_relay,
                schema_version
            ],
        )?;
        count += 1;
    }
    Ok(count)
}
