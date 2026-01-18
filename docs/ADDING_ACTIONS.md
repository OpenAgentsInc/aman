# Adding Actions to Aman

This guide explains how to add new actions to the `orchestrator` crate. Actions are the building blocks of message processing - they define what the bot can do in response to user messages.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                          ROUTER                                  │
│  Analyzes message → Outputs JSON RoutingPlan with actions       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      ROUTING PLAN                                │
│  { "actions": [                                                  │
│      {"type": "search", "query": "..."},                        │
│      {"type": "respond", "sensitivity": "insensitive"}          │
│  ]}                                                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      ORCHESTRATOR                                │
│  execute_plan() loops through actions and dispatches each       │
│  match action { Search => ..., Respond => ..., YourAction => }  │
└─────────────────────────────────────────────────────────────────┘
```

**Key components:**

| Component | Location | Purpose |
|-----------|----------|---------|
| `OrchestratorAction` | `crates/orchestrator/src/actions.rs:211-317` | Enum of all action types |
| Builder methods | `crates/orchestrator/src/actions.rs:319-540` | Helpers to create actions |
| `RoutingPlan` | `crates/orchestrator/src/actions.rs:89-208` | Holds vec of actions |
| `execute_plan()` | `crates/orchestrator/src/orchestrator.rs:318-469` | Dispatches actions |
| `ROUTER_PROMPT.md` | Project root | Instructs router how to classify |

## The OrchestratorAction Enum

Actions are defined as an enum with serde tagging for JSON serialization:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestratorAction {
    /// Search for real-time information before responding.
    Search {
        query: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    /// Clear conversation context.
    ClearContext {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    /// Show help information.
    Help,

    /// Generate final response.
    Respond {
        #[serde(default)]
        sensitivity: Sensitivity,
        #[serde(default)]
        task_hint: TaskHint,
        #[serde(default)]
        has_pii: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pii_types: Vec<String>,
    },

    // ... other actions
}
```

**Serde attributes explained:**
- `#[serde(tag = "type")]` - JSON uses `{"type": "action_name", ...}` format
- `#[serde(rename_all = "snake_case")]` - Rust `ClearContext` becomes JSON `"clear_context"`
- `#[serde(default)]` - Missing fields use `Default::default()`
- `#[serde(skip_serializing_if = "...")]` - Omit field if condition true

## Action Patterns

### Context-accumulating Actions

These actions gather information and add it to the processing context, but don't return a response themselves:

```rust
OrchestratorAction::Search { query, message } => {
    self.execute_search(..., &mut context, ...).await?;
    // Continues to next action in plan
}

OrchestratorAction::UseTool { name, args, message } => {
    self.execute_use_tool(..., &mut context, ...).await?;
    // Continues to next action in plan
}
```

**Characteristics:**
- Add results to `Context` for later use
- Don't return from `execute_plan()`
- Always followed by other actions (usually `Respond`)
- May send status messages to user

### Terminal Actions

These actions complete the processing and return immediately:

```rust
OrchestratorAction::Help => {
    return Ok(OutboundMessage::reply_to(message, HELP_TEXT));
}

OrchestratorAction::Respond { sensitivity, task_hint, .. } => {
    return self.execute_respond(message, &context, sensitivity, task_hint, history_key).await;
}

OrchestratorAction::Skip { reason } => {
    return Err(OrchestratorError::Skipped(reason.clone()));
}
```

**Characteristics:**
- Return `Ok(OutboundMessage)` or `Err(OrchestratorError)`
- End the action chain
- No actions after them will execute

### State-modifying Actions

These actions modify bot state and may either continue or return:

```rust
OrchestratorAction::ClearContext { .. } => {
    self.execute_clear_context(history_key, &message.sender).await?;
    // Continues to next action (usually Respond)
}

OrchestratorAction::SetPreference { preference } => {
    return self.execute_set_preference(message, preference, history_key).await;
}
```

## Step-by-Step Guide

### Example: Adding a "Translate" Action

Let's add a hypothetical action that translates text to another language.

### 1. Add Variant to OrchestratorAction

