//! Model selection logic based on task hints.
//!
//! This module provides the logic for selecting the best model based on
//! the task type, sensitivity, and user preferences.

use crate::actions::TaskHint;

/// Model configuration for Maple (OpenSecret TEE).
#[derive(Debug, Clone)]
pub struct MapleModels {
    /// Default model for general tasks.
    pub general: String,
    /// Model optimized for coding tasks.
    pub coding: String,
    /// Model optimized for math/reasoning tasks.
    pub math: String,
    /// Model optimized for creative tasks.
    pub creative: String,
    /// Model optimized for multilingual tasks.
    pub multilingual: String,
    /// Model optimized for quick responses.
    pub quick: String,
    /// Model for vision/image tasks.
    pub vision: String,
}

impl Default for MapleModels {
    fn default() -> Self {
        Self {
            general: "llama-3.3-70b".to_string(),
            coding: "deepseek-r1-0528".to_string(),
            math: "deepseek-r1-0528".to_string(),
            creative: "gpt-oss-120b".to_string(),
            multilingual: "qwen2-5-72b".to_string(),
            quick: "mistral-small-3-1-24b".to_string(),
            vision: "qwen3-vl-30b".to_string(),
        }
    }
}

impl MapleModels {
    /// Select the best model for a given task hint.
    pub fn select(&self, task_hint: TaskHint) -> &str {
        match task_hint {
            TaskHint::General => &self.general,
            TaskHint::Coding => &self.coding,
            TaskHint::Math => &self.math,
            TaskHint::Creative => &self.creative,
            TaskHint::Multilingual => &self.multilingual,
            TaskHint::Quick => &self.quick,
            TaskHint::Vision => &self.vision,
        }
    }

    /// Create from environment variables.
    ///
    /// Environment variables:
    /// - `MAPLE_MODEL` - Default/general model
    /// - `MAPLE_MODEL_CODING` - Coding model
    /// - `MAPLE_MODEL_MATH` - Math/reasoning model
    /// - `MAPLE_MODEL_CREATIVE` - Creative model
    /// - `MAPLE_MODEL_MULTILINGUAL` - Multilingual model
    /// - `MAPLE_MODEL_QUICK` - Quick response model
    /// - `MAPLE_VISION_MODEL` - Vision model
    pub fn from_env() -> Self {
        let defaults = Self::default();

        Self {
            general: std::env::var("MAPLE_MODEL").unwrap_or(defaults.general),
            coding: std::env::var("MAPLE_MODEL_CODING").unwrap_or(defaults.coding),
            math: std::env::var("MAPLE_MODEL_MATH").unwrap_or(defaults.math),
            creative: std::env::var("MAPLE_MODEL_CREATIVE").unwrap_or(defaults.creative),
            multilingual: std::env::var("MAPLE_MODEL_MULTILINGUAL").unwrap_or(defaults.multilingual),
            quick: std::env::var("MAPLE_MODEL_QUICK").unwrap_or(defaults.quick),
            vision: std::env::var("MAPLE_VISION_MODEL").unwrap_or(defaults.vision),
        }
    }
}

/// Model configuration for Grok (xAI).
#[derive(Debug, Clone)]
pub struct GrokModels {
    /// Default model for general tasks.
    pub general: String,
    /// Model optimized for coding tasks.
    pub coding: String,
    /// Model optimized for math/reasoning tasks.
    pub math: String,
    /// Model optimized for creative tasks.
    pub creative: String,
    /// Model optimized for multilingual tasks.
    pub multilingual: String,
    /// Model optimized for quick responses.
    pub quick: String,
}

impl Default for GrokModels {
    fn default() -> Self {
        Self {
            general: "grok-4-1-fast".to_string(),
            coding: "grok-3".to_string(),
            math: "grok-4".to_string(),
            creative: "grok-4".to_string(),
            multilingual: "grok-4-1-fast".to_string(),
            quick: "grok-3-mini".to_string(),
        }
    }
}

impl GrokModels {
    /// Select the best model for a given task hint.
    ///
    /// Note: Vision tasks are not supported by Grok. If passed Vision,
    /// this falls back to the general model. Callers should route Vision
    /// tasks to Maple instead.
    pub fn select(&self, task_hint: TaskHint) -> &str {
        match task_hint {
            TaskHint::General => &self.general,
            TaskHint::Coding => &self.coding,
            TaskHint::Math => &self.math,
            TaskHint::Creative => &self.creative,
            TaskHint::Multilingual => &self.multilingual,
            TaskHint::Quick => &self.quick,
            // Grok doesn't support vision - fall back to general
            TaskHint::Vision => &self.general,
        }
    }

