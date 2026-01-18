//! Random number generator tool.

use async_trait::async_trait;
use rand::Rng;
use tracing::debug;

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// Random number generator tool.
///
/// Generates random numbers within a specified range. Can generate integers
/// or floating-point numbers, and can generate multiple numbers at once.
///
/// # Parameters
///
/// - `min` (optional): Minimum value (inclusive). Defaults to 1.
/// - `max` (optional): Maximum value (inclusive). Defaults to 100.
/// - `count` (optional): Number of random values to generate. Defaults to 1, max 100.
/// - `float` (optional): If true, generate floating-point numbers. Defaults to false.
///
/// # Examples
///
/// ```json
/// {}                                    // Random integer 1-100
/// {"min": 1, "max": 6}                  // Dice roll (1-6)
/// {"min": 1, "max": 100, "count": 5}    // 5 random numbers 1-100
/// {"min": 0.0, "max": 1.0, "float": true}  // Random float 0-1
/// ```
pub struct RandomNumber;

impl RandomNumber {
    /// Create a new random number generator tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for RandomNumber {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RandomNumber {
    fn name(&self) -> &str {
        "random_number"
    }

    fn description(&self) -> &str {
        "Generates random numbers. Parameters: min (default 1), max (default 100), \
         count (default 1, max 100), float (default false for integers)."
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let min = args.get_number_opt("min")?.unwrap_or(1.0);
        let max = args.get_number_opt("max")?.unwrap_or(100.0);
        let count = args.get_number_opt("count")?.unwrap_or(1.0) as usize;
        let use_float = args.get_bool_opt("float")?.unwrap_or(false);

        debug!("Generating {} random number(s) between {} and {} (float: {})", count, min, max, use_float);

        // Validate inputs
        if min > max {
            return Err(ToolError::InvalidParameter {
                name: "min/max".to_string(),
                reason: "min must be less than or equal to max".to_string(),
            });
        }

        if count == 0 {
            return Err(ToolError::InvalidParameter {
                name: "count".to_string(),
                reason: "count must be at least 1".to_string(),
            });
        }

        if count > 100 {
            return Err(ToolError::InvalidParameter {
                name: "count".to_string(),
                reason: "count cannot exceed 100".to_string(),
            });
        }

        let mut rng = rand::thread_rng();
        let numbers: Vec<String> = (0..count)
            .map(|_| {
                if use_float {
                    let value: f64 = rng.gen_range(min..=max);
                    format!("{:.4}", value).trim_end_matches('0').trim_end_matches('.').to_string()
                } else {
                    let min_int = min.floor() as i64;
                    let max_int = max.floor() as i64;
                    let value: i64 = rng.gen_range(min_int..=max_int);
                    value.to_string()
                }
            })
            .collect();

        let result = if count == 1 {
            numbers[0].clone()
        } else {
            numbers.join(", ")
        };

        debug!("Generated: {}", result);

        // Format output based on context
        let output = if count == 1 {
            if use_float {
                format!("Random number: {}", result)
            } else if min == 1.0 && max == 6.0 {
                format!("Dice roll: {}", result)
            } else if min == 0.0 && max == 1.0 {
                format!("Coin flip: {}", if result == "0" { "Heads" } else { "Tails" })
            } else {
                format!("Random number: {}", result)
            }
        } else {
            format!("{} random numbers: {}", count, result)
        };

        Ok(ToolOutput::success(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::Value;

    fn make_args_empty() -> ToolArgs {
        ToolArgs::new(HashMap::new())
    }

    fn make_args(min: f64, max: f64) -> ToolArgs {
        let mut params = HashMap::new();
        params.insert("min".to_string(), Value::Number(serde_json::Number::from_f64(min).unwrap()));
        params.insert("max".to_string(), Value::Number(serde_json::Number::from_f64(max).unwrap()));
        ToolArgs::new(params)
    }

    fn make_args_with_count(min: f64, max: f64, count: usize) -> ToolArgs {
        let mut params = HashMap::new();
        params.insert("min".to_string(), Value::Number(serde_json::Number::from_f64(min).unwrap()));
        params.insert("max".to_string(), Value::Number(serde_json::Number::from_f64(max).unwrap()));
        params.insert("count".to_string(), Value::Number(serde_json::Number::from_f64(count as f64).unwrap()));
        ToolArgs::new(params)
    }

    #[tokio::test]
    async fn test_default_range() {
        let gen = RandomNumber::new();
        let result = gen.execute(make_args_empty()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("Random number:"));
    }

    #[tokio::test]
    async fn test_dice_roll() {
        let gen = RandomNumber::new();

        for _ in 0..10 {
            let result = gen.execute(make_args(1.0, 6.0)).await.unwrap();
            assert!(result.success);
            assert!(result.content.contains("Dice roll:"));

            // Extract number and verify range
            let num: i32 = result.content
                .split(':')
                .last()
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            assert!(num >= 1 && num <= 6);
        }
    }

    #[tokio::test]
    async fn test_multiple_numbers() {
        let gen = RandomNumber::new();
        let result = gen.execute(make_args_with_count(1.0, 100.0, 5)).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("5 random numbers:"));

        // Should have 5 comma-separated values
        let numbers_part = result.content.split(':').last().unwrap().trim();
        let count = numbers_part.split(',').count();
        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn test_float_numbers() {
        let gen = RandomNumber::new();
        let mut params = HashMap::new();
        params.insert("min".to_string(), Value::Number(serde_json::Number::from_f64(0.0).unwrap()));
        params.insert("max".to_string(), Value::Number(serde_json::Number::from_f64(1.0).unwrap()));
        params.insert("float".to_string(), Value::Bool(true));
        let args = ToolArgs::new(params);

        let result = gen.execute(args).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_invalid_range() {
        let gen = RandomNumber::new();
        let result = gen.execute(make_args(100.0, 1.0)).await;
        assert!(matches!(result, Err(ToolError::InvalidParameter { .. })));
    }

    #[tokio::test]
    async fn test_count_too_large() {
        let gen = RandomNumber::new();
        let result = gen.execute(make_args_with_count(1.0, 100.0, 200)).await;
        assert!(matches!(result, Err(ToolError::InvalidParameter { .. })));
    }

    #[tokio::test]
    async fn test_zero_count() {
        let gen = RandomNumber::new();
        let result = gen.execute(make_args_with_count(1.0, 100.0, 0)).await;
        assert!(matches!(result, Err(ToolError::InvalidParameter { .. })));
    }
}
