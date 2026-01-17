//! Configuration for MapleBrain.

use std::env;
use brain_core::BrainError;

/// Configuration for MapleBrain.
#[derive(Debug, Clone)]
pub struct MapleBrainConfig {
    /// OpenSecret API URL.
    pub api_url: String,

    /// API key for authentication.
    pub api_key: String,

    /// Model name to use.
    pub model: String,

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
            api_url: "https://api.opensecret.cloud".to_string(),
            api_key: String::new(),
            model: "hugging-quants/Meta-Llama-3.1-70B-Instruct-AWQ-INT4".to_string(),
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
    /// - `MAPLE_API_URL` - API URL (default: https://api.opensecret.cloud)
    /// - `MAPLE_MODEL` - Model name (default: hugging-quants/Meta-Llama-3.1-70B-Instruct-AWQ-INT4)
    /// - `MAPLE_SYSTEM_PROMPT` - System prompt
    /// - `MAPLE_MAX_TOKENS` - Max tokens (default: 1024)
    /// - `MAPLE_TEMPERATURE` - Temperature (default: 0.7)
    /// - `MAPLE_MAX_HISTORY_TURNS` - Max history turns (default: 10)
    pub fn from_env() -> Result<Self, BrainError> {
        let api_key = env::var("MAPLE_API_KEY")
            .map_err(|_| BrainError::Configuration("MAPLE_API_KEY not set".to_string()))?;

        let api_url = env::var("MAPLE_API_URL")
            .unwrap_or_else(|_| "https://api.opensecret.cloud".to_string());

        let model = env::var("MAPLE_MODEL")
            .unwrap_or_else(|_| "hugging-quants/Meta-Llama-3.1-70B-Instruct-AWQ-INT4".to_string());

        let system_prompt = env::var("MAPLE_SYSTEM_PROMPT").ok();

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
}
