//! ToolExecutor implementation backed by ToolRegistry.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use brain_core::{ToolExecutor, ToolRequest, ToolRequestMeta, ToolResult};
use indexmap::IndexMap;
use serde_json::{Map, Value};
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::{ToolOutput, ToolRegistry};

/// Default maximum entries in the rate limit cache before LRU eviction.
const DEFAULT_MAX_RATE_LIMIT_ENTRIES: usize = 10000;

/// Default maximum entries in the result cache before LRU eviction.
const DEFAULT_MAX_CACHE_ENTRIES: usize = 5000;

#[derive(Debug, Clone, Copy)]
pub struct RateLimit {
    pub max_requests: u32,
    pub window: Duration,
}

impl RateLimit {
    pub fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            max_requests,
            window,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolPolicy {
    pub allowlist: Option<HashSet<String>>,
    pub sender_allowlist: HashMap<String, HashSet<String>>,
    pub group_allowlist: HashMap<String, HashSet<String>>,
    pub rate_limit_per_sender: Option<RateLimit>,
    pub rate_limit_per_group: Option<RateLimit>,
    pub rate_limit_per_tool: HashMap<String, RateLimit>,
    pub timeout: Option<Duration>,
    pub cache_ttl: Option<Duration>,
    pub format_results_as_json: bool,
    /// Maximum entries in the rate limit cache before LRU eviction.
    pub max_rate_limit_entries: usize,
    /// Maximum entries in the result cache before LRU eviction.
    pub max_cache_entries: usize,
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self {
            allowlist: None,
            sender_allowlist: HashMap::new(),
            group_allowlist: HashMap::new(),
            rate_limit_per_sender: None,
            rate_limit_per_group: None,
            rate_limit_per_tool: HashMap::new(),
            timeout: None,
            cache_ttl: None,
            format_results_as_json: false,
            max_rate_limit_entries: DEFAULT_MAX_RATE_LIMIT_ENTRIES,
            max_cache_entries: DEFAULT_MAX_CACHE_ENTRIES,
        }
    }
}

impl ToolPolicy {
    pub fn allow_tools<I, S>(mut self, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let allowlist = self.allowlist.get_or_insert_with(HashSet::new);
        for tool in tools {
            allowlist.insert(tool.into());
        }
        self
    }

    pub fn allow_sender_tools<I, S>(mut self, sender: impl Into<String>, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let entry = self
            .sender_allowlist
            .entry(sender.into())
            .or_insert_with(HashSet::new);
        for tool in tools {
            entry.insert(tool.into());
        }
        self
    }

    pub fn allow_group_tools<I, S>(mut self, group_id: impl Into<String>, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let entry = self
            .group_allowlist
            .entry(group_id.into())
            .or_insert_with(HashSet::new);
        for tool in tools {
            entry.insert(tool.into());
        }
        self
    }

    pub fn with_sender_rate_limit(mut self, limit: RateLimit) -> Self {
        self.rate_limit_per_sender = Some(limit);
        self
    }

    pub fn with_group_rate_limit(mut self, limit: RateLimit) -> Self {
        self.rate_limit_per_group = Some(limit);
        self
    }

    pub fn with_tool_rate_limit(mut self, tool: impl Into<String>, limit: RateLimit) -> Self {
        self.rate_limit_per_tool.insert(tool.into(), limit);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = Some(ttl);
        self
    }

    pub fn with_json_results(mut self, enabled: bool) -> Self {
        self.format_results_as_json = enabled;
        self
    }
}

struct RateLimitState {
    window_start: Instant,
    count: u32,
}

struct CacheEntry {
    inserted_at: Instant,
    success: bool,
    content: String,
}

pub struct RegistryToolExecutor {
    registry: Arc<ToolRegistry>,
    policy: ToolPolicy,
    /// Rate limit state with LRU eviction (IndexMap preserves insertion order).
    rate_limits: Mutex<IndexMap<String, RateLimitState>>,
    /// Result cache with LRU eviction (IndexMap preserves insertion order).
    cache: Mutex<IndexMap<String, CacheEntry>>,
}

impl RegistryToolExecutor {
    pub fn new(registry: ToolRegistry) -> Self {
        Self::with_policy(registry, ToolPolicy::default())
    }

    pub fn with_policy(registry: ToolRegistry, policy: ToolPolicy) -> Self {
        Self {
            registry: Arc::new(registry),
            policy,
            rate_limits: Mutex::new(IndexMap::new()),
            cache: Mutex::new(IndexMap::new()),
        }
    }

    pub fn from_shared(registry: Arc<ToolRegistry>, policy: ToolPolicy) -> Self {
        Self {
            registry,
            policy,
            rate_limits: Mutex::new(IndexMap::new()),
            cache: Mutex::new(IndexMap::new()),
        }
    }

