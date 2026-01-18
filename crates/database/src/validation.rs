//! Input validation for user profile fields.

use std::fmt;

/// Validation error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Invalid email format.
    InvalidEmail(String),
    /// Invalid Bolt 12 offer format.
    InvalidBolt12Offer(String),
    /// Value too long.
    TooLong { field: String, max: usize, actual: usize },
    /// Empty value where one is required.
    Empty(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::InvalidEmail(msg) => write!(f, "Invalid email: {}", msg),
            ValidationError::InvalidBolt12Offer(msg) => write!(f, "Invalid Bolt 12 offer: {}", msg),
            ValidationError::TooLong { field, max, actual } => {
                write!(f, "{} is too long ({} chars, max {})", field, actual, max)
            }
            ValidationError::Empty(field) => write!(f, "{} cannot be empty", field),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Maximum allowed length for email addresses.
pub const MAX_EMAIL_LENGTH: usize = 254;

/// Maximum allowed length for Bolt 12 offers.
pub const MAX_BOLT12_LENGTH: usize = 1024;

/// Maximum allowed length for model names.
pub const MAX_MODEL_LENGTH: usize = 64;

/// Validate an email address (basic RFC 5322 format check).
///
/// This is a basic validation that checks:
/// - Contains exactly one @
/// - Has at least one character before @
/// - Has at least one character after @
/// - Has at least one dot after @
/// - Is not too long
pub fn validate_email(email: &str) -> Result<(), ValidationError> {
    let email = email.trim();

    if email.is_empty() {
        return Err(ValidationError::Empty("email".to_string()));
    }

    if email.len() > MAX_EMAIL_LENGTH {
        return Err(ValidationError::TooLong {
            field: "email".to_string(),
            max: MAX_EMAIL_LENGTH,
            actual: email.len(),
        });
    }

    // Basic format check: local@domain.tld
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return Err(ValidationError::InvalidEmail(
            "must contain exactly one @ symbol".to_string(),
        ));
    }

    let (local, domain) = (parts[0], parts[1]);

    if local.is_empty() {
        return Err(ValidationError::InvalidEmail(
            "missing local part (before @)".to_string(),
        ));
    }

    if domain.is_empty() {
        return Err(ValidationError::InvalidEmail(
            "missing domain (after @)".to_string(),
        ));
    }

    if !domain.contains('.') {
        return Err(ValidationError::InvalidEmail(
            "domain must contain at least one dot".to_string(),
        ));
    }

    // Check for common invalid patterns
    if domain.starts_with('.') || domain.ends_with('.') {
        return Err(ValidationError::InvalidEmail(
            "domain cannot start or end with a dot".to_string(),
        ));
    }

    if domain.contains("..") {
        return Err(ValidationError::InvalidEmail(
            "domain cannot contain consecutive dots".to_string(),
        ));
    }

    Ok(())
}

/// Validate a Bolt 12 offer.
///
/// Bolt 12 offers must:
/// - Start with "lno1" (offer prefix per BOLT 12 spec)
/// - Be bech32m encoded (lowercase alphanumeric, no 1/b/i/o characters after prefix)
/// - Not be too long
pub fn validate_bolt12_offer(offer: &str) -> Result<(), ValidationError> {
    let offer = offer.trim().to_lowercase();

    if offer.is_empty() {
        return Err(ValidationError::Empty("Bolt 12 offer".to_string()));
    }

    if offer.len() > MAX_BOLT12_LENGTH {
        return Err(ValidationError::TooLong {
            field: "Bolt 12 offer".to_string(),
            max: MAX_BOLT12_LENGTH,
            actual: offer.len(),
        });
    }

    // Must start with lno1 (BOLT 12 offer prefix)
    if !offer.starts_with("lno1") {
        return Err(ValidationError::InvalidBolt12Offer(
            "must start with 'lno1'".to_string(),
        ));
    }

    // The rest should be bech32m characters (after the separator "1")
    // Valid bech32m charset: qpzry9x8gf2tvdw0s3jn54khce6mua7l
    let bech32_chars: &str = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    let data_part = &offer[4..]; // Skip "lno1"

    for c in data_part.chars() {
        if !bech32_chars.contains(c) {
            return Err(ValidationError::InvalidBolt12Offer(format!(
                "invalid character '{}' (must be bech32m encoded)",
                c
            )));
        }
    }

    // Minimum length check (prefix + some data)
    if data_part.len() < 10 {
        return Err(ValidationError::InvalidBolt12Offer(
            "offer is too short".to_string(),
        ));
    }

    Ok(())
}

