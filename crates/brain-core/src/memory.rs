//! Durable memory contracts shared across brains and orchestrators.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Snapshot of durable memory for a sender or group.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemorySnapshot {
    /// Rolling summary of recent conversation context.
    pub summary: Option<String>,
    /// Timestamp of the summary update (provider-defined format).
    pub summary_updated_at: Option<String>,
    /// Tool history entries.
    pub tool_history: Vec<MemoryToolEntry>,
    /// Clear-context events (most recent first).
    pub clear_context_events: Vec<MemoryClearEvent>,
}

impl MemorySnapshot {
    /// Check whether the snapshot contains any usable memory.
    pub fn is_empty(&self) -> bool {
        let summary_empty = self
            .summary
            .as_ref()
            .map(|text| text.trim().is_empty())
            .unwrap_or(true);
        summary_empty && self.tool_history.is_empty() && self.clear_context_events.is_empty()
    }
}

/// Tool history entry included in a memory snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryToolEntry {
    /// Tool name executed.
    pub tool: String,
    /// Whether the tool succeeded.
    pub success: bool,
    /// Tool output content (possibly truncated).
    pub content: String,
    /// Creation timestamp (provider-defined format).
    pub created_at: Option<String>,
}

/// Clear-context event included in a memory snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryClearEvent {
    /// Creation timestamp (provider-defined format).
    pub created_at: Option<String>,
    /// Sender identifier, if available.
    pub sender_id: Option<String>,
}

/// PII handling policy for memory prompts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPiiPolicy {
    /// Include memory as-is.
    Allow,
    /// Apply lightweight redaction to memory content.
    Redact,
    /// Skip memory injection entirely.
    Skip,
}

/// Policy for formatting memory into a prompt-safe context block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPromptPolicy {
    /// Maximum characters for the entire memory prompt (0 disables).
    pub max_chars: usize,
    /// Maximum characters from the summary.
    pub max_summary_chars: usize,
    /// Maximum tool history entries.
    pub max_tool_entries: usize,
    /// Maximum characters per tool entry.
    pub max_tool_entry_chars: usize,
    /// Maximum clear-context events to include.
    pub max_clear_events: usize,
    /// Whether to include the summary section.
    pub include_summary: bool,
    /// Whether to include tool history entries.
    pub include_tool_history: bool,
    /// Whether to include clear-context events.
    pub include_clear_context: bool,
    /// PII handling policy.
    pub pii_policy: MemoryPiiPolicy,
}

impl Default for MemoryPromptPolicy {
    fn default() -> Self {
        Self {
            max_chars: 1800,
            max_summary_chars: 1000,
            max_tool_entries: 3,
            max_tool_entry_chars: 280,
            max_clear_events: 2,
            include_summary: true,
            include_tool_history: true,
            include_clear_context: true,
            pii_policy: MemoryPiiPolicy::Allow,
        }
    }
}

/// Errors returned by memory stores.
#[derive(Debug, Error)]
pub enum MemoryError {
    /// Storage or retrieval failure.
    #[error("memory store error: {0}")]
    Store(String),
}

/// A durable memory store for conversation summaries and tool history.
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Load a snapshot of memory for a sender/group history key.
    async fn snapshot(&self, history_key: &str) -> Result<MemorySnapshot, MemoryError>;
}

