//! Dictionary tool using Free Dictionary API.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::debug;

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// Response from Free Dictionary API.
#[derive(Debug, Deserialize)]
struct DictionaryEntry {
    word: String,
    phonetic: Option<String>,
    meanings: Vec<Meaning>,
}

#[derive(Debug, Deserialize)]
struct Meaning {
    #[serde(rename = "partOfSpeech")]
    part_of_speech: String,
    definitions: Vec<Definition>,
}

#[derive(Debug, Deserialize)]
struct Definition {
    definition: String,
    example: Option<String>,
    synonyms: Option<Vec<String>>,
}

/// Dictionary tool using Free Dictionary API.
///
/// Looks up word definitions, pronunciation, and examples.
/// Free API, no key required.
///
/// # Parameters
///
/// - `word` (required): Word to look up
///
/// # Examples
///
/// ```json
/// {"word": "serendipity"}
/// {"word": "ephemeral"}
/// {"word": "ubiquitous"}
/// ```
pub struct Dictionary {
    client: reqwest::Client,
}

impl Dictionary {
    /// Create a new dictionary tool.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("AmanBot/1.0")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Fetch definition from Free Dictionary API.
    async fn lookup(&self, word: &str) -> Result<DictionaryEntry, ToolError> {
        let url = format!(
            "https://api.dictionaryapi.dev/api/v2/entries/en/{}",
            urlencoding::encode(word)
        );

        debug!("Looking up word: {}", url);

        let response = self.client.get(&url).send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ToolError::ExecutionFailed(format!(
                "Word '{}' not found in dictionary",
                word
            )));
        }

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "Dictionary API returned status {}",
                response.status()
            )));
        }

        let entries: Vec<DictionaryEntry> = response.json().await?;

        entries.into_iter().next().ok_or_else(|| {
            ToolError::ExecutionFailed(format!("No definition found for '{}'", word))
        })
    }

    /// Format the dictionary entry for display.
    fn format_entry(entry: &DictionaryEntry) -> String {
        let mut output = format!("**{}**", entry.word);

        // Add phonetic pronunciation if available
        if let Some(ref phonetic) = entry.phonetic {
            output.push_str(&format!(" {}", phonetic));
        }
        output.push('\n');

        // Add meanings (limit to first 3)
        for meaning in entry.meanings.iter().take(3) {
            output.push_str(&format!("\n_{}_\n", meaning.part_of_speech));

            // Add definitions (limit to first 2 per part of speech)
            for (i, def) in meaning.definitions.iter().take(2).enumerate() {
                output.push_str(&format!("{}. {}\n", i + 1, def.definition));

                // Add example if available
                if let Some(ref example) = def.example {
                    output.push_str(&format!("   Example: \"{}\"\n", example));
                }
            }

            // Add synonyms if available (limit to 5)
            if let Some(def) = meaning.definitions.first() {
                if let Some(ref synonyms) = def.synonyms {
                    if !synonyms.is_empty() {
                        let syns: Vec<_> = synonyms.iter().take(5).map(|s| s.as_str()).collect();
                        output.push_str(&format!("   Synonyms: {}\n", syns.join(", ")));
                    }
                }
            }
        }

        output
    }
}

impl Default for Dictionary {
    fn default() -> Self {
        Self::new()
    }
}

// Simple URL encoding for the word
mod urlencoding {
    pub fn encode(input: &str) -> String {
        let mut result = String::new();
        for c in input.chars() {
            match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                    result.push(c);
                }
                ' ' => result.push_str("%20"),
                _ => {
                    for byte in c.to_string().as_bytes() {
                        result.push_str(&format!("%{:02X}", byte));
                    }
                }
            }
        }
        result
    }
}

#[async_trait]
impl Tool for Dictionary {
    fn name(&self) -> &str {
        "dictionary"
    }

    fn description(&self) -> &str {
        "Looks up word definitions, pronunciation, examples, and synonyms."
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let word = args.get_string("word")?;

        if word.trim().is_empty() {
            return Err(ToolError::InvalidParameter {
                name: "word".to_string(),
                reason: "Word cannot be empty".to_string(),
            });
        }

        debug!("Looking up definition for: {}", word);

        let entry = self.lookup(&word).await?;
        let formatted = Self::format_entry(&entry);

        Ok(ToolOutput::success(formatted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_url_encoding() {
        assert_eq!(urlencoding::encode("hello"), "hello");
        assert_eq!(urlencoding::encode("hello world"), "hello%20world");
        assert_eq!(urlencoding::encode("café"), "caf%C3%A9");
    }

    #[tokio::test]
    #[ignore] // Requires network
    async fn test_lookup_word() {
        let tool = Dictionary::new();
        let mut params = HashMap::new();
        params.insert(
            "word".to_string(),
            serde_json::Value::String("hello".to_string()),
        );

        let result = tool.execute(ToolArgs::new(params)).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("hello"));
    }

    #[tokio::test]
    #[ignore] // Requires network
    async fn test_lookup_nonexistent_word() {
        let tool = Dictionary::new();
        let mut params = HashMap::new();
        params.insert(
            "word".to_string(),
            serde_json::Value::String("asdfghjklqwerty".to_string()),
        );

        let result = tool.execute(ToolArgs::new(params)).await;
        assert!(matches!(result, Err(ToolError::ExecutionFailed(_))));
    }
}
