//! # Environment Configuration
//!
//! Provides environment variable storage and common type conversion helpers.
//!
//! [`EnvConfig`] captures environment variables into an in-memory map so that
//! configuration objects can read from a single, stable source instead of
//! accessing `std::env` directly.
//!
//! This is useful for:
//!
//! - Reading environment variables only once at application startup.
//! - Preserving application-specific variables unknown to `wzs-web`.
//! - Building multiple configuration sets from the same environment snapshot.
//! - Creating prefixed configuration views for multiple API entry points.
//! - Testing configuration without modifying process environment variables.
//!
//! # Examples
//!
//! ## Capture the process environment
//!
//! ```rust,no_run
//! use wzs_web::config::env::EnvConfig;
//!
//! let env = EnvConfig::from_env();
//!
//! let bcrypt_cost = env.get_u32("BCRYPT_COST").unwrap_or(12);
//! let base_url = env.get("PUBLIC_WEB_BASE_URL");
//! ```
//!
//! ## Create an environment for testing
//!
//! ```rust
//! use wzs_web::config::env::EnvConfig;
//!
//! let env = EnvConfig::from_pairs([
//!     ("PUBLIC_CORS_ENABLED", "true"),
//!     ("BCRYPT_COST", "4"),
//! ]);
//!
//! assert_eq!(env.get_bool("PUBLIC_CORS_ENABLED"), Some(true));
//! assert_eq!(env.get_u32("BCRYPT_COST"), Some(4));
//! ```
//!
//! ## Create a prefixed configuration view
//!
//! ```rust
//! use wzs_web::config::env::EnvConfig;
//!
//! let env = EnvConfig::from_pairs([
//!     ("PUBLIC_CORS_ENABLED", "true"),
//!     ("PUBLIC_JWT_SECRET", "public-secret"),
//!     ("ADMIN_JWT_SECRET", "admin-secret"),
//! ]);
//!
//! let public_env = env.with_prefix("PUBLIC_");
//!
//! assert_eq!(public_env.get("CORS_ENABLED"), Some("true"));
//! assert_eq!(public_env.get("JWT_SECRET"), Some("public-secret"));
//! assert_eq!(public_env.get("ADMIN_JWT_SECRET"), None);
//! ```
//!
//! # Security
//!
//! [`EnvConfig`] intentionally does not expose stored values through its
//! [`std::fmt::Debug`] implementation. Environment variables may contain
//! passwords, JWT secrets, database credentials, API keys, and other sensitive
//! information.
//!
//! The `Debug` representation therefore contains only environment variable
//! names, never their values.

use std::collections::HashMap;
use std::fmt;

/// In-memory snapshot of environment variables.
///
/// `EnvConfig` stores all supplied environment variables, including variables
/// that are not known by `wzs-web`.
///
/// Higher-level configuration objects can therefore use `EnvConfig` as their
/// configuration source without directly accessing the process environment.
///
/// # Examples
///
/// ```rust
/// use wzs_web::config::env::EnvConfig;
///
/// let env = EnvConfig::from_pairs([
///     ("BCRYPT_COST", "4"),
///     ("FEATURE_ENABLED", "true"),
/// ]);
///
/// assert_eq!(env.get("BCRYPT_COST"), Some("4"));
/// assert_eq!(env.get_u32("BCRYPT_COST"), Some(4));
/// assert_eq!(env.get_bool("FEATURE_ENABLED"), Some(true));
/// ```
#[derive(Clone, Default)]
pub struct EnvConfig {
    values: HashMap<String, String>,
}

impl EnvConfig {
    /// Captures all current process environment variables.
    ///
    /// The environment is read only when this function is called. Subsequent
    /// changes to the process environment are not reflected in this
    /// `EnvConfig`.
    ///
    /// This makes the configuration effectively a snapshot of the environment
    /// at application startup.
    pub fn from_env() -> Self {
        Self::from_pairs(std::env::vars())
    }