    pub fn registry(&self) -> &ToolRegistry {
        self.registry.as_ref()
    }

    pub fn policy(&self) -> &ToolPolicy {
        &self.policy
    }

    fn is_allowed(&self, tool: &str, metadata: Option<&ToolRequestMeta>) -> bool {
        if let Some(ref allowlist) = self.policy.allowlist {
            if !allowlist.contains(tool) {
                return false;
            }
        }

        if let Some(meta) = metadata {
            if let Some(sender) = meta.sender.as_ref() {
                if let Some(allowed) = self.policy.sender_allowlist.get(sender) {
                    return allowed.contains(tool);
                }
            }

            if let Some(group_id) = meta.group_id.as_ref() {
                if let Some(allowed) = self.policy.group_allowlist.get(group_id) {
                    return allowed.contains(tool);
                }
            }
        }

        true
    }

    fn rate_limit_for(&self, tool: &str, metadata: Option<&ToolRequestMeta>) -> Option<RateLimit> {
        if let Some(limit) = self.policy.rate_limit_per_tool.get(tool) {
            return Some(*limit);
        }

        if let Some(meta) = metadata {
            if meta.sender.is_some() {
                return self.policy.rate_limit_per_sender;
            }
            if meta.group_id.is_some() {
                return self.policy.rate_limit_per_group;
            }
        }

        None
    }

    fn rate_limit_key(tool: &str, metadata: Option<&ToolRequestMeta>) -> String {
        if let Some(meta) = metadata {
            if let Some(sender) = meta.sender.as_ref() {
                return format!("sender:{}:{}", sender, tool);
            }
            if let Some(group_id) = meta.group_id.as_ref() {
                return format!("group:{}:{}", group_id, tool);
            }
        }

        format!("global:{}", tool)
    }

    async fn check_rate_limit(
        &self,
        tool: &str,
        metadata: Option<&ToolRequestMeta>,
    ) -> Result<(), String> {
        let limit = match self.rate_limit_for(tool, metadata) {
            Some(limit) => limit,
            None => return Ok(()),
        };

        let key = Self::rate_limit_key(tool, metadata);
        let mut state = self.rate_limits.lock().await;

        // Move to end if exists (LRU behavior)
        let entry = if let Some(existing) = state.shift_remove(&key) {
            state.insert(key.clone(), existing);
            state.get_mut(&key).unwrap()
        } else {
            state.insert(
                key.clone(),
                RateLimitState {
                    window_start: Instant::now(),
                    count: 0,
                },
            );
            state.get_mut(&key).unwrap()
        };

        let now = Instant::now();
        if now.duration_since(entry.window_start) >= limit.window {
            entry.window_start = now;
            entry.count = 0;
        }

        if entry.count >= limit.max_requests {
            return Err("Rate limit exceeded".to_string());
        }

        entry.count += 1;

        // LRU eviction: remove oldest entries if we exceed max
        while state.len() > self.policy.max_rate_limit_entries {
            state.shift_remove_index(0);
        }

        Ok(())
    }

    fn cache_key(tool: &str, args: &HashMap<String, Value>) -> String {
        let mut keys: Vec<&String> = args.keys().collect();
        keys.sort();

        let mut map = Map::new();
        for key in keys {
            if let Some(value) = args.get(key) {
                map.insert(key.clone(), value.clone());
            }
        }

        let args_json = Value::Object(map).to_string();
        format!("{}|{}", tool, args_json)
    }

    async fn try_cache(
        &self,
        tool: &str,
        args: &HashMap<String, Value>,
    ) -> Option<ToolResult> {
        let ttl = self.policy.cache_ttl?;
        let key = Self::cache_key(tool, args);
        let mut cache = self.cache.lock().await;

        // Move to end if exists (LRU behavior) and check expiry
        if let Some(entry) = cache.shift_remove(&key) {
            if entry.inserted_at.elapsed() > ttl {
                // Expired, don't re-insert
                return None;
            }
            // Re-insert at end (LRU)
            let result = if entry.success {
                ToolResult::success("cached", entry.content.clone())
            } else {
                ToolResult::error("cached", entry.content.clone())
            };
            cache.insert(key, entry);
            return Some(result);
        }
        None
    }

    async fn store_cache(
        &self,
        tool: &str,
        args: &HashMap<String, Value>,
        output: &ToolOutput,
    ) {
        if self.policy.cache_ttl.is_none() {
            return;
        }
        let key = Self::cache_key(tool, args);
        let mut cache = self.cache.lock().await;

        // Remove if exists to update position (LRU)
        cache.shift_remove(&key);
        cache.insert(
            key,
            CacheEntry {
                inserted_at: Instant::now(),
                success: output.success,
                content: output.content.clone(),
            },
        );

        // LRU eviction: remove oldest entries if we exceed max
        while cache.len() > self.policy.max_cache_entries {
            cache.shift_remove_index(0);
        }
    }

