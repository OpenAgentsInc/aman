//! Durable memory helpers for the orchestrator.

use std::env;
use std::time::Duration;

use database::{
    clear_context_event, conversation_summary, tool_history, ConversationSummary, Database,
};

/// Summary formatting policy.
#[derive(Debug, Clone)]
pub struct SummaryPolicy {
    pub max_entries: usize,
    pub max_entry_chars: usize,
    pub max_summary_chars: usize,
}

impl Default for SummaryPolicy {
    fn default() -> Self {
        Self {
            max_entries: 8,
            max_entry_chars: 160,
            max_summary_chars: 1200,
        }
    }
}

/// Retention policy for memory tables.
#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    pub summary_ttl: Option<Duration>,
    pub tool_history_ttl: Option<Duration>,
    pub clear_context_ttl: Option<Duration>,
    pub max_summaries: Option<usize>,
    pub max_tool_history_total: Option<usize>,
    pub max_tool_history_per_key: Option<usize>,
    pub max_clear_context_events: Option<usize>,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            summary_ttl: Some(Duration::from_secs(30 * 24 * 60 * 60)),
            tool_history_ttl: Some(Duration::from_secs(14 * 24 * 60 * 60)),
            clear_context_ttl: Some(Duration::from_secs(30 * 24 * 60 * 60)),
            max_summaries: Some(5_000),
            max_tool_history_total: Some(10_000),
            max_tool_history_per_key: Some(200),
            max_clear_context_events: Some(5_000),
        }
    }
}

/// Memory settings for summaries and tool history.
#[derive(Debug, Clone)]
pub struct MemorySettings {
    pub summary: SummaryPolicy,
    pub retention: RetentionPolicy,
    pub tool_output_max_chars: usize,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            summary: SummaryPolicy::default(),
            retention: RetentionPolicy::default(),
            tool_output_max_chars: 2000,
        }
    }
}

impl MemorySettings {
    /// Load memory settings from environment variables.
    pub fn from_env() -> Self {
        let mut settings = Self::default();

        if let Some(value) = env_usize("AMAN_MEMORY_SUMMARY_MAX_ENTRIES") {
            settings.summary.max_entries = value;
        }
        if let Some(value) = env_usize("AMAN_MEMORY_SUMMARY_MAX_ENTRY_CHARS") {
            settings.summary.max_entry_chars = value;
        }
        if let Some(value) = env_usize("AMAN_MEMORY_SUMMARY_MAX_CHARS") {
            settings.summary.max_summary_chars = value;
        }
        if let Some(value) = env_usize("AMAN_MEMORY_TOOL_OUTPUT_MAX_CHARS") {
            settings.tool_output_max_chars = value;
        }

        if let Some(days) = env_u64("AMAN_MEMORY_SUMMARY_TTL_DAYS") {
            settings.retention.summary_ttl = days_to_duration(days);
        }
        if let Some(days) = env_u64("AMAN_MEMORY_TOOL_TTL_DAYS") {
            settings.retention.tool_history_ttl = days_to_duration(days);
        }
        if let Some(days) = env_u64("AMAN_MEMORY_CLEAR_TTL_DAYS") {
            settings.retention.clear_context_ttl = days_to_duration(days);
        }

        if let Some(value) = env_usize("AMAN_MEMORY_MAX_SUMMARIES") {
            settings.retention.max_summaries = cap_from_env(value);
        }
        if let Some(value) = env_usize("AMAN_MEMORY_MAX_TOOL_HISTORY") {
            settings.retention.max_tool_history_total = cap_from_env(value);
        }
        if let Some(value) = env_usize("AMAN_MEMORY_MAX_TOOL_HISTORY_PER_KEY") {
            settings.retention.max_tool_history_per_key = cap_from_env(value);
        }
        if let Some(value) = env_usize("AMAN_MEMORY_MAX_CLEAR_EVENTS") {
            settings.retention.max_clear_context_events = cap_from_env(value);
        }

        settings
    }
}

/// Durable memory store backed by SQLite.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    database: Database,
    settings: MemorySettings,
}

impl MemoryStore {
    pub fn new(database: Database, settings: MemorySettings) -> Self {
        Self { database, settings }
    }

    pub fn settings(&self) -> &MemorySettings {
        &self.settings
    }

    pub async fn get_summary(&self, history_key: &str) -> Option<String> {
        let record = conversation_summary::get_summary(self.database.pool(), history_key)
            .await
            .ok()
            .flatten();
        record.map(|row| row.summary)
    }

    pub async fn record_exchange(
        &self,
        history_key: &str,
        user_text: &str,
        assistant_text: &str,
    ) -> database::Result<()> {
        let existing = conversation_summary::get_summary(self.database.pool(), history_key).await?;
        let (summary, message_count) =
            self.build_summary(existing.as_ref(), user_text, assistant_text);

        conversation_summary::upsert_summary(
            self.database.pool(),
            history_key,
            &summary,
            message_count,
        )
        .await?;

        self.prune(history_key).await?;
        Ok(())
    }