    /// Creates an environment snapshot from key-value pairs.
    ///
    /// If the same key appears more than once, the last value wins.
    pub fn from_pairs<I, K, V>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self {
            values: pairs
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }

    /// Creates a new environment snapshot containing only variables with
    /// `prefix`, with that prefix removed from their keys.
    ///
    /// This is useful when multiple independently configured services or API
    /// entry points share the same process environment.
    ///
    /// For example:
    ///
    /// ```text
    /// PUBLIC_JWT_SECRET=public-secret
    /// PUBLIC_CORS_ENABLED=true
    /// ADMIN_JWT_SECRET=admin-secret
    /// ```
    ///
    /// Calling `with_prefix("PUBLIC_")` produces an environment equivalent to:
    ///
    /// ```text
    /// JWT_SECRET=public-secret
    /// CORS_ENABLED=true
    /// ```
    ///
    /// Variables that do not start with `prefix` are not included.
    ///
    /// The original `EnvConfig` is not modified.
    ///
    /// # Example
    ///
    /// ```rust
    /// use wzs_web::config::env::EnvConfig;
    ///
    /// let env = EnvConfig::from_pairs([
    ///     ("PUBLIC_JWT_SECRET", "public-secret"),
    ///     ("PUBLIC_CORS_ENABLED", "true"),
    ///     ("ADMIN_JWT_SECRET", "admin-secret"),
    /// ]);
    ///
    /// let public_env = env.with_prefix("PUBLIC_");
    ///
    /// assert_eq!(
    ///     public_env.get("JWT_SECRET"),
    ///     Some("public-secret")
    /// );
    /// assert_eq!(
    ///     public_env.get_bool("CORS_ENABLED"),
    ///     Some(true)
    /// );
    /// assert_eq!(
    ///     public_env.get("ADMIN_JWT_SECRET"),
    ///     None
    /// );
    /// ```
    pub fn with_prefix(&self, prefix: &str) -> Self {
        Self::from_pairs(self.values.iter().filter_map(|(key, value)| {
            key.strip_prefix(prefix)
                .map(|key| (key.to_string(), value.clone()))
        }))
    }

    /// Returns the raw value associated with `key`.
    ///
    /// No trimming, parsing, or other transformation is performed.
    ///
    /// Returns `None` when the key does not exist.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Returns an owned copy of the value associated with `key`.
    ///
    /// Returns `None` when the key does not exist.
    ///
    /// Prefer [`EnvConfig::get`] when ownership is not required.
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }

    /// Parses a boolean value associated with `key`.
    ///
    /// The following values are recognized case-insensitively:
    ///
    /// Truthy:
    ///
    /// - `"1"`
    /// - `"true"`
    /// - `"yes"`
    /// - `"on"`
    ///
    /// Falsy:
    ///
    /// - `"0"`
    /// - `"false"`
    /// - `"no"`
    /// - `"off"`
    ///
    /// Leading and trailing whitespace is ignored. A single pair of surrounding
    /// single or double quotes is also ignored for compatibility with existing
    /// configuration behavior.
    ///
    /// Returns `None` when the key does not exist or the value is invalid.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(parse_bool)
    }

    /// Parses the value associated with `key` as a `u16`.
    pub fn get_u16(&self, key: &str) -> Option<u16> {
        self.parse(key)
    }

    /// Parses the value associated with `key` as a `u32`.
    pub fn get_u32(&self, key: &str) -> Option<u32> {
        self.parse(key)
    }

    /// Parses the value associated with `key` as a `u64`.
    pub fn get_u64(&self, key: &str) -> Option<u64> {
        self.parse(key)
    }

    /// Parses the value associated with `key` as a `usize`.
    pub fn get_usize(&self, key: &str) -> Option<usize> {
        self.parse(key)
    }

    /// Returns `true` when `key` exists in this environment snapshot.
    ///
    /// An empty value still counts as present.
    pub fn contains_key(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// Returns the number of environment variables stored in this snapshot.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` when no environment variables are stored.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Parses an environment value using its [`std::str::FromStr`]
    /// implementation.
    fn parse<T>(&self, key: &str) -> Option<T>
    where
        T: std::str::FromStr,
    {
        self.get(key)?.trim().parse::<T>().ok()
    }
}

