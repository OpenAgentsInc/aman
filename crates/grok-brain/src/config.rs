//! Configuration for GrokBrain.

use brain_core::BrainError;
use std::env;
use std::path::Path;

/// Default system prompt file name.
pub const DEFAULT_PROMPT_FILE: &str = "SYSTEM_PROMPT.md";

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

    /// Maximum characters for memory prompt injection (0 disables).
    pub memory_prompt_max_chars: usize,
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
            memory_prompt_max_chars: 1800,
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
    /// - `GROK_SYSTEM_PROMPT` - System prompt (overrides prompt file)
    /// - `GROK_PROMPT_FILE` - Path to system prompt file (default: SYSTEM_PROMPT.md)
    /// - `GROK_MAX_TOKENS` - Max tokens (default: 1024)
    /// - `GROK_TEMPERATURE` - Temperature (default: 0.7)
    /// - `GROK_MAX_HISTORY_TURNS` - Max history turns (default: 10)
    /// - `GROK_ENABLE_X_SEARCH` - Enable X Search tool (default: false)
    /// - `GROK_ENABLE_WEB_SEARCH` - Enable Web Search tool (default: false)
    /// - `GROK_MEMORY_PROMPT_MAX_CHARS` - Max memory prompt chars (default: 1800)
    /// - `GROK_MEMORY_PROMPT_MAX_TOKENS` - Max memory prompt tokens (approx, optional)
    ///
    /// System prompt priority:
    /// 1. `GROK_SYSTEM_PROMPT` env var (if set)
    /// 2. Contents of prompt file (if exists)
    /// 3. None
    pub fn from_env() -> Result<Self, BrainError> {
        let api_key = env::var("GROK_API_KEY")
            .map_err(|_| BrainError::Configuration("GROK_API_KEY not set".to_string()))?;

        let api_url = env::var("GROK_API_URL").unwrap_or_else(|_| "https://api.x.ai".to_string());

        let model =
            env::var("GROK_MODEL").unwrap_or_else(|_| "grok-4-1-fast".to_string());

        // System prompt: env var takes precedence, then try loading from file
        let system_prompt = if let Ok(prompt) = env::var("GROK_SYSTEM_PROMPT") {
            Some(prompt)
        } else {
            // Try to load from prompt file
            let prompt_file = env::var("GROK_PROMPT_FILE")
                .unwrap_or_else(|_| DEFAULT_PROMPT_FILE.to_string());
            load_prompt_file(&prompt_file)
        };

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

        let memory_prompt_max_chars = env::var("GROK_MEMORY_PROMPT_MAX_CHARS")
            .ok()
            .and_then(|v| v.parse().ok())
            .or_else(|| {
                env::var("GROK_MEMORY_PROMPT_MAX_TOKENS")
                    .ok()
                    .and_then(|v| v.parse::<usize>().ok())
                    .map(|tokens| tokens.saturating_mul(4))
            })
            .unwrap_or(1800);

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
            memory_prompt_max_chars,
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

    /// Set the maximum memory prompt characters.
    pub fn memory_prompt_max_chars(mut self, chars: usize) -> Self {
        self.config.memory_prompt_max_chars = chars;
        self
    }

    /// Build the configuration.
    pub fn build(self) -> GrokBrainConfig {
        self.config
    }

    /// Load system prompt from a file.
    ///
    /// If the file exists and is non-empty, sets the system prompt.
    /// Returns self for chaining.
    pub fn load_prompt_file(mut self, path: impl AsRef<Path>) -> Self {
        if let Some(prompt) = load_prompt_file(path) {
            self.config.system_prompt = Some(prompt);
        }
        self
    }
}

