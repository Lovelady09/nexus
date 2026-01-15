//! Duration string validation
//!
//! Validates the duration field for ban and trust operations.
//! This performs length-only validation; semantic validation (parsing the
//! duration format like "10m", "4h", "7d", "0") is handled by server handlers.

/// Maximum length for a duration string in bytes.
///
/// Duration format examples:
/// - "0" (permanent, 1 char)
/// - "10m" (10 minutes, 3 chars)
/// - "4h" (4 hours, 2 chars)
/// - "7d" (7 days, 2 chars)
/// - "30d" (30 days, 3 chars)
/// - "365d" (365 days, 4 chars)
///
/// Using 10 as a reasonable upper bound that covers all realistic cases.
pub const MAX_DURATION_LENGTH: usize = 10;

/// Validation error for duration string
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurationError {
    /// Duration exceeds maximum length
    TooLong,
}

/// Validate a duration string (length-only)
///
/// This only validates the length. Semantic validation (whether it's a valid
/// duration format) is performed by server handlers via `parse_duration`.
///
/// Note: Empty/None durations are valid (meaning permanent), so we don't
/// reject empty strings here.
///
/// # Arguments
/// * `duration` - The duration string to validate
///
/// # Returns
/// * `Ok(())` if the duration length is valid
/// * `Err(DurationError)` if validation fails
pub fn validate_duration(duration: &str) -> Result<(), DurationError> {
    if duration.len() > MAX_DURATION_LENGTH {
        return Err(DurationError::TooLong);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_durations() {
        // Permanent
        assert!(validate_duration("0").is_ok());
        // Minutes
        assert!(validate_duration("10m").is_ok());
        // Hours
        assert!(validate_duration("4h").is_ok());
        // Days
        assert!(validate_duration("7d").is_ok());
        assert!(validate_duration("30d").is_ok());
        assert!(validate_duration("365d").is_ok());
        // Empty (permanent)
        assert!(validate_duration("").is_ok());
        // At max length
        assert!(validate_duration(&"a".repeat(MAX_DURATION_LENGTH)).is_ok());
    }

    #[test]
    fn test_duration_too_long() {
        assert_eq!(
            validate_duration(&"a".repeat(MAX_DURATION_LENGTH + 1)),
            Err(DurationError::TooLong)
        );
    }
}
