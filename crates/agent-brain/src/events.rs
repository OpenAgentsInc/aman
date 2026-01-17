//! Regional alert event types.

use serde::{Deserialize, Serialize};

/// A normalized regional event for alert fanout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionEvent {
    pub region: String,
    pub kind: String,
    pub severity: String,
    pub confidence: String,
    pub summary: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub ts: Option<String>,
}

impl RegionEvent {
    /// Render the event into a short alert message.
    pub fn render_alert(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "{} alert ({}, {} confidence)",
            self.region, self.severity, self.confidence
        ));
        lines.push(self.summary.clone());

        if let Some(ref ts) = self.ts {
            lines.push(format!("Time: {}", ts));
        }

        if !self.source_refs.is_empty() {
            let sources = self.source_refs.join(", ");
            lines.push(format!("Sources: {}", sources));
        }

        lines.join("\n")
    }
}