    pub async fn clear_context(
        &self,
        history_key: &str,
        sender_id: Option<&str>,
    ) -> database::Result<()> {
        conversation_summary::clear_summary(self.database.pool(), history_key).await?;
        clear_context_event::insert_event(self.database.pool(), history_key, sender_id).await?;
        self.prune(history_key).await?;
        Ok(())
    }

    pub async fn record_tool(
        &self,
        history_key: &str,
        tool_name: &str,
        success: bool,
        content: &str,
        sender_id: Option<&str>,
        group_id: Option<&str>,
    ) -> database::Result<()> {
        let content = truncate_text(content, self.settings.tool_output_max_chars);
        tool_history::insert_tool_history(
            self.database.pool(),
            history_key,
            tool_name,
            success,
            &content,
            sender_id,
            group_id,
        )
        .await?;

        self.prune(history_key).await?;
        Ok(())
    }

    async fn prune(&self, history_key: &str) -> database::Result<()> {
        if let Some(ttl) = self.settings.retention.summary_ttl {
            let _ = conversation_summary::prune_older_than(self.database.pool(), ttl).await?;
        }
        if let Some(max_rows) = self.settings.retention.max_summaries {
            let _ = conversation_summary::prune_over_limit(self.database.pool(), max_rows).await?;
        }

        if let Some(ttl) = self.settings.retention.tool_history_ttl {
            let _ = tool_history::prune_older_than(self.database.pool(), ttl).await?;
        }
        if let Some(max_rows) = self.settings.retention.max_tool_history_total {
            let _ = tool_history::prune_over_limit(self.database.pool(), max_rows).await?;
        }
        if let Some(max_rows) = self.settings.retention.max_tool_history_per_key {
            let _ =
                tool_history::prune_over_limit_for_key(self.database.pool(), history_key, max_rows)
                    .await?;
        }

        if let Some(ttl) = self.settings.retention.clear_context_ttl {
            let _ = clear_context_event::prune_older_than(self.database.pool(), ttl).await?;
        }
        if let Some(max_rows) = self.settings.retention.max_clear_context_events {
            let _ = clear_context_event::prune_over_limit(self.database.pool(), max_rows).await?;
        }

        Ok(())
    }

    fn build_summary(
        &self,
        existing: Option<&ConversationSummary>,
        user_text: &str,
        assistant_text: &str,
    ) -> (String, i64) {
        let mut lines: Vec<String> = existing
            .map(|row| row.summary.lines().map(|line| line.to_string()).collect())
            .unwrap_or_default();
        let message_count = existing.map(|row| row.message_count).unwrap_or(0);

        let user_line = format!(
            "U: {}",
            truncate_text(&collapse_lines(user_text), self.settings.summary.max_entry_chars)
        );
        let assistant_line = format!(
            "A: {}",
            truncate_text(&collapse_lines(assistant_text), self.settings.summary.max_entry_chars)
        );
        lines.push(user_line);
        lines.push(assistant_line);

        let max_entries = self.settings.summary.max_entries;
        if max_entries > 0 {
            let max_lines = max_entries.saturating_mul(2);
            if lines.len() > max_lines {
                let trim = lines.len() - max_lines;
                lines.drain(0..trim);
            }
        }

        let mut summary = lines.join("\n");
        let max_chars = self.settings.summary.max_summary_chars;
        if max_chars > 0 && summary.len() > max_chars {
            while summary.len() > max_chars && lines.len() > 2 {
                lines.drain(0..2);
                summary = lines.join("\n");
            }

            if summary.len() > max_chars {
                summary = truncate_text(&summary, max_chars);
            }
        }

        (summary, message_count + 1)
    }
}

fn env_usize(key: &str) -> Option<usize> {
    env::var(key).ok()?.parse().ok()
}

fn env_u64(key: &str) -> Option<u64> {
    env::var(key).ok()?.parse().ok()
}

fn days_to_duration(days: u64) -> Option<Duration> {
    if days == 0 {
        None
    } else {
        Some(Duration::from_secs(days.saturating_mul(24 * 60 * 60)))
    }
}

fn cap_from_env(value: usize) -> Option<usize> {
    if value == 0 { None } else { Some(value) }
}

fn collapse_lines(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let total_chars = text.chars().count();
    if total_chars <= max_chars {
        return text.to_string();
    }

    let ellipsis = "...";
    let available = max_chars.saturating_sub(ellipsis.len());
    let mut output: String = text.chars().take(available).collect();
    if output.is_empty() {
        output = text.chars().take(max_chars).collect();
        return output;
    }
    output.push_str(ellipsis);
    output
}
