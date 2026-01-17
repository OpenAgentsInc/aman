//! Configuration for MapleBrain.

use std::env;
use std::path::Path;
use brain_core::BrainError;

/// Default path for the system prompt file.
pub const DEFAULT_PROMPT_FILE: &str = "PROMPT.md";

/// Configuration for MapleBrain.
#[derive(Debug, Clone)]
pub struct MapleBrainConfig {
    /// OpenSecret API URL.
    pub api_url: String,

    /// API key for authentication.
    pub api_key: String,

    /// Model name to use for text-only messages.
    pub model: String,

    /// Model name to use for messages with images (vision model).
    pub vision_model: String,

    /// Optional system prompt.
    pub system_prompt: Option<String>,

    /// Maximum tokens for response.
    pub max_tokens: Option<u32>,

    /// Temperature for generation (0.0 - 2.0).
    pub temperature: Option<f32>,

    /// Maximum number of conversation turns to keep in history.
    pub max_history_turns: usize,
}

impl Default for MapleBrainConfig {
    fn default() -> Self {
        Self {
            api_url: "https://enclave.trymaple.ai".to_string(),
            api_key: String::new(),
            model: "llama-3.3-70b".to_string(),
            vision_model: "qwen3-vl-30b".to_string(),
            system_prompt: None,
            max_tokens: Some(1024),
            temperature: Some(0.7),
            max_history_turns: 10,
        }
    }
}

impl MapleBrainConfig {
    /// Create configuration from environment variables.
    ///
    /// Required environment variables:
    /// - `MAPLE_API_KEY` - API key for authentication
    ///
    /// Optional environment variables:
    /// - `MAPLE_API_URL` - API URL (default: https://enclave.trymaple.ai)
    /// - `MAPLE_MODEL` - Model name for text (default: llama-3.3-70b)
    /// - `MAPLE_VISION_MODEL` - Model name for images (default: qwen3-vl-30b)
    /// - `MAPLE_SYSTEM_PROMPT` - System prompt (overrides prompt file)
    /// - `MAPLE_PROMPT_FILE` - Path to system prompt file (default: PROMPT.md)
    /// - `MAPLE_MAX_TOKENS` - Max tokens (default: 1024)
    /// - `MAPLE_TEMPERATURE` - Temperature (default: 0.7)
    /// - `MAPLE_MAX_HISTORY_TURNS` - Max history turns (default: 10)
    ///
    /// System prompt priority:
    /// 1. `MAPLE_SYSTEM_PROMPT` env var (if set)
    /// 2. Contents of prompt file (if exists)
    /// 3. None
    pub fn from_env() -> Result<Self, BrainError> {
        let api_key = env::var("MAPLE_API_KEY")
            .map_err(|_| BrainError::Configuration("MAPLE_API_KEY not set".to_string()))?;

        let api_url = env::var("MAPLE_API_URL")
            .unwrap_or_else(|_| "https://enclave.trymaple.ai".to_string());

        let model = env::var("MAPLE_MODEL")
            .unwrap_or_else(|_| "llama-3.3-70b".to_string());

        let vision_model = env::var("MAPLE_VISION_MODEL")
            .unwrap_or_else(|_| "qwen3-vl-30b".to_string());

        // System prompt: env var takes precedence, then try loading from file
        let system_prompt = if let Ok(prompt) = env::var("MAPLE_SYSTEM_PROMPT") {
            Some(prompt)
        } else {
            // Try to load from prompt file
            let prompt_file = env::var("MAPLE_PROMPT_FILE")
                .unwrap_or_else(|_| DEFAULT_PROMPT_FILE.to_string());
            load_prompt_file(&prompt_file)
        };

        let max_tokens = env::var("MAPLE_MAX_TOKENS")
            .ok()
            .and_then(|s| s.parse().ok());

        let temperature = env::var("MAPLE_TEMPERATURE")
            .ok()
            .and_then(|s| s.parse().ok());

        let max_history_turns = env::var("MAPLE_MAX_HISTORY_TURNS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        Ok(Self {
            api_url,
            api_key,
            model,
            vision_model,
            system_prompt,
            max_tokens: max_tokens.or(Some(1024)),
            temperature: temperature.or(Some(0.7)),
            max_history_turns,
        })
    }

    /// Create a new configuration with required fields.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            ..Default::default()
        }
    }

    /// Set the API URL.
    pub fn with_api_url(mut self, url: impl Into<String>) -> Self {
        self.api_url = url.into();
        self
    }

    /// Set the system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set the max tokens.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set the temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the max history turns.
    pub fn with_max_history_turns(mut self, turns: usize) -> Self {
        self.max_history_turns = turns;
        self
    }

    /// Set the vision model.
    pub fn with_vision_model(mut self, model: impl Into<String>) -> Self {
        self.vision_model = model.into();
        self
    }

    /// Load system prompt from a file.
    ///
    /// Returns `Ok(self)` with the prompt loaded, or the original config if file doesn't exist.
    pub fn with_prompt_file(mut self, path: impl AsRef<Path>) -> Self {
        if let Some(prompt) = load_prompt_file(path.as_ref()) {
            self.system_prompt = Some(prompt);
        }
        self
    }
}

/// Load a prompt from a file path.
///
/// Returns `Some(content)` if the file exists and is readable, `None` otherwise.
fn load_prompt_file(path: impl AsRef<Path>) -> Option<String> {
    let path = path.as_ref();

    match std::fs::read_to_string(path) {
        Ok(content) => {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Err(_) => None,
    }
}
