//! # Environment Variable Utilities
//!
//! Provides helpers for reading environment variables with common type conversions.
//! Includes parsing for boolean flags and numeric values with fallback defaults.
//!
//! These functions are typically used in configuration loading (e.g. `AppConfig`).
//!
//! # Examples
//! ```rust,no_run
//! use wzs_web::config::env::{read_flag, read_u32};
//!
//! let debug = read_flag("DEBUG", false);
//! let port = read_u32("PORT", 8080);
//! ```

/// Reads a boolean flag from an environment variable.
///
/// Returns `true` for any of the following case-insensitive values:
/// `"1"`, `"true"`, `"yes"`, `"on"`.
///
/// # Example
/// ```rust,no_run
/// use wzs_web::config::env::{read_flag, read_flag_from};
///
/// assert!(read_flag_from(|_| Some("yes".into()), "DEBUG", false));
/// ```
pub fn read_flag(name: &str, default: bool) -> bool {
    read_flag_from(|k| std::env::var(k).ok(), name, default)
}

/// Reads a boolean flag using a custom provider function.
///
/// Useful for testing or mocking environment sources.
///
/// # Example
/// ```rust
/// use wzs_web::config::env::read_flag_from;
///
/// let val = read_flag_from(|_| Some("true".into()), "ENABLE_FEATURE", false);
/// assert!(val);
/// ```
pub fn read_flag_from<F>(provider: F, name: &str, default: bool) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    match provider(name) {
        Some(v) => {
            let s = v.trim().trim_matches(|c| c == '"' || c == '\'');
            matches!(s.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
        }
        None => default,
    }
}

/// Reads an unsigned integer (`u32`) from an environment variable,
/// returning the provided default if parsing fails.
///
/// # Example
/// ```rust,no_run
/// use wzs_web::config::env::read_u32;
///
/// let limit = read_u32("LIMIT", 100);
/// ```
pub fn read_u32(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_flag_true_variants() {
        for val in ["1", "true", "TRUE", "yes", "YES", "on", "On"] {
            let got = read_flag_from(|_| Some(val.into()), "X", false);
            assert!(got, "Expected {val:?} to be truthy");
        }
    }

    #[test]
    fn test_read_flag_false_variants() {
        for val in ["0", "false", "no", "off", "xyz", ""] {
            let got = read_flag_from(|_| Some(val.into()), "X", true);
            assert!(!got, "Expected {val:?} to be falsy");
        }
    }

    #[test]
    fn test_read_flag_default_when_missing() {
        assert!(read_flag_from(|_| None, "X", true));
        assert!(!read_flag_from(|_| None, "X", false));
    }

    #[test]
    fn test_read_flag_strips_quotes() {
        assert!(read_flag_from(|_| Some("\"true\"".into()), "X", false));
        assert!(read_flag_from(|_| Some("'yes'".into()), "X", false));
    }

    fn read_u32_from<F>(provider: F, name: &str, default: u32) -> u32
    where
        F: Fn(&str) -> Option<String>,
    {
        provider(name)
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(default)
    }

    #[test]
    fn test_read_u32_valid_number() {
        let got = read_u32_from(|_| Some("42".into()), "LIMIT", 10);
        assert_eq!(got, 42);
    }

    #[test]
    fn test_read_u32_invalid_or_missing() {
        let got = read_u32_from(|_| Some("not_a_number".into()), "LIMIT", 99);
        assert_eq!(got, 99);

        let got = read_u32_from(|_| None, "LIMIT", 77);
        assert_eq!(got, 77);
    }
}
