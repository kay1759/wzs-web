//! # HTTP and CORS Configuration
//!
//! Defines configuration structures for HTTP request handling and
//! CORS (Cross-Origin Resource Sharing) behavior.
//!
//! Configuration can be built from an [`EnvConfig`] environment snapshot.
//! These structures are typically included within
//! [`AppConfig`](crate::config::app::AppConfig) or other service-specific
//! configuration layers.
//!
//! # Environment Variables
//!
//! ## HTTP
//!
//! - `HTTP_MAX_BODY_BYTES` — maximum request body size in bytes
//! - `HTTP_MAX_BODY_MB` — maximum request body size in megabytes when
//!   `HTTP_MAX_BODY_BYTES` is missing or invalid
//!
//! The default maximum body size is 5 MiB.
//!
//! ## CORS
//!
//! - `CORS_ENABLED` — enables CORS (default: `false`)
//! - `CORS_ORIGINS` — allowed origins (default: empty string)
//! - `CORS_CREDENTIALS` — allows credentials (default: `false`)
//!
//! # Examples
//!
//! ```rust
//! use wzs_web::config::env::EnvConfig;
//! use wzs_web::config::web::{CorsConfig, HttpConfig};
//!
//! let env = EnvConfig::from_iter([
//!     ("HTTP_MAX_BODY_MB", "10"),
//!     ("CORS_ENABLED", "true"),
//!     ("CORS_ORIGINS", "http://localhost:5173"),
//!     ("CORS_CREDENTIALS", "true"),
//! ]);
//!
//! let http = HttpConfig::from_env_config(&env);
//! let cors = CorsConfig::from_env_config(&env);
//!
//! assert_eq!(http.max_body_bytes, 10 * 1024 * 1024);
//! assert!(cors.enabled);
//! assert_eq!(cors.env, "http://localhost:5173");
//! assert!(cors.credentials);
//! ```

use crate::config::env::EnvConfig;

/// Default maximum HTTP request body size in megabytes.
const DEFAULT_MAX_BODY_MB: usize = 5;

/// Number of bytes in one mebibyte.
const BYTES_PER_MB: usize = 1024 * 1024;

/// HTTP-related configuration.
///
/// Controls HTTP-layer settings such as the maximum request body size.
///
/// # Environment Variables
///
/// - `HTTP_MAX_BODY_BYTES` — maximum body size in bytes
/// - `HTTP_MAX_BODY_MB` — fallback maximum body size in megabytes
///
/// `HTTP_MAX_BODY_BYTES` takes precedence when it contains a valid `usize`.
/// Otherwise `HTTP_MAX_BODY_MB` is used.
///
/// If neither contains a valid value, the default is 5 MiB.
#[derive(Clone, Debug, PartialEq)]
pub struct HttpConfig {
    /// Maximum allowed HTTP request body size in bytes.
    pub max_body_bytes: usize,
}

impl HttpConfig {
    /// Builds an [`HttpConfig`] from an [`EnvConfig`] snapshot.
    ///
    /// `HTTP_MAX_BODY_BYTES` takes precedence over `HTTP_MAX_BODY_MB`.
    ///
    /// If `HTTP_MAX_BODY_BYTES` is missing or invalid,
    /// `HTTP_MAX_BODY_MB` is used.
    ///
    /// If `HTTP_MAX_BODY_MB` is also missing or invalid, the maximum body
    /// size defaults to 5 MiB.
    ///
    /// # Example
    ///
    /// ```rust
    /// use wzs_web::config::env::EnvConfig;
    /// use wzs_web::config::web::HttpConfig;
    ///
    /// let env = EnvConfig::from_iter([
    ///     ("HTTP_MAX_BODY_MB", "10"),
    /// ]);
    ///
    /// let cfg = HttpConfig::from_env_config(&env);
    ///
    /// assert_eq!(cfg.max_body_bytes, 10 * 1024 * 1024);
    /// ```
    pub fn from_env_config(env: &EnvConfig) -> Self {
        let max_body_bytes = env.get_usize("HTTP_MAX_BODY_BYTES").unwrap_or_else(|| {
            env.get_usize("HTTP_MAX_BODY_MB")
                .unwrap_or(DEFAULT_MAX_BODY_MB)
                * BYTES_PER_MB
        });

        Self { max_body_bytes }
    }
}

