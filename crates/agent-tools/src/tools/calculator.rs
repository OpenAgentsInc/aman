//! Calculator tool for safe math expression evaluation.

use async_trait::async_trait;
use tracing::debug;

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// Calculator tool that safely evaluates mathematical expressions.
///
/// Uses the `meval` crate for parsing and evaluation, which only supports
/// mathematical operations (no code execution, no side effects).
///
/// # Parameters
///
/// - `expression` (required): The mathematical expression to evaluate.
///
/// # Examples
///
/// ```json
/// {"expression": "2 + 2 * 3"}
/// {"expression": "sqrt(16) + sin(3.14159/2)"}
/// {"expression": "(10 - 5) / 2"}
/// ```
pub struct Calculator;

impl Calculator {
    /// Create a new calculator tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for Calculator {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Evaluates mathematical expressions safely. Supports basic arithmetic, \
         trigonometric functions (sin, cos, tan), logarithms (ln, log), \
         powers (^), roots (sqrt), and constants (pi, e)."
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let expression = args.get_string("expression")?;

        debug!("Evaluating expression: {}", expression);

        // Use meval for safe expression evaluation
        match meval::eval_str(&expression) {
            Ok(result) => {
                // Format the result nicely
                let formatted = if result.fract() == 0.0 && result.abs() < 1e15 {
                    // Integer-like result
                    format!("{:.0}", result)
                } else {
                    // Floating point result
                    format!("{}", result)
                };

                debug!("Result: {}", formatted);
                Ok(ToolOutput::success(format!("{} = {}", expression, formatted)))
            }
            Err(e) => {
                debug!("Evaluation error: {}", e);
                Err(ToolError::EvalError(format!(
                    "Failed to evaluate '{}': {}",
                    expression, e
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::Value;

    fn make_args(expr: &str) -> ToolArgs {
        let mut params = HashMap::new();
        params.insert("expression".to_string(), Value::String(expr.to_string()));
        ToolArgs::new(params)
    }

    #[tokio::test]
    async fn test_basic_arithmetic() {
        let calc = Calculator::new();

        let result = calc.execute(make_args("2 + 2")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("4"));

        let result = calc.execute(make_args("10 - 3")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("7"));

        let result = calc.execute(make_args("4 * 5")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("20"));

        let result = calc.execute(make_args("15 / 3")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("5"));
    }

    #[tokio::test]
    async fn test_order_of_operations() {
        let calc = Calculator::new();

        let result = calc.execute(make_args("2 + 3 * 4")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("14"));

        let result = calc.execute(make_args("(2 + 3) * 4")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("20"));
    }

    #[tokio::test]
    async fn test_functions() {
        let calc = Calculator::new();

        let result = calc.execute(make_args("sqrt(16)")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("4"));

        let result = calc.execute(make_args("abs(-5)")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("5"));
    }

    #[tokio::test]
    async fn test_constants() {
        let calc = Calculator::new();

        let result = calc.execute(make_args("pi")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("3.14"));

        let result = calc.execute(make_args("e")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("2.71"));
    }

    #[tokio::test]
    async fn test_invalid_expression() {
        let calc = Calculator::new();

        let result = calc.execute(make_args("2 +")).await;
        assert!(matches!(result, Err(ToolError::EvalError(_))));

        let result = calc.execute(make_args("undefined_var")).await;
        assert!(matches!(result, Err(ToolError::EvalError(_))));
    }

    #[tokio::test]
    async fn test_missing_expression() {
        let calc = Calculator::new();
        let args = ToolArgs::new(HashMap::new());

        let result = calc.execute(args).await;
        assert!(matches!(result, Err(ToolError::MissingParameter(_))));
    }
}
