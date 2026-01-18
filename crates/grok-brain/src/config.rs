//! Configuration for GrokBrain.

use brain_core::BrainError;
use std::env;

/// Configuration for GrokBrain.
#[derive(Debug, Clone)]
pub struct GrokBrainConfig {
    /// xAI API URL.
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

    /// Enable X Search tool for real-time Twitter/X data.
    pub enable_x_search: bool,

    /// Enable Web Search tool for current web information.
    pub enable_web_search: bool,
}

impl Default for GrokBrainConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.x.ai".to_string(),
            api_key: String::new(),
            model: "grok-4-1-fast".to_string(),
            system_prompt: None,
            max_tokens: Some(1024),
            temperature: Some(0.7),
            max_history_turns: 10,
            enable_x_search: false,
            enable_web_search: false,
        }
    }
}

impl GrokBrainConfig {
    /// Create configuration from environment variables.
    ///
    /// Required environment variables:
    /// - `GROK_API_KEY` - API key for authentication
    ///
    /// Optional environment variables:
    /// - `GROK_API_URL` - API URL (default: https://api.x.ai)
    /// - `GROK_MODEL` - Model name (default: grok-4-1-fast)
    /// - `GROK_SYSTEM_PROMPT` - System prompt
    /// - `GROK_MAX_TOKENS` - Max tokens (default: 1024)
    /// - `GROK_TEMPERATURE` - Temperature (default: 0.7)
    /// - `GROK_MAX_HISTORY_TURNS` - Max history turns (default: 10)
    /// - `GROK_ENABLE_X_SEARCH` - Enable X Search tool (default: false)
    /// - `GROK_ENABLE_WEB_SEARCH` - Enable Web Search tool (default: false)
    pub fn from_env() -> Result<Self, BrainError> {
        let api_key = env::var("GROK_API_KEY")
            .map_err(|_| BrainError::Configuration("GROK_API_KEY not set".to_string()))?;

        let api_url = env::var("GROK_API_URL").unwrap_or_else(|_| "https://api.x.ai".to_string());

        let model =
            env::var("GROK_MODEL").unwrap_or_else(|_| "grok-4-1-fast".to_string());

        let system_prompt = env::var("GROK_SYSTEM_PROMPT").ok();

        let max_tokens = env::var("GROK_MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(Some(1024));

        let temperature = env::var("GROK_TEMPERATURE")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(Some(0.7));

        let max_history_turns = env::var("GROK_MAX_HISTORY_TURNS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        let enable_x_search = env::var("GROK_ENABLE_X_SEARCH")
            .ok()
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        let enable_web_search = env::var("GROK_ENABLE_WEB_SEARCH")
            .ok()
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        Ok(Self {
            api_url,
            api_key,
            model,
            system_prompt,
            max_tokens,
            temperature,
            max_history_turns,
            enable_x_search,
            enable_web_search,
        })
    }

    /// Create a new config builder.
    pub fn builder() -> GrokBrainConfigBuilder {
        GrokBrainConfigBuilder::default()
    }
}

/// Builder for GrokBrainConfig.
#[derive(Debug, Default)]
pub struct GrokBrainConfigBuilder {
    config: GrokBrainConfig,
}

impl GrokBrainConfigBuilder {
    /// Set the API key.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.config.api_key = key.into();
        self
    }

    /// Set the API URL.
    pub fn api_url(mut self, url: impl Into<String>) -> Self {
        self.config.api_url = url.into();
        self
    }

    /// Set the model name.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.config.model = model.into();
        self
    }

    /// Set the system prompt.
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.config.system_prompt = Some(prompt.into());
        self
    }

    /// Set the max tokens.
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.config.max_tokens = Some(tokens);
        self
    }

    /// Set the temperature.
    pub fn temperature(mut self, temp: f32) -> Self {
        self.config.temperature = Some(temp);
        self
    }

    /// Set the max history turns.
    pub fn max_history_turns(mut self, turns: usize) -> Self {
        self.config.max_history_turns = turns;
        self
    }

    /// Enable X Search tool.
    pub fn enable_x_search(mut self, enable: bool) -> Self {
        self.config.enable_x_search = enable;
        self
    }

    /// Enable Web Search tool.
    pub fn enable_web_search(mut self, enable: bool) -> Self {
        self.config.enable_web_search = enable;
        self
    }

    /// Build the configuration.
    pub fn build(self) -> GrokBrainConfig {
        self.config
    }
}
