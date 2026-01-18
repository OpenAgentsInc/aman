//! Unit converter tool for converting between common units.

use async_trait::async_trait;
use tracing::debug;

use crate::error::ToolError;
use crate::tool::{Tool, ToolArgs, ToolOutput};

/// Unit converter tool for converting between common measurement units.
///
/// Supports conversions for:
/// - Length: meters, kilometers, miles, feet, inches, centimeters, yards
/// - Weight: kilograms, pounds, ounces, grams, stones
/// - Temperature: celsius, fahrenheit, kelvin
/// - Volume: liters, gallons, milliliters, cups, pints, quarts
/// - Area: square meters, square feet, acres, hectares
/// - Speed: km/h, mph, m/s, knots
/// - Data: bytes, kilobytes, megabytes, gigabytes, terabytes
///
/// # Parameters
///
/// - `value` (required): The numeric value to convert.
/// - `from` (required): The source unit (e.g., "km", "miles", "celsius").
/// - `to` (required): The target unit (e.g., "miles", "km", "fahrenheit").
///
/// # Examples
///
/// ```json
/// {"value": 100, "from": "km", "to": "miles"}
/// {"value": 32, "from": "fahrenheit", "to": "celsius"}
/// {"value": 5.5, "from": "kg", "to": "lb"}
/// ```
pub struct UnitConverter;

impl UnitConverter {
    /// Create a new unit converter tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnitConverter {
    fn default() -> Self {
        Self::new()
    }
}

/// Conversion factor to a base unit within each category.
struct ConversionFactor {
    /// Factor to multiply by to get to base unit.
    to_base: f64,
    /// Category of the unit (e.g., "length", "weight").
    category: &'static str,
}

