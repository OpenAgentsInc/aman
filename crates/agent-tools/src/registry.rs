//! Tool registry for managing and executing tools.

use std::collections::HashMap;
use std::sync::Arc;

use brain_core::Brain;
use serde_json::Value;
use tracing::{debug, info};

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// Registry for managing tools.
///
/// The registry holds a collection of tools and can dispatch execution
/// requests to the appropriate tool by name.
pub struct ToolRegistry {
    /// Registered tools by name.
    tools: HashMap<String, Arc<dyn Tool>>,
    /// Optional shared brain for tools that need AI processing.
    brain: Option<Arc<dyn Brain>>,
}

impl ToolRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            brain: None,
        }
    }

    /// Create a registry with a shared brain for AI-powered tools.
    pub fn with_brain(brain: Arc<dyn Brain>) -> Self {
        Self {
            tools: HashMap::new(),
            brain: Some(brain),
        }
    }

    /// Set the brain for AI-powered tools.
    pub fn set_brain(&mut self, brain: Arc<dyn Brain>) {
        self.brain = Some(brain);
    }

    /// Register a tool.
    ///
    /// If a tool with the same name already exists, it will be replaced.
    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_string();
        info!("Registering tool: {}", name);
        self.tools.insert(name, Arc::new(tool));
    }

    /// Register a boxed tool.
    pub fn register_boxed(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        info!("Registering tool: {}", name);
        self.tools.insert(name, tool);
    }

    /// Get a list of registered tool names.
    pub fn list_tools(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// Check if a tool is registered.
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get tool descriptions for help text.
    pub fn get_descriptions(&self) -> Vec<(&str, &str)> {
        self.tools
            .values()
            .map(|t| (t.name(), t.description()))
            .collect()
    }

    /// Execute a tool by name with the given parameters.
    ///
    /// The registry will automatically inject the shared brain if available.
    pub async fn execute(
        &self,
        name: &str,
        params: HashMap<String, Value>,
    ) -> Result<ToolOutput, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;

        debug!("Executing tool '{}' with {} params", name, params.len());

        let args = if let Some(ref brain) = self.brain {
            ToolArgs::with_brain(params, brain.clone())
        } else {
            ToolArgs::new(params)
        };

        let result = tool.execute(args).await?;

        debug!(
            "Tool '{}' completed: success={}, content_len={}",
            name,
            result.success,
            result.content.len()
        );

        Ok(result)
    }

    /// Execute a tool with JSON arguments string.
    ///
    /// This is a convenience method that parses the JSON string into parameters.
    pub async fn execute_json(
        &self,
        name: &str,
        args_json: &str,
    ) -> Result<ToolOutput, ToolError> {
        let params: HashMap<String, Value> = serde_json::from_str(args_json)?;
        self.execute(name, params).await
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn description(&self) -> &str {
            "Echoes back the input"
        }

        async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
            let message = args.get_string("message")?;
            Ok(ToolOutput::success(message))
        }
    }

    #[tokio::test]
    async fn test_registry_basic() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);

        assert!(registry.has_tool("echo"));
        assert!(!registry.has_tool("nonexistent"));
        assert_eq!(registry.list_tools(), vec!["echo"]);
    }

    #[tokio::test]
    async fn test_registry_execute() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);

        let mut params = HashMap::new();
        params.insert("message".to_string(), Value::String("hello".to_string()));

        let result = registry.execute("echo", params).await.unwrap();
        assert!(result.success);
        assert_eq!(result.content, "hello");
    }

    #[tokio::test]
    async fn test_registry_execute_json() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);

        let result = registry
            .execute_json("echo", r#"{"message": "world"}"#)
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.content, "world");
    }

    #[tokio::test]
    async fn test_registry_not_found() {
        let registry = ToolRegistry::new();
        let result = registry.execute("nonexistent", HashMap::new()).await;
        assert!(matches!(result, Err(ToolError::NotFound(_))));
    }
}