In `crates/orchestrator/src/actions.rs`, add the new variant:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestratorAction {
    // ... existing variants ...

    /// Translate text to another language.
    Translate {
        /// Target language code (e.g., "es", "fr", "de").
        target_language: String,
        /// Optional source language (auto-detect if not provided).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_language: Option<String>,
        /// Status message to show user.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
}
```

### 2. Add Builder Method

Add a constructor method for convenient action creation:

```rust
impl OrchestratorAction {
    // ... existing methods ...

    /// Create a translate action.
    pub fn translate(target_language: impl Into<String>) -> Self {
        Self::Translate {
            target_language: target_language.into(),
            source_language: None,
            message: None,
        }
    }

    /// Create a translate action with source language.
    pub fn translate_with_source(
        target_language: impl Into<String>,
        source_language: impl Into<String>,
    ) -> Self {
        Self::Translate {
            target_language: target_language.into(),
            source_language: Some(source_language.into()),
            message: None,
        }
    }

    /// Create a translate action with status message.
    pub fn translate_with_message(
        target_language: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Translate {
            target_language: target_language.into(),
            source_language: None,
            message: Some(message.into()),
        }
    }
}
```

### 3. Add to description() Method

Update the `description()` method for logging/debugging:

```rust
impl OrchestratorAction {
    pub fn description(&self) -> String {
        match self {
            // ... existing matches ...

            Self::Translate {
                target_language,
                source_language,
                ..
            } => {
                if let Some(src) = source_language {
                    format!("Translate from {} to {}", src, target_language)
                } else {
                    format!("Translate to {}", target_language)
                }
            }
        }
    }
}
```

### 4. (Optional) Add RoutingPlan Helper

If you need to check for this action in plans:

```rust
impl RoutingPlan {
    // ... existing methods ...

    /// Check if the plan contains a translate action.
    pub fn has_translate(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a, OrchestratorAction::Translate { .. }))
    }
}
```

### 5. Add Match Arm in execute_plan()

In `crates/orchestrator/src/orchestrator.rs`, add the dispatch case:

```rust
async fn execute_plan(
    &self,
    message: &InboundMessage,
    plan: &RoutingPlan,
    recipient: &str,
    is_group: bool,
    history_key: &str,
) -> Result<OutboundMessage, OrchestratorError> {
    let mut context = Context::new();

    for action in &plan.actions {
        match action {
            // ... existing matches ...

            OrchestratorAction::Translate {
                target_language,
                source_language,
                message: status_msg,
            } => {
                self.execute_translate(
                    message,
                    history_key,
                    target_language,
                    source_language.as_deref(),
                    status_msg.as_deref(),
                    &mut context,
                    recipient,
                    is_group,
                )
                .await?;
            }
        }
    }

    // ... rest of method
}
```

### 6. Implement the execute_* Method

Add the implementation method in `orchestrator.rs`:

```rust
impl<S: MessageSender> Orchestrator<S> {
    // ... existing methods ...

    /// Execute a translate action.
    async fn execute_translate(
        &self,
        message: &InboundMessage,
        history_key: &str,
        target_language: &str,
        source_language: Option<&str>,
        status_message: Option<&str>,
        context: &mut Context,
        recipient: &str,
        is_group: bool,
    ) -> Result<(), OrchestratorError> {
        info!(
            "Translating to {} (source: {:?})",
            target_language,
            source_language
        );

        // Send status message if provided
        if let Some(msg) = status_message {
            if let Err(e) = self.sender.send_message(recipient, msg, is_group).await {
                warn!("Failed to send translate status: {}", e);
            }

            // Restart typing indicator
            if let Err(e) = self.sender.set_typing(recipient, is_group, true).await {
                warn!("Failed to restart typing indicator: {}", e);
            }
        }

        // Call translation service (could use a brain or external API)
        let translated = self
            .maple_brain
            .translate(&message.text, target_language, source_language)
            .await
            .map_err(|e| OrchestratorError::ToolFailed(format!("Translation failed: {}", e)))?;

        // Add to context for the Respond action to use
        context.add_tool_result("translate", &translated);

        // Record in memory if enabled
        self.record_tool_history(history_key, "translate", true, &translated, message)
            .await;

        Ok(())
    }
}
```

### 7. Update ROUTER_PROMPT.md

Add documentation for the router to understand and emit your action:

```markdown
### Control Actions
- "clear_context": Clear conversation history.
- "translate": Translate the user's message. Include:
  - "target_language": Language code (e.g., "es", "fr", "de", "ja")
  - "source_language": (optional) Source language code
  - "message": (optional) Status message like "Translating..."
- "help": User is asking about bot capabilities.
```

Add examples:

```markdown
[MESSAGE: translate this to Spanish: Hello, how are you?]
[ATTACHMENTS: none]
→ {"actions": [{"type": "translate", "target_language": "es", "message": "Translating..."}, {"type": "respond", "sensitivity": "insensitive", "task_hint": "multilingual"}]}

[MESSAGE: say "good morning" in Japanese]
[ATTACHMENTS: none]
→ {"actions": [{"type": "translate", "target_language": "ja", "message": "Translating..."}, {"type": "respond", "sensitivity": "insensitive", "task_hint": "multilingual"}]}