/// Load a prompt file, returning None if not found or empty.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GrokBrainConfig::default();

        assert_eq!(config.api_url, "https://api.x.ai");
        assert!(config.api_key.is_empty());
        assert_eq!(config.model, "grok-4-1-fast");
        assert!(config.system_prompt.is_none());
        assert_eq!(config.max_tokens, Some(1024));
        assert_eq!(config.temperature, Some(0.7));
        assert_eq!(config.max_history_turns, 10);
        assert!(!config.enable_x_search);
        assert!(!config.enable_web_search);
        assert_eq!(config.memory_prompt_max_chars, 1800);
    }

    #[test]
    fn test_builder_api_key() {
        let config = GrokBrainConfig::builder()
            .api_key("test-api-key")
            .build();

        assert_eq!(config.api_key, "test-api-key");
    }

    #[test]
    fn test_builder_all_options() {
        let config = GrokBrainConfig::builder()
            .api_key("my-key")
            .api_url("https://custom.api.com")
            .model("grok-4")
            .system_prompt("You are helpful")
            .max_tokens(512)
            .temperature(0.5)
            .max_history_turns(5)
            .enable_x_search(true)
            .enable_web_search(true)
            .memory_prompt_max_chars(1200)
            .build();

        assert_eq!(config.api_key, "my-key");
        assert_eq!(config.api_url, "https://custom.api.com");
        assert_eq!(config.model, "grok-4");
        assert_eq!(config.system_prompt, Some("You are helpful".to_string()));
        assert_eq!(config.max_tokens, Some(512));
        assert_eq!(config.temperature, Some(0.5));
        assert_eq!(config.max_history_turns, 5);
        assert!(config.enable_x_search);
        assert!(config.enable_web_search);
        assert_eq!(config.memory_prompt_max_chars, 1200);
    }

    // Environment-based tests are combined into a single test to avoid
    // race conditions when tests run in parallel (env vars are process-global).
    #[test]
    fn test_from_env_scenarios() {
        use std::sync::Mutex;
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let _guard = ENV_LOCK.lock().unwrap();

        // Helper to clear all GROK_ env vars
        fn clear_all_grok_vars() {
            std::env::remove_var("GROK_API_KEY");
            std::env::remove_var("GROK_API_URL");
            std::env::remove_var("GROK_MODEL");
            std::env::remove_var("GROK_SYSTEM_PROMPT");
            std::env::remove_var("GROK_PROMPT_FILE");
            std::env::remove_var("GROK_MAX_TOKENS");
            std::env::remove_var("GROK_TEMPERATURE");
            std::env::remove_var("GROK_MAX_HISTORY_TURNS");
            std::env::remove_var("GROK_ENABLE_X_SEARCH");
            std::env::remove_var("GROK_ENABLE_WEB_SEARCH");
            std::env::remove_var("GROK_MEMORY_PROMPT_MAX_CHARS");
            std::env::remove_var("GROK_MEMORY_PROMPT_MAX_TOKENS");
        }

        // Scenario 1: Missing API key should error
        clear_all_grok_vars();
        let result = GrokBrainConfig::from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            BrainError::Configuration(msg) => {
                assert!(msg.contains("GROK_API_KEY"));
            }
            _ => panic!("Expected Configuration error"),
        }

        // Scenario 2: Only API key set, defaults used
        clear_all_grok_vars();
        std::env::set_var("GROK_API_KEY", "test-env-key");

        let config = GrokBrainConfig::from_env().unwrap();
        assert_eq!(config.api_key, "test-env-key");
        assert_eq!(config.api_url, "https://api.x.ai");
        assert_eq!(config.model, "grok-4-1-fast");
        assert!(config.system_prompt.is_none());
        assert_eq!(config.max_tokens, Some(1024));
        assert_eq!(config.temperature, Some(0.7));
        assert_eq!(config.max_history_turns, 10);
        assert!(!config.enable_x_search);
        assert!(!config.enable_web_search);

        // Scenario 3: All vars set
        clear_all_grok_vars();
        std::env::set_var("GROK_API_KEY", "full-test-key");
        std::env::set_var("GROK_API_URL", "https://test.api.com");
        std::env::set_var("GROK_MODEL", "grok-4");
        std::env::set_var("GROK_SYSTEM_PROMPT", "Test prompt");
        std::env::set_var("GROK_MAX_TOKENS", "2048");
        std::env::set_var("GROK_TEMPERATURE", "0.9");
        std::env::set_var("GROK_MAX_HISTORY_TURNS", "20");
        std::env::set_var("GROK_ENABLE_X_SEARCH", "true");
        std::env::set_var("GROK_ENABLE_WEB_SEARCH", "1");

        let config = GrokBrainConfig::from_env().unwrap();
        assert_eq!(config.api_key, "full-test-key");
        assert_eq!(config.api_url, "https://test.api.com");
        assert_eq!(config.model, "grok-4");
        assert_eq!(config.system_prompt, Some("Test prompt".to_string()));
        assert_eq!(config.max_tokens, Some(2048));
        assert_eq!(config.temperature, Some(0.9));
        assert_eq!(config.max_history_turns, 20);
        assert!(config.enable_x_search);
        assert!(config.enable_web_search);

        // Scenario 4: Search flags set to false/0
        clear_all_grok_vars();
        std::env::set_var("GROK_API_KEY", "test-key");
        std::env::set_var("GROK_ENABLE_X_SEARCH", "false");
        std::env::set_var("GROK_ENABLE_WEB_SEARCH", "0");

        let config = GrokBrainConfig::from_env().unwrap();
        assert!(!config.enable_x_search);
        assert!(!config.enable_web_search);

        // Cleanup
        clear_all_grok_vars();
    }
}