/// Provides a safe debug representation.
///
/// Environment variable values are intentionally omitted because they may
/// contain passwords, tokens, JWT secrets, database credentials, or other
/// sensitive information.
///
/// Only variable names are displayed.
impl fmt::Debug for EnvConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut keys: Vec<&str> = self.values.keys().map(String::as_str).collect();

        keys.sort_unstable();

        f.debug_struct("EnvConfig").field("keys", &keys).finish()
    }
}

/// Parses a string as a boolean value.
fn parse_bool(value: &str) -> Option<bool> {
    let value = value
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .to_ascii_lowercase();

    match value.as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

// -----------------------------------------------------------------------------
// Legacy environment helpers
// -----------------------------------------------------------------------------
//
// These functions are retained for backward compatibility.
//
// New configuration code should prefer `EnvConfig` so that environment
// variables can be captured once and injected into configuration loaders.

/// Reads a boolean flag from an environment variable.
pub fn read_flag(name: &str, default: bool) -> bool {
    read_flag_from(|key| std::env::var(key).ok(), name, default)
}

/// Reads a boolean flag using a custom provider function.
pub fn read_flag_from<F>(provider: F, name: &str, default: bool) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    match provider(name) {
        Some(value) => parse_bool(&value).unwrap_or(false),
        None => default,
    }
}