fn get_conversion(unit: &str) -> Option<ConversionFactor> {
    let unit_lower = unit.to_lowercase();
    let unit = unit_lower.as_str();

    match unit {
        // Length (base: meters)
        "m" | "meter" | "meters" => Some(ConversionFactor { to_base: 1.0, category: "length" }),
        "km" | "kilometer" | "kilometers" => Some(ConversionFactor { to_base: 1000.0, category: "length" }),
        "cm" | "centimeter" | "centimeters" => Some(ConversionFactor { to_base: 0.01, category: "length" }),
        "mm" | "millimeter" | "millimeters" => Some(ConversionFactor { to_base: 0.001, category: "length" }),
        "mi" | "mile" | "miles" => Some(ConversionFactor { to_base: 1609.344, category: "length" }),
        "ft" | "foot" | "feet" => Some(ConversionFactor { to_base: 0.3048, category: "length" }),
        "in" | "inch" | "inches" => Some(ConversionFactor { to_base: 0.0254, category: "length" }),
        "yd" | "yard" | "yards" => Some(ConversionFactor { to_base: 0.9144, category: "length" }),
        "nm" | "nautical mile" | "nautical miles" => Some(ConversionFactor { to_base: 1852.0, category: "length" }),

        // Weight (base: kilograms)
        "kg" | "kilogram" | "kilograms" => Some(ConversionFactor { to_base: 1.0, category: "weight" }),
        "g" | "gram" | "grams" => Some(ConversionFactor { to_base: 0.001, category: "weight" }),
        "mg" | "milligram" | "milligrams" => Some(ConversionFactor { to_base: 0.000001, category: "weight" }),
        "lb" | "lbs" | "pound" | "pounds" => Some(ConversionFactor { to_base: 0.453592, category: "weight" }),
        "oz" | "ounce" | "ounces" => Some(ConversionFactor { to_base: 0.0283495, category: "weight" }),
        "st" | "stone" | "stones" => Some(ConversionFactor { to_base: 6.35029, category: "weight" }),
        "t" | "ton" | "tons" | "tonne" | "tonnes" => Some(ConversionFactor { to_base: 1000.0, category: "weight" }),

        // Volume (base: liters)
        "l" | "liter" | "liters" | "litre" | "litres" => Some(ConversionFactor { to_base: 1.0, category: "volume" }),
        "ml" | "milliliter" | "milliliters" | "millilitre" | "millilitres" => Some(ConversionFactor { to_base: 0.001, category: "volume" }),
        "gal" | "gallon" | "gallons" => Some(ConversionFactor { to_base: 3.78541, category: "volume" }),
        "qt" | "quart" | "quarts" => Some(ConversionFactor { to_base: 0.946353, category: "volume" }),
        "pt" | "pint" | "pints" => Some(ConversionFactor { to_base: 0.473176, category: "volume" }),
        "cup" | "cups" => Some(ConversionFactor { to_base: 0.236588, category: "volume" }),
        "fl oz" | "fluid ounce" | "fluid ounces" | "floz" => Some(ConversionFactor { to_base: 0.0295735, category: "volume" }),

        // Area (base: square meters)
        "m2" | "sqm" | "square meter" | "square meters" => Some(ConversionFactor { to_base: 1.0, category: "area" }),
        "km2" | "sqkm" | "square kilometer" | "square kilometers" => Some(ConversionFactor { to_base: 1_000_000.0, category: "area" }),
        "ft2" | "sqft" | "square foot" | "square feet" => Some(ConversionFactor { to_base: 0.092903, category: "area" }),
        "acre" | "acres" => Some(ConversionFactor { to_base: 4046.86, category: "area" }),
        "ha" | "hectare" | "hectares" => Some(ConversionFactor { to_base: 10000.0, category: "area" }),

        // Speed (base: m/s)
        "m/s" | "mps" => Some(ConversionFactor { to_base: 1.0, category: "speed" }),
        "km/h" | "kph" | "kmh" => Some(ConversionFactor { to_base: 0.277778, category: "speed" }),
        "mph" => Some(ConversionFactor { to_base: 0.44704, category: "speed" }),
        "knot" | "knots" | "kn" => Some(ConversionFactor { to_base: 0.514444, category: "speed" }),
        "ft/s" | "fps" => Some(ConversionFactor { to_base: 0.3048, category: "speed" }),

        // Data (base: bytes)
        "b" | "byte" | "bytes" => Some(ConversionFactor { to_base: 1.0, category: "data" }),
        "kb" | "kilobyte" | "kilobytes" => Some(ConversionFactor { to_base: 1024.0, category: "data" }),
        "mb" | "megabyte" | "megabytes" => Some(ConversionFactor { to_base: 1_048_576.0, category: "data" }),
        "gb" | "gigabyte" | "gigabytes" => Some(ConversionFactor { to_base: 1_073_741_824.0, category: "data" }),
        "tb" | "terabyte" | "terabytes" => Some(ConversionFactor { to_base: 1_099_511_627_776.0, category: "data" }),

        // Temperature is handled specially
        "c" | "celsius" => Some(ConversionFactor { to_base: 1.0, category: "temperature" }),
        "f" | "fahrenheit" => Some(ConversionFactor { to_base: 1.0, category: "temperature" }),
        "k" | "kelvin" => Some(ConversionFactor { to_base: 1.0, category: "temperature" }),

        _ => None,
    }
}

/// Convert temperature values (special case, not linear).
fn convert_temperature(value: f64, from: &str, to: &str) -> f64 {
    let from = from.to_lowercase();
    let to = to.to_lowercase();

    // First convert to Celsius
    let celsius = match from.as_str() {
        "c" | "celsius" => value,
        "f" | "fahrenheit" => (value - 32.0) * 5.0 / 9.0,
        "k" | "kelvin" => value - 273.15,
        _ => value,
    };

    // Then convert from Celsius to target
    match to.as_str() {
        "c" | "celsius" => celsius,
        "f" | "fahrenheit" => celsius * 9.0 / 5.0 + 32.0,
        "k" | "kelvin" => celsius + 273.15,
        _ => celsius,
    }
}

