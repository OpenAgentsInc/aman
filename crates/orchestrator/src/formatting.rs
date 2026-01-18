//! Markdown to Signal formatting conversion.
//!
//! Converts common markdown syntax to Signal's text style format.
//! Signal uses "body ranges" to specify formatting, not inline syntax.

use brain_core::TextStyle;

/// A formatted message ready to send via Signal.
#[derive(Debug, Clone, Default)]
pub struct FormattedMessage {
    /// Plain text with markdown markers removed.
    pub text: String,
    /// Text style ranges for formatting.
    pub styles: Vec<TextStyle>,
}

impl FormattedMessage {
    /// Create a new formatted message with just text (no styles).
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            styles: Vec::new(),
        }
    }

    /// Check if this message has any formatting.
    pub fn has_styles(&self) -> bool {
        !self.styles.is_empty()
    }
}

/// Signal text style types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleType {
    Bold,
    Italic,
    Monospace,
    Strikethrough,
}

impl StyleType {
    /// Get the Signal style name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Bold => "BOLD",
            Self::Italic => "ITALIC",
            Self::Monospace => "MONOSPACE",
            Self::Strikethrough => "STRIKETHROUGH",
        }
    }
}

/// A detected formatting span in the source text.
#[derive(Debug, Clone)]
struct FormatSpan {
    /// Start position in source text (including markers).
    start: usize,
    /// End position in source text (including markers).
    end: usize,
    /// Length of the opening marker.
    open_len: usize,
    /// Length of the closing marker.
    close_len: usize,
    /// The style type.
    style: StyleType,
}

/// Parse markdown-style formatting and convert to Signal text styles.
///
/// Supported syntax:
/// - `**bold**` or `__bold__` → BOLD
/// - `*italic*` or `_italic_` → ITALIC
/// - `` `code` `` → MONOSPACE
/// - `~~strikethrough~~` → STRIKETHROUGH
///
/// Note: Nested formatting is not fully supported. The parser processes
/// patterns in order: bold (2-char markers) before italic (1-char markers).
pub fn parse_markdown(input: &str) -> FormattedMessage {
    let mut spans: Vec<FormatSpan> = Vec::new();

    // Find all formatting spans (order matters: longer markers first)
    find_spans(input, "**", "**", StyleType::Bold, &mut spans);
    find_spans(input, "__", "__", StyleType::Bold, &mut spans);
    find_spans(input, "~~", "~~", StyleType::Strikethrough, &mut spans);
    find_spans(input, "*", "*", StyleType::Italic, &mut spans);
    find_spans(input, "_", "_", StyleType::Italic, &mut spans);
    find_spans(input, "`", "`", StyleType::Monospace, &mut spans);

    // Sort spans by start position
    spans.sort_by_key(|s| s.start);

    // Remove overlapping spans (keep first one found)
    let spans = remove_overlapping(spans);

    // Build output text and calculate new positions
    build_formatted_message(input, &spans)
}

/// Find all spans matching a pattern.
fn find_spans(
    input: &str,
    open: &str,
    close: &str,
    style: StyleType,
    spans: &mut Vec<FormatSpan>,
) {
    let bytes = input.as_bytes();
    let open_bytes = open.as_bytes();
    let close_bytes = close.as_bytes();
    let open_len = open.len();
    let close_len = close.len();

    let mut pos = 0;
    while pos < input.len() {
        // Find opening marker
        if let Some(rel_start) = find_pattern(&bytes[pos..], open_bytes) {
            let start = pos + rel_start;
            let content_start = start + open_len;

            // Find closing marker (must have content between)
            if content_start < input.len() {
                if let Some(rel_end) = find_pattern(&bytes[content_start..], close_bytes) {
                    if rel_end > 0 {
                        // Found a valid span
                        let end = content_start + rel_end + close_len;

                        // Check this doesn't overlap with existing spans
                        let overlaps = spans.iter().any(|s| {
                            (start >= s.start && start < s.end) ||
                            (end > s.start && end <= s.end)
                        });

                        if !overlaps {
                            spans.push(FormatSpan {
                                start,
                                end,
                                open_len,
                                close_len,
                                style,
                            });
                            pos = end;
                            continue;
                        }
                    }
                }
            }
        }
        pos += 1;
    }
}