/// Reads an unsigned integer (`u32`) from an environment variable.
pub fn read_u32(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // EnvConfig construction
    // -------------------------------------------------------------------------

    #[test]
    fn env_config_from_iter_stores_values() {
        let env = EnvConfig::from_pairs([
            ("BCRYPT_COST", "4"),
            ("PUBLIC_WEB_BASE_URL", "https://example.com"),
        ]);

        assert_eq!(env.get("BCRYPT_COST"), Some("4"));

        assert_eq!(env.get("PUBLIC_WEB_BASE_URL"), Some("https://example.com"));
    }

    #[test]
    fn env_config_from_iter_last_duplicate_value_wins() {
        let env = EnvConfig::from_pairs([("VALUE", "first"), ("VALUE", "second")]);

        assert_eq!(env.get("VALUE"), Some("second"));
    }

    #[test]
    fn env_config_default_is_empty() {
        let env = EnvConfig::default();

        assert!(env.is_empty());
        assert_eq!(env.len(), 0);
    }

    // -------------------------------------------------------------------------
    // Prefix views
    // -------------------------------------------------------------------------

    #[test]
    fn with_prefix_selects_matching_values() {
        let env = EnvConfig::from_pairs([
            ("PUBLIC_JWT_SECRET", "public-secret"),
            ("PUBLIC_CORS_ENABLED", "true"),
            ("ADMIN_JWT_SECRET", "admin-secret"),
            ("BCRYPT_COST", "12"),
        ]);

        let public_env = env.with_prefix("PUBLIC_");

        assert_eq!(public_env.get("JWT_SECRET"), Some("public-secret"));

        assert_eq!(public_env.get("CORS_ENABLED"), Some("true"));

        assert_eq!(public_env.len(), 2);
    }

    #[test]
    fn with_prefix_removes_prefix_from_keys() {
        let env = EnvConfig::from_pairs([("PUBLIC_CORS_ORIGINS", "https://example.com")]);

        let public_env = env.with_prefix("PUBLIC_");

        assert_eq!(public_env.get("CORS_ORIGINS"), Some("https://example.com"));

        assert_eq!(public_env.get("PUBLIC_CORS_ORIGINS"), None);
    }

    #[test]
    fn with_prefix_excludes_other_prefixes_and_unprefixed_values() {
        let env = EnvConfig::from_pairs([
            ("PUBLIC_JWT_SECRET", "public-secret"),
            ("ADMIN_JWT_SECRET", "admin-secret"),
            ("JWT_SECRET", "legacy-secret"),
            ("BCRYPT_COST", "12"),
        ]);

        let public_env = env.with_prefix("PUBLIC_");

        assert_eq!(public_env.get("JWT_SECRET"), Some("public-secret"));

        assert_eq!(public_env.get("ADMIN_JWT_SECRET"), None);

        assert_eq!(public_env.get("BCRYPT_COST"), None);
        assert_eq!(public_env.len(), 1);
    }

    #[test]
    fn with_prefix_returns_empty_config_when_no_values_match() {
        let env =
            EnvConfig::from_pairs([("ADMIN_JWT_SECRET", "admin-secret"), ("BCRYPT_COST", "12")]);

        let public_env = env.with_prefix("PUBLIC_");

        assert!(public_env.is_empty());
    }

    #[test]
    fn with_prefix_does_not_modify_original_config() {
        let env = EnvConfig::from_pairs([
            ("PUBLIC_JWT_SECRET", "public-secret"),
            ("ADMIN_JWT_SECRET", "admin-secret"),
        ]);

        let public_env = env.with_prefix("PUBLIC_");

        assert_eq!(env.get("PUBLIC_JWT_SECRET"), Some("public-secret"));

        assert_eq!(env.get("ADMIN_JWT_SECRET"), Some("admin-secret"));

        assert_eq!(public_env.get("JWT_SECRET"), Some("public-secret"));
    }

    #[test]
    fn with_prefix_preserves_empty_values() {
        let env = EnvConfig::from_pairs([("PUBLIC_JWT_SECRET", "")]);

        let public_env = env.with_prefix("PUBLIC_");

        assert!(public_env.contains_key("JWT_SECRET"));
        assert_eq!(public_env.get("JWT_SECRET"), Some(""));
    }

    #[test]
    fn with_prefix_can_create_independent_api_configs() {
        let env = EnvConfig::from_pairs([
            ("PUBLIC_JWT_SECRET", "public-secret"),
            ("PUBLIC_CORS_ENABLED", "true"),
            ("ADMIN_JWT_SECRET", "admin-secret"),
            ("ADMIN_CORS_ENABLED", "false"),
        ]);

        let public_env = env.with_prefix("PUBLIC_");
        let admin_env = env.with_prefix("ADMIN_");

        assert_eq!(public_env.get("JWT_SECRET"), Some("public-secret"));

        assert_eq!(public_env.get_bool("CORS_ENABLED"), Some(true));

        assert_eq!(admin_env.get("JWT_SECRET"), Some("admin-secret"));

        assert_eq!(admin_env.get_bool("CORS_ENABLED"), Some(false));
    }

    // -------------------------------------------------------------------------
    // Raw access
    // -------------------------------------------------------------------------

    #[test]
    fn get_returns_existing_value() {
        let env = EnvConfig::from_pairs([("SCHOOL_YEAR", "2026")]);

        assert_eq!(env.get("SCHOOL_YEAR"), Some("2026"));
    }

    #[test]
    fn get_returns_none_for_missing_value() {
        let env = EnvConfig::default();

        assert_eq!(env.get("UNKNOWN"), None);
    }

    #[test]
    fn get_preserves_empty_value() {
        let env = EnvConfig::from_pairs([("EMPTY", "")]);

        assert_eq!(env.get("EMPTY"), Some(""));
    }

    #[test]
    fn get_string_returns_owned_value() {
        let env = EnvConfig::from_pairs([("VALUE", "hello")]);

        let value = env.get_string("VALUE");

        assert_eq!(value, Some("hello".to_string()));
    }

    #[test]
    fn contains_key_distinguishes_missing_from_empty() {
        let env = EnvConfig::from_pairs([("EMPTY", "")]);

        assert!(env.contains_key("EMPTY"));
        assert!(!env.contains_key("UNKNOWN"));
    }

    #[test]
    fn len_returns_number_of_values() {
        let env = EnvConfig::from_pairs([("A", "1"), ("B", "2"), ("C", "3")]);

        assert_eq!(env.len(), 3);
        assert!(!env.is_empty());
    }

    // -------------------------------------------------------------------------
    // Boolean parsing
    // -------------------------------------------------------------------------

    #[test]
    fn get_bool_parses_true_variants() {
        for value in ["1", "true", "TRUE", "True", "yes", "YES", "on", "ON"] {
            let env = EnvConfig::from_pairs([("VALUE", value)]);

            assert_eq!(
                env.get_bool("VALUE"),
                Some(true),
                "Expected {value:?} to be true"
            );
        }
    }

    #[test]
    fn get_bool_parses_false_variants() {
        for value in ["0", "false", "FALSE", "False", "no", "NO", "off", "OFF"] {
            let env = EnvConfig::from_pairs([("VALUE", value)]);

            assert_eq!(
                env.get_bool("VALUE"),
                Some(false),
                "Expected {value:?} to be false"
            );
        }
    }

    #[test]
    fn get_bool_ignores_surrounding_whitespace() {
        let env = EnvConfig::from_pairs([("A", " true "), ("B", "\tfalse\n")]);

        assert_eq!(env.get_bool("A"), Some(true));
        assert_eq!(env.get_bool("B"), Some(false));
    }

    #[test]
    fn get_bool_supports_quoted_values() {
        let env = EnvConfig::from_pairs([
            ("A", "\"true\""),
            ("B", "'yes'"),
            ("C", "\"false\""),
            ("D", "'off'"),
        ]);

        assert_eq!(env.get_bool("A"), Some(true));
        assert_eq!(env.get_bool("B"), Some(true));
        assert_eq!(env.get_bool("C"), Some(false));
        assert_eq!(env.get_bool("D"), Some(false));
    }

    #[test]
    fn get_bool_returns_none_for_invalid_value() {
        let env = EnvConfig::from_pairs([("VALUE", "not-a-bool")]);

        assert_eq!(env.get_bool("VALUE"), None);
    }

    #[test]
    fn get_bool_returns_none_for_empty_value() {
        let env = EnvConfig::from_pairs([("VALUE", "")]);

        assert_eq!(env.get_bool("VALUE"), None);
    }

    #[test]
    fn get_bool_returns_none_for_missing_value() {
        let env = EnvConfig::default();

        assert_eq!(env.get_bool("UNKNOWN"), None);
    }

    // -------------------------------------------------------------------------
    // Numeric parsing
    // -------------------------------------------------------------------------

    #[test]
    fn get_u16_parses_valid_value() {
        let env = EnvConfig::from_pairs([("PORT", "3000")]);

        assert_eq!(env.get_u16("PORT"), Some(3000));
    }

    #[test]
    fn get_u32_parses_valid_value() {
        let env = EnvConfig::from_pairs([("BCRYPT_COST", "12")]);

        assert_eq!(env.get_u32("BCRYPT_COST"), Some(12));
    }

    #[test]
    fn get_u64_parses_valid_value() {
        let env = EnvConfig::from_pairs([("MAX_VALUE", "123456789")]);

        assert_eq!(env.get_u64("MAX_VALUE"), Some(123_456_789));
    }

    #[test]
    fn get_usize_parses_valid_value() {
        let env = EnvConfig::from_pairs([("MAX_BODY_BYTES", "5242880")]);

        assert_eq!(env.get_usize("MAX_BODY_BYTES"), Some(5_242_880));
    }

    #[test]
    fn numeric_getters_ignore_surrounding_whitespace() {
        let env = EnvConfig::from_pairs([("VALUE", " 42 ")]);

        assert_eq!(env.get_u32("VALUE"), Some(42));
    }

    #[test]
    fn numeric_getters_return_none_for_invalid_value() {
        let env = EnvConfig::from_pairs([("VALUE", "not-a-number")]);

        assert_eq!(env.get_u16("VALUE"), None);
        assert_eq!(env.get_u32("VALUE"), None);
        assert_eq!(env.get_u64("VALUE"), None);
        assert_eq!(env.get_usize("VALUE"), None);
    }

    #[test]
    fn numeric_getters_return_none_for_negative_value() {
        let env = EnvConfig::from_pairs([("VALUE", "-1")]);

        assert_eq!(env.get_u16("VALUE"), None);
        assert_eq!(env.get_u32("VALUE"), None);
        assert_eq!(env.get_u64("VALUE"), None);
        assert_eq!(env.get_usize("VALUE"), None);
    }

    #[test]
    fn numeric_getters_return_none_for_missing_value() {
        let env = EnvConfig::default();

        assert_eq!(env.get_u16("UNKNOWN"), None);
        assert_eq!(env.get_u32("UNKNOWN"), None);
        assert_eq!(env.get_u64("UNKNOWN"), None);
        assert_eq!(env.get_usize("UNKNOWN"), None);
    }

    // -------------------------------------------------------------------------
    // Debug security
    // -------------------------------------------------------------------------

    #[test]
    fn debug_does_not_expose_values() {
        let env = EnvConfig::from_pairs([
            ("JWT_SECRET", "super-secret-jwt-value"),
            ("SMTP_PASSWORD", "super-secret-password"),
        ]);

        let debug = format!("{env:?}");

        assert!(debug.contains("JWT_SECRET"));
        assert!(debug.contains("SMTP_PASSWORD"));

        assert!(!debug.contains("super-secret-jwt-value"));

        assert!(!debug.contains("super-secret-password"));
    }

    #[test]
    fn debug_output_is_deterministic() {
        let env = EnvConfig::from_pairs([("Z_VALUE", "z"), ("A_VALUE", "a"), ("M_VALUE", "m")]);

        let debug = format!("{env:?}");

        let a = debug.find("A_VALUE").unwrap();
        let m = debug.find("M_VALUE").unwrap();
        let z = debug.find("Z_VALUE").unwrap();

        assert!(a < m);
        assert!(m < z);
    }

    // -------------------------------------------------------------------------
    // Legacy read_flag
    // -------------------------------------------------------------------------

    #[test]
    fn test_read_flag_true_variants() {
        for value in ["1", "true", "TRUE", "yes", "YES", "on", "On"] {
            let got = read_flag_from(|_| Some(value.into()), "X", false);

            assert!(got, "Expected {value:?} to be truthy");
        }
    }

    #[test]
    fn test_read_flag_false_variants() {
        for value in ["0", "false", "no", "off", "xyz", ""] {
            let got = read_flag_from(|_| Some(value.into()), "X", true);

            assert!(!got, "Expected {value:?} to be falsy");
        }
    }

    #[test]
    fn test_read_flag_default_when_missing() {
        assert!(read_flag_from(|_| None, "X", true));
        assert!(!read_flag_from(|_| None, "X", false));
    }

    #[test]
    fn test_read_flag_strips_quotes() {
        assert!(read_flag_from(|_| Some("\"true\"".into()), "X", false,));

        assert!(read_flag_from(|_| Some("'yes'".into()), "X", false,));
    }

    // -------------------------------------------------------------------------
    // Legacy read_u32
    // -------------------------------------------------------------------------

    fn read_u32_from<F>(provider: F, name: &str, default: u32) -> u32
    where
        F: Fn(&str) -> Option<String>,
    {
        provider(name)
            .and_then(|value| value.trim().parse::<u32>().ok())
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