/// Format a number nicely.
fn format_number(value: f64) -> String {
    if value.abs() < 0.0001 || value.abs() >= 1e10 {
        format!("{:.4e}", value)
    } else if value.fract().abs() < 0.0001 {
        format!("{:.0}", value)
    } else if value.abs() < 1.0 {
        format!("{:.6}", value).trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        format!("{:.4}", value).trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[async_trait]
impl Tool for UnitConverter {
    fn name(&self) -> &str {
        "unit_converter"
    }

    fn description(&self) -> &str {
        "Converts values between common units. Supports length (km, miles, feet, meters), \
         weight (kg, lb, oz), temperature (celsius, fahrenheit, kelvin), volume (liters, gallons), \
         area (sqft, acres), speed (km/h, mph), and data (KB, MB, GB)."
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let value = args.get_number("value")?;
        let from = args.get_string("from")?;
        let to = args.get_string("to")?;

        debug!("Converting {} {} to {}", value, from, to);

        // Get conversion factors
        let from_conv = get_conversion(&from).ok_or_else(|| ToolError::InvalidParameter {
            name: "from".to_string(),
            reason: format!("Unknown unit: {}. Supported units include: km, miles, kg, lb, celsius, fahrenheit, liters, gallons, etc.", from),
        })?;

        let to_conv = get_conversion(&to).ok_or_else(|| ToolError::InvalidParameter {
            name: "to".to_string(),
            reason: format!("Unknown unit: {}. Supported units include: km, miles, kg, lb, celsius, fahrenheit, liters, gallons, etc.", to),
        })?;

        // Check same category
        if from_conv.category != to_conv.category {
            return Err(ToolError::InvalidParameter {
                name: "to".to_string(),
                reason: format!(
                    "Cannot convert {} ({}) to {} ({}). Units must be of the same type.",
                    from, from_conv.category, to, to_conv.category
                ),
            });
        }

        // Temperature is special
        let result = if from_conv.category == "temperature" {
            convert_temperature(value, &from, &to)
        } else {
            // Convert: value * from_to_base / to_to_base
            let base_value = value * from_conv.to_base;
            base_value / to_conv.to_base
        };

        let formatted = format_number(result);
        debug!("Result: {} {} = {} {}", value, from, formatted, to);

        Ok(ToolOutput::success(format!(
            "{} {} = {} {}",
            format_number(value), from, formatted, to
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::Value;

    fn make_args(value: f64, from: &str, to: &str) -> ToolArgs {
        let mut params = HashMap::new();
        params.insert("value".to_string(), Value::Number(serde_json::Number::from_f64(value).unwrap()));
        params.insert("from".to_string(), Value::String(from.to_string()));
        params.insert("to".to_string(), Value::String(to.to_string()));
        ToolArgs::new(params)
    }

    #[tokio::test]
    async fn test_length_conversion() {
        let converter = UnitConverter::new();

        // km to miles
        let result = converter.execute(make_args(100.0, "km", "miles")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("62.13"));

        // feet to meters
        let result = converter.execute(make_args(10.0, "ft", "m")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("3.048"));
    }

    #[tokio::test]
    async fn test_weight_conversion() {
        let converter = UnitConverter::new();

        // kg to pounds
        let result = converter.execute(make_args(1.0, "kg", "lb")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("2.2"));

        // ounces to grams
        let result = converter.execute(make_args(1.0, "oz", "g")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("28.3"));
    }

    #[tokio::test]
    async fn test_temperature_conversion() {
        let converter = UnitConverter::new();

        // Celsius to Fahrenheit
        let result = converter.execute(make_args(0.0, "celsius", "fahrenheit")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("32"));

        // Fahrenheit to Celsius
        let result = converter.execute(make_args(32.0, "fahrenheit", "celsius")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("0"));

        // Celsius to Kelvin
        let result = converter.execute(make_args(0.0, "c", "k")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("273.15"));
    }

    #[tokio::test]
    async fn test_volume_conversion() {
        let converter = UnitConverter::new();

        // liters to gallons
        let result = converter.execute(make_args(10.0, "liters", "gallons")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("2.64"));
    }

    #[tokio::test]
    async fn test_data_conversion() {
        let converter = UnitConverter::new();

        // GB to MB
        let result = converter.execute(make_args(1.0, "gb", "mb")).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("1024"));
    }

    #[tokio::test]
    async fn test_incompatible_units() {
        let converter = UnitConverter::new();

        let result = converter.execute(make_args(100.0, "km", "kg")).await;
        assert!(matches!(result, Err(ToolError::InvalidParameter { .. })));
    }

    #[tokio::test]
    async fn test_unknown_unit() {
        let converter = UnitConverter::new();

        let result = converter.execute(make_args(100.0, "foobar", "km")).await;
        assert!(matches!(result, Err(ToolError::InvalidParameter { .. })));
    }

    #[tokio::test]
    async fn test_case_insensitive() {
        let converter = UnitConverter::new();

        let result = converter.execute(make_args(100.0, "KM", "Miles")).await.unwrap();
        assert!(result.success);
    }
}