/// Validate a model name length.
pub fn validate_model_length(model: &str) -> Result<(), ValidationError> {
    let model = model.trim();

    if model.is_empty() {
        return Err(ValidationError::Empty("model".to_string()));
    }

    if model.len() > MAX_MODEL_LENGTH {
        return Err(ValidationError::TooLong {
            field: "model".to_string(),
            max: MAX_MODEL_LENGTH,
            actual: model.len(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_email_valid() {
        assert!(validate_email("test@example.com").is_ok());
        assert!(validate_email("user.name@domain.co.uk").is_ok());
        assert!(validate_email("a@b.c").is_ok());
        assert!(validate_email(" test@example.com ").is_ok()); // trimmed
    }

    #[test]
    fn test_validate_email_invalid() {
        // Empty
        assert!(matches!(
            validate_email(""),
            Err(ValidationError::Empty(_))
        ));

        // No @
        assert!(matches!(
            validate_email("test.example.com"),
            Err(ValidationError::InvalidEmail(_))
        ));

        // Multiple @
        assert!(matches!(
            validate_email("test@example@com"),
            Err(ValidationError::InvalidEmail(_))
        ));

        // Missing local part
        assert!(matches!(
            validate_email("@example.com"),
            Err(ValidationError::InvalidEmail(_))
        ));

        // Missing domain
        assert!(matches!(
            validate_email("test@"),
            Err(ValidationError::InvalidEmail(_))
        ));

        // No dot in domain
        assert!(matches!(
            validate_email("test@localhost"),
            Err(ValidationError::InvalidEmail(_))
        ));

        // Dot at start of domain
        assert!(matches!(
            validate_email("test@.example.com"),
            Err(ValidationError::InvalidEmail(_))
        ));

        // Dot at end of domain
        assert!(matches!(
            validate_email("test@example.com."),
            Err(ValidationError::InvalidEmail(_))
        ));

        // Consecutive dots
        assert!(matches!(
            validate_email("test@example..com"),
            Err(ValidationError::InvalidEmail(_))
        ));
    }

    #[test]
    fn test_validate_email_too_long() {
        // MAX_EMAIL_LENGTH is 254, so we need an email > 254 chars
        let long_local = "a".repeat(250);
        let email = format!("{}@example.com", long_local);
        assert!(email.len() > 254);
        assert!(matches!(
            validate_email(&email),
            Err(ValidationError::TooLong { .. })
        ));
    }

    #[test]
    fn test_validate_bolt12_valid() {
        // Example Bolt 12 offer (simplified for testing)
        assert!(validate_bolt12_offer("lno1qcp4256ypqpq8q2qqqqqq").is_ok());
        assert!(validate_bolt12_offer("LNO1QCP4256YPQPQ8Q2QQQQQQ").is_ok()); // case insensitive
    }

    #[test]
    fn test_validate_bolt12_invalid() {
        // Empty
        assert!(matches!(
            validate_bolt12_offer(""),
            Err(ValidationError::Empty(_))
        ));

        // Wrong prefix
        assert!(matches!(
            validate_bolt12_offer("lnbc1abc123"),
            Err(ValidationError::InvalidBolt12Offer(_))
        ));

        // Invalid characters
        assert!(matches!(
            validate_bolt12_offer("lno1abc!@#"),
            Err(ValidationError::InvalidBolt12Offer(_))
        ));

        // Too short
        assert!(matches!(
            validate_bolt12_offer("lno1abc"),
            Err(ValidationError::InvalidBolt12Offer(_))
        ));
    }

    #[test]
    fn test_validate_model_length() {
        assert!(validate_model_length("llama").is_ok());
        assert!(validate_model_length("grok-4-1-fast").is_ok());

        // Empty
        assert!(matches!(
            validate_model_length(""),
            Err(ValidationError::Empty(_))
        ));

        // Too long
        let long_model = "a".repeat(100);
        assert!(matches!(
            validate_model_length(&long_model),
            Err(ValidationError::TooLong { .. })
        ));
    }

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError::InvalidEmail("test message".to_string());
        assert_eq!(err.to_string(), "Invalid email: test message");

        let err = ValidationError::TooLong {
            field: "email".to_string(),
            max: 254,
            actual: 300,
        };
        assert_eq!(err.to_string(), "email is too long (300 chars, max 254)");
    }
}