    /// Create from environment variables.
    ///
    /// Environment variables:
    /// - `GROK_MODEL` - Default/general model
    /// - `GROK_MODEL_CODING` - Coding model
    /// - `GROK_MODEL_MATH` - Math/reasoning model
    /// - `GROK_MODEL_CREATIVE` - Creative model
    /// - `GROK_MODEL_MULTILINGUAL` - Multilingual model
    /// - `GROK_MODEL_QUICK` - Quick response model
    pub fn from_env() -> Self {
        let defaults = Self::default();

        Self {
            general: std::env::var("GROK_MODEL").unwrap_or(defaults.general),
            coding: std::env::var("GROK_MODEL_CODING").unwrap_or(defaults.coding),
            math: std::env::var("GROK_MODEL_MATH").unwrap_or(defaults.math),
            creative: std::env::var("GROK_MODEL_CREATIVE").unwrap_or(defaults.creative),
            multilingual: std::env::var("GROK_MODEL_MULTILINGUAL").unwrap_or(defaults.multilingual),
            quick: std::env::var("GROK_MODEL_QUICK").unwrap_or(defaults.quick),
        }
    }
}

/// Combined model selector for both providers.
#[derive(Debug, Clone)]
pub struct ModelSelector {
    /// Maple models configuration.
    pub maple: MapleModels,
    /// Grok models configuration.
    pub grok: GrokModels,
}

impl Default for ModelSelector {
    fn default() -> Self {
        Self {
            maple: MapleModels::default(),
            grok: GrokModels::default(),
        }
    }
}

impl ModelSelector {
    /// Create a new model selector with default configurations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create from environment variables.
    pub fn from_env() -> Self {
        Self {
            maple: MapleModels::from_env(),
            grok: GrokModels::from_env(),
        }
    }

    /// Select the best Maple model for a task hint.
    pub fn select_maple(&self, task_hint: TaskHint) -> &str {
        self.maple.select(task_hint)
    }

    /// Select the best Grok model for a task hint.
    pub fn select_grok(&self, task_hint: TaskHint) -> &str {
        self.grok.select(task_hint)
    }

    /// Get the vision model for Maple.
    pub fn maple_vision(&self) -> &str {
        &self.maple.vision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maple_models_default() {
        let models = MapleModels::default();
        assert_eq!(models.general, "llama-3.3-70b");
        assert_eq!(models.coding, "deepseek-r1-0528");
        assert_eq!(models.math, "deepseek-r1-0528");
        assert_eq!(models.creative, "gpt-oss-120b");
        assert_eq!(models.multilingual, "qwen2-5-72b");
        assert_eq!(models.quick, "mistral-small-3-1-24b");
        assert_eq!(models.vision, "qwen3-vl-30b");
    }

    #[test]
    fn test_grok_models_default() {
        let models = GrokModels::default();
        assert_eq!(models.general, "grok-4-1-fast");
        assert_eq!(models.coding, "grok-3");
        assert_eq!(models.math, "grok-4");
        assert_eq!(models.creative, "grok-4");
        assert_eq!(models.multilingual, "grok-4-1-fast");
        assert_eq!(models.quick, "grok-3-mini");
    }

    #[test]
    fn test_maple_select() {
        let models = MapleModels::default();
        assert_eq!(models.select(TaskHint::General), "llama-3.3-70b");
        assert_eq!(models.select(TaskHint::Coding), "deepseek-r1-0528");
        assert_eq!(models.select(TaskHint::Math), "deepseek-r1-0528");
        assert_eq!(models.select(TaskHint::Creative), "gpt-oss-120b");
        assert_eq!(models.select(TaskHint::Multilingual), "qwen2-5-72b");
        assert_eq!(models.select(TaskHint::Quick), "mistral-small-3-1-24b");
        assert_eq!(models.select(TaskHint::Vision), "qwen3-vl-30b");
    }

    #[test]
    fn test_grok_select() {
        let models = GrokModels::default();
        assert_eq!(models.select(TaskHint::General), "grok-4-1-fast");
        assert_eq!(models.select(TaskHint::Coding), "grok-3");
        assert_eq!(models.select(TaskHint::Math), "grok-4");
        assert_eq!(models.select(TaskHint::Creative), "grok-4");
        assert_eq!(models.select(TaskHint::Multilingual), "grok-4-1-fast");
        assert_eq!(models.select(TaskHint::Quick), "grok-3-mini");
        // Vision falls back to general since Grok doesn't support it
        assert_eq!(models.select(TaskHint::Vision), "grok-4-1-fast");
    }

    #[test]
    fn test_model_selector() {
        let selector = ModelSelector::new();
        assert_eq!(selector.select_maple(TaskHint::Coding), "deepseek-r1-0528");
        assert_eq!(selector.select_grok(TaskHint::Coding), "grok-3");
        assert_eq!(selector.maple_vision(), "qwen3-vl-30b");
    }
}