/// CORS (Cross-Origin Resource Sharing) configuration.
///
/// Defines whether CORS is enabled, the configured allowed origins, and
/// whether credentialed requests are allowed.
///
/// # Environment Variables
///
/// - `CORS_ENABLED` — enables CORS
/// - `CORS_ORIGINS` — allowed origins
/// - `CORS_CREDENTIALS` — allows credentials
///
/// Both boolean values default to `false`.
#[derive(Clone, Debug, PartialEq)]
pub struct CorsConfig {
    /// Whether CORS support is enabled.
    pub enabled: bool,

    /// Configured allowed origins.
    ///
    /// The value is retained as a string because parsing and interpretation
    /// are performed by the CORS layer.
    pub env: String,

    /// Whether credentialed CORS requests are allowed.
    pub credentials: bool,
}

impl CorsConfig {
    /// Builds a [`CorsConfig`] from an [`EnvConfig`] snapshot.
    ///
    /// Missing values use the following defaults:
    ///
    /// - `CORS_ENABLED`: `false`
    /// - `CORS_ORIGINS`: `""`
    /// - `CORS_CREDENTIALS`: `false`
    ///
    /// For compatibility with the existing configuration behavior, an
    /// explicitly supplied boolean value is `true` only when it is one of:
    ///
    /// - `1`
    /// - `true`
    /// - `yes`
    /// - `on`
    ///
    /// Other supplied values are treated as `false`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use wzs_web::config::env::EnvConfig;
    /// use wzs_web::config::web::CorsConfig;
    ///
    /// let env = EnvConfig::from_iter([
    ///     ("CORS_ENABLED", "true"),
    ///     ("CORS_ORIGINS", "http://localhost:5173"),
    ///     ("CORS_CREDENTIALS", "true"),
    /// ]);
    ///
    /// let cfg = CorsConfig::from_env_config(&env);
    ///
    /// assert!(cfg.enabled);
    /// assert_eq!(cfg.env, "http://localhost:5173");
    /// assert!(cfg.credentials);
    /// ```
    pub fn from_env_config(env: &EnvConfig) -> Self {
        Self {
            enabled: read_flag(env, "CORS_ENABLED", false),
            env: env.get_string("CORS_ORIGINS").unwrap_or_default(),
            credentials: read_flag(env, "CORS_CREDENTIALS", false),
        }
    }
}

/// Reads a boolean flag while preserving the existing configuration behavior.
///
/// Missing values return `default`.
///
/// When a value is present, only recognized truthy values return `true`.
/// All other values return `false`.
fn read_flag(env: &EnvConfig, key: &str, default: bool) -> bool {
    match env.get(key) {
        Some(value) => is_truthy(value),
        None => default,
    }
}