/// Find a byte pattern in a slice.
fn find_pattern(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Remove overlapping spans, keeping the first one.
fn remove_overlapping(mut spans: Vec<FormatSpan>) -> Vec<FormatSpan> {
    if spans.is_empty() {
        return spans;
    }

    let mut result = Vec::with_capacity(spans.len());
    spans.sort_by_key(|s| s.start);

    let mut last_end = 0;
    for span in spans {
        if span.start >= last_end {
            last_end = span.end;
            result.push(span);
        }
    }

    result
}

/// Build the final formatted message by removing markers and adjusting positions.
fn build_formatted_message(input: &str, spans: &[FormatSpan]) -> FormattedMessage {
    if spans.is_empty() {
        return FormattedMessage::plain(input);
    }

    let mut output = String::with_capacity(input.len());
    let mut styles = Vec::with_capacity(spans.len());
    let mut pos = 0;
    let mut offset = 0usize; // How much shorter output is vs input

    for span in spans {
        // Copy text before this span
        if span.start > pos {
            output.push_str(&input[pos..span.start]);
        }

        // Calculate output position (accounting for removed markers)
        let output_start = span.start - offset;

        // Extract content (without markers)
        let content_start = span.start + span.open_len;
        let content_end = span.end - span.close_len;
        let content = &input[content_start..content_end];

        // Add content to output
        output.push_str(content);

        // Create style for this span
        styles.push(TextStyle {
            start: output_start as u32,
            length: content.len() as u32,
            style: span.style.as_str().to_string(),
        });

        // Update offset (we removed open + close markers)
        offset += span.open_len + span.close_len;
        pos = span.end;
    }

    // Copy remaining text
    if pos < input.len() {
        output.push_str(&input[pos..]);
    }

    FormattedMessage {
        text: output,
        styles,
    }
}

/// Format a response with a metadata footer.
///
/// The footer contains mode indicator and optionally model info.
pub fn format_with_footer(
    response: &str,
    mode: &str,
    model: Option<&str>,
    tools_used: Option<&[String]>,
) -> FormattedMessage {
    let mut footer_parts = vec![mode.to_string()];

    if let Some(model_name) = model {
        // Shorten model name for display
        let short_name = shorten_model_name(model_name);
        footer_parts.push(short_name);
    }

    if let Some(tools) = tools_used {
        if !tools.is_empty() {
            footer_parts.push(format!("Tools: {}", tools.join(", ")));
        }
    }

    let footer = footer_parts.join(" · ");
    let full_text = format!("{}\n\n—\n{}", response.trim(), footer);

    // Calculate footer position for italic styling
    let footer_start = response.trim().len() + 4; // "\n\n—\n" = 4 chars
    let footer_len = footer.len();

    // Parse any markdown in the response body first
    let mut formatted = parse_markdown(&full_text);

    // Add italic style for the footer
    formatted.styles.push(TextStyle {
        start: footer_start as u32,
        length: footer_len as u32,
        style: "ITALIC".to_string(),
    });

    formatted
}

/// Shorten model names for display.
fn shorten_model_name(name: &str) -> String {
    // Common shortenings
    match name {
        n if n.starts_with("grok-") => n.to_string(),
        n if n.starts_with("llama-") => n.replace("llama-", "Llama "),
        n if n.starts_with("deepseek-") => n.replace("deepseek-", "DeepSeek "),
        n if n.starts_with("qwen") => n.to_string(),
        n if n.starts_with("mistral-") => n.replace("mistral-", "Mistral "),
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bold() {
        let result = parse_markdown("Hello **world**!");
        assert_eq!(result.text, "Hello world!");
        assert_eq!(result.styles.len(), 1);
        assert_eq!(result.styles[0].start, 6);
        assert_eq!(result.styles[0].length, 5);
        assert_eq!(result.styles[0].style, "BOLD");
    }

    #[test]
    fn test_parse_italic() {
        let result = parse_markdown("Hello *world*!");
        assert_eq!(result.text, "Hello world!");
        assert_eq!(result.styles.len(), 1);
        assert_eq!(result.styles[0].start, 6);
        assert_eq!(result.styles[0].length, 5);
        assert_eq!(result.styles[0].style, "ITALIC");
    }

    #[test]
    fn test_parse_monospace() {
        let result = parse_markdown("Use `code` here");
        assert_eq!(result.text, "Use code here");
        assert_eq!(result.styles.len(), 1);
        assert_eq!(result.styles[0].start, 4);
        assert_eq!(result.styles[0].length, 4);
        assert_eq!(result.styles[0].style, "MONOSPACE");
    }

    #[test]
    fn test_parse_strikethrough() {
        let result = parse_markdown("This is ~~wrong~~ right");
        assert_eq!(result.text, "This is wrong right");
        assert_eq!(result.styles.len(), 1);
        assert_eq!(result.styles[0].start, 8);
        assert_eq!(result.styles[0].length, 5);
        assert_eq!(result.styles[0].style, "STRIKETHROUGH");
    }

    #[test]
    fn test_parse_multiple_formats() {
        let result = parse_markdown("**Bold** and *italic* text");
        assert_eq!(result.text, "Bold and italic text");
        assert_eq!(result.styles.len(), 2);

        // Bold
        assert_eq!(result.styles[0].start, 0);
        assert_eq!(result.styles[0].length, 4);
        assert_eq!(result.styles[0].style, "BOLD");

        // Italic
        assert_eq!(result.styles[1].start, 9);
        assert_eq!(result.styles[1].length, 6);
        assert_eq!(result.styles[1].style, "ITALIC");
    }

    #[test]
    fn test_parse_underscore_formats() {
        let result = parse_markdown("__bold__ and _italic_");
        assert_eq!(result.text, "bold and italic");
        assert_eq!(result.styles.len(), 2);
        assert_eq!(result.styles[0].style, "BOLD");
        assert_eq!(result.styles[1].style, "ITALIC");
    }

    #[test]
    fn test_no_formatting() {
        let result = parse_markdown("Plain text message");
        assert_eq!(result.text, "Plain text message");
        assert!(result.styles.is_empty());
    }

    #[test]
    fn test_unclosed_markers() {
        let result = parse_markdown("Hello **world without closing");
        assert_eq!(result.text, "Hello **world without closing");
        assert!(result.styles.is_empty());
    }

    #[test]
    fn test_empty_markers() {
        let result = parse_markdown("Empty ** ** markers");
        // Should still work - space between markers
        assert_eq!(result.text, "Empty   markers");
    }

    #[test]
    fn test_adjacent_formats() {
        let result = parse_markdown("**bold***italic*");
        assert_eq!(result.text, "bolditalic");
        assert_eq!(result.styles.len(), 2);
    }

    #[test]
    fn test_format_with_footer() {
        let result = format_with_footer(
            "Hello world",
            "⚡ Speed",
            Some("grok-4"),
            None,
        );
        assert!(result.text.contains("Hello world"));
        assert!(result.text.contains("⚡ Speed"));
        assert!(result.text.contains("grok-4"));
        assert!(result.text.contains("—")); // Footer separator
    }

    #[test]
    fn test_format_with_footer_and_tools() {
        let tools = vec!["calculator".to_string(), "weather".to_string()];
        let result = format_with_footer(
            "The answer is 42",
            "🔒 Privacy",
            Some("llama-3.3-70b"),
            Some(&tools),
        );
        assert!(result.text.contains("calculator"));
        assert!(result.text.contains("weather"));
    }

    #[test]
    fn test_markdown_in_response_with_footer() {
        let result = format_with_footer(
            "Here is **important** info",
            "⚡ Speed",
            None,
            None,
        );
        assert_eq!(result.text, "Here is important info\n\n—\n⚡ Speed");
        // Should have both: bold for "important" and italic for footer
        assert!(result.styles.iter().any(|s| s.style == "BOLD"));
        assert!(result.styles.iter().any(|s| s.style == "ITALIC"));
    }
}