[MESSAGE: translate from French to English: Bonjour le monde]
[ATTACHMENTS: none]
→ {"actions": [{"type": "translate", "target_language": "en", "source_language": "fr"}, {"type": "respond", "sensitivity": "insensitive", "task_hint": "multilingual"}]}
```

### 8. Add Serialization Tests

Add tests in `crates/orchestrator/src/actions.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ... existing tests ...

    #[test]
    fn test_parse_translate() {
        let json = r#"{"actions": [{"type": "translate", "target_language": "es"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();
        assert!(plan.has_translate());

        if let OrchestratorAction::Translate {
            target_language,
            source_language,
            message,
        } = &plan.actions[0]
        {
            assert_eq!(target_language, "es");
            assert!(source_language.is_none());
            assert!(message.is_none());
        } else {
            panic!("Expected Translate action");
        }
    }

    #[test]
    fn test_parse_translate_with_source() {
        let json = r#"{"actions": [{"type": "translate", "target_language": "en", "source_language": "fr"}]}"#;
        let plan: RoutingPlan = serde_json::from_str(json).unwrap();

        if let OrchestratorAction::Translate {
            target_language,
            source_language,
            ..
        } = &plan.actions[0]
        {
            assert_eq!(target_language, "en");
            assert_eq!(source_language.as_deref(), Some("fr"));
        } else {
            panic!("Expected Translate action");
        }
    }

    #[test]
    fn test_serialize_translate() {
        let plan = RoutingPlan::new(vec![
            OrchestratorAction::translate("ja"),
            OrchestratorAction::respond(Sensitivity::Insensitive),
        ]);

        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("translate"));
        assert!(json.contains("\"target_language\":\"ja\""));
    }

    #[test]
    fn test_translate_description() {
        let action = OrchestratorAction::translate("es");
        assert!(action.description().contains("Translate to es"));

        let action_with_src = OrchestratorAction::translate_with_source("en", "de");
        assert!(action_with_src.description().contains("from de to en"));
    }
}
```

## Existing Actions Reference

| Action | Type | Purpose | Fields |
|--------|------|---------|--------|
| `Search` | Context-accumulating | Real-time web search | `query`, `message` |
| `UseTool` | Context-accumulating | Execute registry tool | `name`, `args`, `message` |
| `ClearContext` | State-modifying | Clear conversation history | `message` |
| `SetPreference` | Terminal | Set user agent preference | `preference` |
| `Help` | Terminal | Show help text | (none) |
| `Respond` | Terminal | Generate AI response | `sensitivity`, `task_hint`, `has_pii`, `pii_types` |
| `Grok` | Terminal | Direct Grok query | `query`, `task_hint` |
| `Maple` | Terminal | Direct Maple query | `query`, `task_hint` |
| `Skip` | Terminal (error) | Skip with reason | `reason` |
| `Ignore` | Terminal (error) | Silent ignore | (none) |
| `AskPrivacyChoice` | Terminal | Ask about PII handling | `pii_types`, `original_message`, `sensitivity`, `task_hint` |
| `PrivacyChoiceResponse` | Terminal | Handle PII choice | `choice` |

## Checklist

When adding a new action, ensure you've completed:

- [ ] Added variant to `OrchestratorAction` enum in `actions.rs`
- [ ] Used appropriate serde attributes (`#[serde(default)]`, etc.)
- [ ] Added builder method(s) (e.g., `OrchestratorAction::new_action()`)
- [ ] Updated `description()` method with new match arm
- [ ] (Optional) Added `RoutingPlan::has_*()` helper method
- [ ] Added match arm in `execute_plan()` in `orchestrator.rs`
- [ ] Implemented `execute_*()` method in `orchestrator.rs`
- [ ] Updated `ROUTER_PROMPT.md` with action documentation
- [ ] Added examples to `ROUTER_PROMPT.md`
- [ ] Added serialization/deserialization tests
- [ ] Ran `cargo test -p orchestrator` to verify tests pass
- [ ] Ran `cargo build -p orchestrator` to verify compilation

## Testing Your Action

```bash
# Run unit tests
cd crates/orchestrator && cargo test

# Test via the bot
./scripts/dev.sh --build
# Send messages that should trigger your new action

# Check logs to see action being parsed and executed
RUST_LOG=debug ./scripts/dev.sh
```

## Tips

1. **Keep actions focused** - Each action should do one thing well
2. **Use optional fields** - Allow the router flexibility with `#[serde(default)]`
3. **Add status messages** - Users appreciate knowing what's happening
4. **Context-accumulating vs Terminal** - Decide early if your action returns or continues
5. **Test serialization** - Router outputs JSON, so test both parse and serialize
6. **Update the router prompt** - The router won't emit your action unless you document it

## See Also

- [Adding Tools](./ADDING_TOOLS.md) - How to add new tools
- [Architecture](./ARCHITECTURE.md) - Overall system architecture
- `crates/orchestrator/src/actions.rs` - Action definitions
- `crates/orchestrator/src/orchestrator.rs` - Action execution
- `ROUTER_PROMPT.md` - Router classification instructions