/// Returns whether a string represents a truthy configuration value.
///
/// Matching is case-insensitive and surrounding whitespace is ignored.
fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_config_holds_value() {
        let cfg = HttpConfig {
            max_body_bytes: 10 * 1024 * 1024,
        };

        assert_eq!(cfg.max_body_bytes, 10 * 1024 * 1024);
    }

    #[test]
    fn http_config_uses_default_body_size() {
        let env = EnvConfig::default();

        let cfg = HttpConfig::from_env_config(&env);

        assert_eq!(cfg.max_body_bytes, 5 * 1024 * 1024);
    }

    #[test]
    fn http_config_reads_body_size_in_bytes() {
        let env = EnvConfig::from_iter([("HTTP_MAX_BODY_BYTES", "3145728")]);

        let cfg = HttpConfig::from_env_config(&env);

        assert_eq!(cfg.max_body_bytes, 3 * 1024 * 1024);
    }

    #[test]
    fn http_config_reads_body_size_in_megabytes() {
        let env = EnvConfig::from_iter([("HTTP_MAX_BODY_MB", "7")]);

        let cfg = HttpConfig::from_env_config(&env);

        assert_eq!(cfg.max_body_bytes, 7 * 1024 * 1024);
    }

    #[test]
    fn http_body_bytes_takes_precedence_over_megabytes() {
        let env = EnvConfig::from_iter([
            ("HTTP_MAX_BODY_BYTES", "3145728"),
            ("HTTP_MAX_BODY_MB", "99"),
        ]);

        let cfg = HttpConfig::from_env_config(&env);

        assert_eq!(cfg.max_body_bytes, 3 * 1024 * 1024);
    }

    #[test]
    fn http_invalid_bytes_falls_back_to_megabytes() {
        let env = EnvConfig::from_iter([
            ("HTTP_MAX_BODY_BYTES", "invalid"),
            ("HTTP_MAX_BODY_MB", "7"),
        ]);

        let cfg = HttpConfig::from_env_config(&env);

        assert_eq!(cfg.max_body_bytes, 7 * 1024 * 1024);
    }

    #[test]
    fn http_invalid_numbers_use_default() {
        let env = EnvConfig::from_iter([
            ("HTTP_MAX_BODY_BYTES", "invalid"),
            ("HTTP_MAX_BODY_MB", "also-invalid"),
        ]);

        let cfg = HttpConfig::from_env_config(&env);

        assert_eq!(cfg.max_body_bytes, 5 * 1024 * 1024);
    }

    #[test]
    fn cors_config_holds_values() {
        let cfg = CorsConfig {
            enabled: true,
            env: "http://localhost:5173".into(),
            credentials: true,
        };

        assert!(cfg.enabled);
        assert_eq!(cfg.env, "http://localhost:5173");
        assert!(cfg.credentials);
    }

    #[test]
    fn cors_config_uses_defaults() {
        let env = EnvConfig::default();

        let cfg = CorsConfig::from_env_config(&env);

        assert!(!cfg.enabled);
        assert_eq!(cfg.env, "");
        assert!(!cfg.credentials);
    }

    #[test]
    fn cors_config_reads_values() {
        let env = EnvConfig::from_iter([
            ("CORS_ENABLED", "true"),
            (
                "CORS_ORIGINS",
                "https://a.example.com,https://b.example.com",
            ),
            ("CORS_CREDENTIALS", "true"),
        ]);

        let cfg = CorsConfig::from_env_config(&env);

        assert!(cfg.enabled);
        assert_eq!(cfg.env, "https://a.example.com,https://b.example.com");
        assert!(cfg.credentials);
    }

    #[test]
    fn cors_config_accepts_truthy_flags() {
        for value in ["1", "true", "TRUE", "Yes", " on  "] {
            let env = EnvConfig::from_iter([("CORS_ENABLED", value), ("CORS_CREDENTIALS", value)]);

            let cfg = CorsConfig::from_env_config(&env);

            assert!(cfg.enabled, "Expected {value:?} to enable CORS");

            assert!(cfg.credentials, "Expected {value:?} to enable credentials");
        }
    }

    #[test]
    fn cors_config_accepts_falsy_flags() {
        for value in ["0", "false", "FALSE", "No", "off", "", "  ", "invalid"] {
            let env = EnvConfig::from_iter([("CORS_ENABLED", value), ("CORS_CREDENTIALS", value)]);

            let cfg = CorsConfig::from_env_config(&env);

            assert!(!cfg.enabled, "Expected {value:?} to disable CORS");

            assert!(
                !cfg.credentials,
                "Expected {value:?} to disable credentials"
            );
        }
    }

    #[test]
    fn http_and_cors_configs_are_clone_and_debug() {
        let http_cfg = HttpConfig {
            max_body_bytes: 123,
        };

        let http_clone = http_cfg.clone();

        assert_eq!(http_cfg, http_clone);

        let cors_cfg = CorsConfig {
            enabled: true,
            env: "dev".into(),
            credentials: false,
        };

        let cors_clone = cors_cfg.clone();

        assert_eq!(cors_cfg, cors_clone);

        let debug = format!("{cors_cfg:?}");

        assert!(debug.contains("enabled"));
        assert!(debug.contains("dev"));
    }
}