/// Format a memory snapshot into a prompt-safe context block.
pub fn format_memory_prompt(
    snapshot: &MemorySnapshot,
    policy: &MemoryPromptPolicy,
) -> Option<String> {
    if policy.max_chars == 0 || policy.pii_policy == MemoryPiiPolicy::Skip {
        return None;
    }
    if snapshot.is_empty() {
        return None;
    }

    let mut sections: Vec<String> = Vec::new();

    if policy.include_summary {
        if let Some(summary) = snapshot.summary.as_ref() {
            let mut text = summary.trim().to_string();
            if !text.is_empty() {
                text = apply_pii_policy(&text, policy.pii_policy);
                text = truncate_text(&text, policy.max_summary_chars);
                let mut section = String::new();
                section.push_str("[SUMMARY]\n");
                if let Some(updated_at) = snapshot.summary_updated_at.as_ref() {
                    section.push_str(&format!("Updated: {}\n", updated_at));
                }
                section.push_str(&text);
                sections.push(section);
            }
        }
    }

    if policy.include_tool_history && policy.max_tool_entries > 0 && !snapshot.tool_history.is_empty()
    {
        let mut section = String::new();
        section.push_str("[TOOLS]\n");
        for (idx, entry) in snapshot
            .tool_history
            .iter()
            .take(policy.max_tool_entries)
            .enumerate()
        {
            let status = if entry.success { "ok" } else { "error" };
            let timestamp = entry
                .created_at
                .as_deref()
                .unwrap_or("unknown time");
            let mut content = apply_pii_policy(&entry.content, policy.pii_policy);
            content = truncate_text(&content, policy.max_tool_entry_chars);
            section.push_str(&format!(
                "{}. {} ({}, {}): {}\n",
                idx + 1,
                entry.tool,
                status,
                timestamp,
                content
            ));
        }
        sections.push(section.trim_end().to_string());
    }

    if policy.include_clear_context
        && policy.max_clear_events > 0
        && !snapshot.clear_context_events.is_empty()
    {
        let mut section = String::new();
        section.push_str("[CLEAR CONTEXT]\n");
        for (idx, entry) in snapshot
            .clear_context_events
            .iter()
            .take(policy.max_clear_events)
            .enumerate()
        {
            let timestamp = entry
                .created_at
                .as_deref()
                .unwrap_or("unknown time");
            let sender = entry
                .sender_id
                .as_deref()
                .unwrap_or("unknown sender");
            let line = format!("{}. {} by {}", idx + 1, timestamp, sender);
            section.push_str(&format!("{}\n", apply_pii_policy(&line, policy.pii_policy)));
        }
        sections.push(section.trim_end().to_string());
    }

    if sections.is_empty() {
        return None;
    }

    let header = "[MEMORY SNAPSHOT]\nNotes from prior conversations. Use as context; do not quote as sources.\n";
    let footer = "[END MEMORY SNAPSHOT]";
    let mut body = sections.join("\n\n");
    body = body.trim().to_string();

    let mut output = format!("{}{}\n{}", header, body, footer);
    let max_chars = policy.max_chars;
    if max_chars > 0 && output.chars().count() > max_chars {
        let header_len = header.chars().count();
        let footer_len = footer.chars().count();
        let available = max_chars.saturating_sub(header_len + footer_len + 1);
        let trimmed_body = truncate_text(&body, available);
        output = format!("{}{}\n{}", header, trimmed_body, footer);
    }

    Some(output)
}

fn apply_pii_policy(text: &str, policy: MemoryPiiPolicy) -> String {
    match policy {
        MemoryPiiPolicy::Allow => text.to_string(),
        MemoryPiiPolicy::Redact => redact_pii(text),
        MemoryPiiPolicy::Skip => String::new(),
    }
}

fn redact_pii(text: &str) -> String {
    let without_emails = redact_email_tokens(text);
    redact_digit_runs(&without_emails)
}

fn redact_email_tokens(text: &str) -> String {
    let mut output = String::new();
    let mut token = String::new();

    for ch in text.chars() {
        if ch.is_whitespace() {
            output.push_str(&redact_token(&token));
            output.push(ch);
            token.clear();
        } else {
            token.push(ch);
        }
    }

    if !token.is_empty() {
        output.push_str(&redact_token(&token));
    }

    output
}

fn redact_token(token: &str) -> String {
    if token.contains('@') && token.contains('.') {
        "[REDACTED_EMAIL]".to_string()
    } else {
        token.to_string()
    }
}

fn redact_digit_runs(text: &str) -> String {
    let mut output = String::new();
    let mut digits = String::new();

    for ch in text.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            if !digits.is_empty() {
                output.push_str(&redact_digits(&digits));
                digits.clear();
            }
            output.push(ch);
        }
    }

    if !digits.is_empty() {
        output.push_str(&redact_digits(&digits));
    }

    output
}

fn redact_digits(digits: &str) -> String {
    if digits.len() >= 6 {
        "[REDACTED_NUMBER]".to_string()
    } else {
        digits.to_string()
    }
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