    fn format_result(&self, tool: &str, output: &ToolOutput) -> String {
        if self.policy.format_results_as_json {
            serde_json::json!({
                "tool": tool,
                "success": output.success,
                "content": output.content,
            })
            .to_string()
        } else {
            output.content.clone()
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for RegistryToolExecutor {
    async fn execute(&self, request: ToolRequest) -> ToolResult {
        if !self.is_allowed(&request.name, request.metadata.as_ref()) {
            return ToolResult::error(&request.id, "Tool not allowed");
        }

        if let Err(error) = self
            .check_rate_limit(&request.name, request.metadata.as_ref())
            .await
        {
            return ToolResult::error(&request.id, error);
        }

        if let Some(mut cached) = self.try_cache(&request.name, &request.arguments).await {
            cached.tool_call_id = request.id;
            return cached;
        }

        let execute_future = self
            .registry
            .execute(&request.name, request.arguments.clone());

        let output = match self.policy.timeout {
            Some(timeout_duration) => match timeout(timeout_duration, execute_future).await {
                Ok(result) => match result {
                    Ok(output) => output,
                    Err(error) => {
                        return ToolResult::error(&request.id, error.to_string());
                    }
                },
                Err(_) => {
                    return ToolResult::error(&request.id, "Tool execution timed out");
                }
            },
            None => match execute_future.await {
                Ok(output) => output,
                Err(error) => return ToolResult::error(&request.id, error.to_string()),
            },
        };

        self.store_cache(&request.name, &request.arguments, &output)
            .await;

        let formatted = self.format_result(&request.name, &output);
        if output.success {
            ToolResult::success(&request.id, formatted)
        } else {
            ToolResult::error(&request.id, formatted)
        }
    }

    fn supported_tools(&self) -> Vec<&str> {
        self.registry.list_tools()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tool, ToolArgs, ToolError, ToolOutput};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingTool {
        count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Tool for CountingTool {
        fn name(&self) -> &str {
            "counting_tool"
        }

        fn description(&self) -> &str {
            "Counts executions"
        }

        async fn execute(&self, _args: ToolArgs) -> Result<ToolOutput, ToolError> {
            let current = self.count.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(ToolOutput::success(format!("count: {}", current)))
        }
    }

    fn registry_with_counter(counter: Arc<AtomicUsize>) -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        registry.register(CountingTool { count: counter });
        registry
    }

    #[tokio::test]
    async fn test_allowlist_blocks() {
        let counter = Arc::new(AtomicUsize::new(0));
        let registry = registry_with_counter(counter.clone());
        let policy = ToolPolicy::default().allow_tools(["other_tool"]);
        let executor = RegistryToolExecutor::with_policy(registry, policy);

        let request = ToolRequest {
            id: "1".to_string(),
            name: "counting_tool".to_string(),
            arguments: HashMap::new(),
            metadata: None,
        };

        let result = executor.execute(request).await;
        assert!(!result.success);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_rate_limit_blocks() {
        let counter = Arc::new(AtomicUsize::new(0));
        let registry = registry_with_counter(counter.clone());
        let policy = ToolPolicy::default().with_sender_rate_limit(RateLimit::new(
            1,
            Duration::from_secs(60),
        ));
        let executor = RegistryToolExecutor::with_policy(registry, policy);

        let metadata = ToolRequestMeta {
            sender: Some("user1".to_string()),
            group_id: None,
            is_group: Some(false),
        };

        let request = ToolRequest {
            id: "1".to_string(),
            name: "counting_tool".to_string(),
            arguments: HashMap::new(),
            metadata: Some(metadata.clone()),
        };

        let result = executor.execute(request.clone()).await;
        assert!(result.success);

        let result = executor.execute(request).await;
        assert!(!result.success);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_cache_hits() {
        let counter = Arc::new(AtomicUsize::new(0));
        let registry = registry_with_counter(counter.clone());
        let policy = ToolPolicy::default().with_cache_ttl(Duration::from_secs(60));
        let executor = RegistryToolExecutor::with_policy(registry, policy);

        let mut args = HashMap::new();
        args.insert("key".to_string(), Value::String("value".to_string()));

        let request = ToolRequest {
            id: "1".to_string(),
            name: "counting_tool".to_string(),
            arguments: args.clone(),
            metadata: None,
        };

        let result = executor.execute(request.clone()).await;
        assert!(result.success);
        let result = executor.execute(request).await;
        assert!(result.success);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
