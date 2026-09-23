//! # CSRF Configuration
//!
//! Provides configuration for CSRF (Cross-Site Request Forgery) protection,
//! including secret key management, cookie naming, and cookie security flags.
//!
//! CSRF configuration can be built either from the current process environment,
//! from an [`EnvConfig`] environment snapshot, or from a custom environment
//! provider.
//!
//! The following environment variables are supported:
//!
//! - `CSRF_SECRET` — base string used to derive a 32-byte secret
//! - `CSRF_COOKIE_NAME` — CSRF cookie name (default: `csrf`)
//! - `CSRF_COOKIE_SECURE` — enables the `Secure` cookie flag (default: `true`)
//! - `CSRF_COOKIE_HTTP_ONLY` — enables the `HttpOnly` cookie flag (default: `true`)
//! - `CSRF_COOKIE_HTTPONLY` — deprecated alias of `CSRF_COOKIE_HTTP_ONLY`
//!
//! When both HttpOnly variables are present, `CSRF_COOKIE_HTTP_ONLY` takes
//! precedence. The deprecated alias remains supported for backward
//! compatibility.
//!
//! If `CSRF_SECRET` is missing, a random 32-byte secret is generated.
//!
//! # Examples
//!
//! ## From the process environment
//!
//! ```rust,no_run
//! use wzs_web::config::csrf::CsrfConfig;
//!
//! let cfg = CsrfConfig::from_env();
//!
//! assert_eq!(cfg.secret.len(), 32);
//! ```
//!
//! ## From an environment snapshot
//!
//! ```rust
//! use wzs_web::config::csrf::{derive_secret_from_string, CsrfConfig};
//! use wzs_web::config::env::EnvConfig;
//!
//! let env = EnvConfig::from_pairs([
//!     ("CSRF_SECRET", "my-top-secret"),
//!     ("CSRF_COOKIE_NAME", "public_csrf"),
//!     ("CSRF_COOKIE_SECURE", "false"),
//!     ("CSRF_COOKIE_HTTP_ONLY", "true"),
//! ]);
//!
//! let cfg = CsrfConfig::from_env_config(&env);
//!
//! assert_eq!(
//!     cfg.secret,
//!     derive_secret_from_string("my-top-secret")
//! );
//! assert_eq!(cfg.cookie_name, "public_csrf");
//! assert!(!cfg.cookie_secure);
//! assert!(cfg.cookie_http_only);
//! ```

use std::fmt;

use rand::Rng;
use sha2::{Digest, Sha256};

use crate::config::env::EnvConfig;

/// Default name used for the CSRF cookie.
pub const DEFAULT_CSRF_COOKIE_NAME: &str = "csrf";

/// Configuration for CSRF protection.
///
/// Controls secret key generation, cookie naming, and cookie security flags.
///
/// `CsrfConfig` can be constructed from:
///
/// - the current process environment with [`CsrfConfig::from_env`],
/// - an [`EnvConfig`] snapshot with [`CsrfConfig::from_env_config`], or
/// - a custom provider with [`CsrfConfig::from_env_with`].
#[derive(Clone, PartialEq, Eq)]
pub struct CsrfConfig {
    /// Secret key used for CSRF token signing.
    pub secret: [u8; 32],

    /// Name of the cookie that stores the CSRF token.
    ///
    /// Defaults to [`DEFAULT_CSRF_COOKIE_NAME`]. Leading and trailing
    /// whitespace is removed. An empty or whitespace-only value also falls
    /// back to the default.
    pub cookie_name: String,

    /// Whether the CSRF cookie uses the `Secure` flag.
    ///
    /// Defaults to `true`.
    pub cookie_secure: bool,

    /// Whether the CSRF cookie uses the `HttpOnly` flag.
    ///
    /// Defaults to `true`.
    pub cookie_http_only: bool,
}

impl CsrfConfig {
    /// Builds a [`CsrfConfig`] from the current process environment.
    ///
    /// This method is retained for backward compatibility.
    ///
    /// New configuration code should generally capture the environment once
    /// with [`EnvConfig::from_env`] and use [`CsrfConfig::from_env_config`].
    pub fn from_env() -> Self {
        let env = EnvConfig::from_env();

        Self::from_env_config(&env)
    }

    /// Builds a [`CsrfConfig`] from an [`EnvConfig`] snapshot.
    ///
    /// If `CSRF_SECRET` is present, its value is deterministically converted
    /// into a 32-byte secret using SHA-256.
    ///
    /// If `CSRF_SECRET` is missing, a random 32-byte secret is generated.
    ///
    /// `CSRF_COOKIE_NAME` defaults to [`DEFAULT_CSRF_COOKIE_NAME`].
    ///
    /// `CSRF_COOKIE_SECURE` and `CSRF_COOKIE_HTTP_ONLY` default to `true` when
    /// missing. The deprecated `CSRF_COOKIE_HTTPONLY` name is accepted when
    /// `CSRF_COOKIE_HTTP_ONLY` is absent.
    ///
    /// For compatibility with the existing behavior, an explicitly supplied
    /// value is `true` only when it is one of the recognized truthy values.
    /// Other supplied values are treated as `false`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use wzs_web::config::csrf::CsrfConfig;
    /// use wzs_web::config::env::EnvConfig;
    ///
    /// let env = EnvConfig::from_pairs([
    ///     ("CSRF_SECRET", "my-top-secret"),
    ///     ("CSRF_COOKIE_NAME", "public_csrf"),
    ///     ("CSRF_COOKIE_SECURE", "false"),
    ///     ("CSRF_COOKIE_HTTP_ONLY", "true"),
    /// ]);
    ///
    /// let cfg = CsrfConfig::from_env_config(&env);
    ///
    /// assert_eq!(cfg.cookie_name, "public_csrf");
    /// assert!(!cfg.cookie_secure);
    /// assert!(cfg.cookie_http_only);
    /// ```
    pub fn from_env_config(env: &EnvConfig) -> Self {
        Self::from_env_with(|key| env.get_string(key))
    }

    /// Builds a [`CsrfConfig`] using a custom environment value provider.
    ///
    /// This method is retained for backward compatibility and can also be
    /// useful for tests or custom configuration sources.
    ///
    /// If `CSRF_SECRET` is missing, a random secret is generated.
    ///
    /// The cookie name defaults to [`DEFAULT_CSRF_COOKIE_NAME`] and cookie
    /// flags default to `true` when missing.
    pub fn from_env_with<F>(get: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let secret = match get("CSRF_SECRET") {
            Some(value) if !value.trim().is_empty() => derive_secret_from_string(&value),
            _ => random_secret(),
        };

        let cookie_name = get("CSRF_COOKIE_NAME")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_CSRF_COOKIE_NAME.to_string());

        let cookie_secure = get("CSRF_COOKIE_SECURE")
            .as_deref()
            .map(is_truthy)
            .unwrap_or(true);

        // Prefer the consistently separated name. Keep the previous spelling
        // as a fallback so existing deployments continue to work.
        let cookie_http_only = get("CSRF_COOKIE_HTTP_ONLY")
            .or_else(|| get("CSRF_COOKIE_HTTPONLY"))
            .as_deref()
            .map(is_truthy)
            .unwrap_or(true);

        Self {
            secret,
            cookie_name,
            cookie_secure,
            cookie_http_only,
        }
    }
}

/// Provides a redacted debug representation of [`CsrfConfig`].
///
/// The CSRF signing secret must not be written to logs or other debug
/// output. The secret is therefore always displayed as `[REDACTED]`.
impl fmt::Debug for CsrfConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CsrfConfig")
            .field("secret", &"[REDACTED]")
            .field("cookie_name", &self.cookie_name)
            .field("cookie_secure", &self.cookie_secure)
            .field("cookie_http_only", &self.cookie_http_only)
            .finish()
    }
}

/// Returns `true` if a string represents a truthy value.
///
/// Accepted values are case-insensitive:
///
/// - `"1"`
/// - `"true"`
/// - `"yes"`
/// - `"on"`
///
/// Leading and trailing whitespace is ignored.
///
/// All other values return `false`.
fn is_truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Derives a deterministic 32-byte secret key from a string.
///
/// SHA-256 is used to convert an arbitrary string into a fixed-length key.
///
/// # Example
///
/// ```rust
/// use wzs_web::config::csrf::derive_secret_from_string;
///
/// let first = derive_secret_from_string("secret");
/// let second = derive_secret_from_string("secret");
///
/// assert_eq!(first, second);
/// assert_eq!(first.len(), 32);
/// ```
pub fn derive_secret_from_string(s: &str) -> [u8; 32] {
    let digest = Sha256::digest(s.as_bytes());

    let mut key = [0u8; 32];
    key.copy_from_slice(&digest[..32]);

    key
}

/// Generates a new random 32-byte secret key.
///
/// # Example
///
/// ```rust
/// use wzs_web::config::csrf::random_secret;
///
/// let secret = random_secret();
///
/// assert_eq!(secret.len(), 32);
/// ```
pub fn random_secret() -> [u8; 32] {
    let mut key = [0u8; 32];

    rand::rng().fill_bytes(&mut key);

    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_config_uses_defaults_when_missing() {
        let env = EnvConfig::default();

        let cfg = CsrfConfig::from_env_config(&env);

        assert_eq!(cfg.secret.len(), 32);
        assert_eq!(cfg.cookie_name, DEFAULT_CSRF_COOKIE_NAME);
        assert!(cfg.cookie_secure);
        assert!(cfg.cookie_http_only);
    }

    #[test]
    fn from_env_config_respects_secret_and_flags() {
        let env = EnvConfig::from_pairs([
            ("CSRF_SECRET", "my-top-secret"),
            ("CSRF_COOKIE_NAME", "public_csrf"),
            ("CSRF_COOKIE_SECURE", "false"),
            ("CSRF_COOKIE_HTTP_ONLY", "0"),
        ]);

        let cfg = CsrfConfig::from_env_config(&env);

        assert_eq!(cfg.secret, derive_secret_from_string("my-top-secret"));
        assert_eq!(cfg.cookie_name, "public_csrf");
        assert!(!cfg.cookie_secure);
        assert!(!cfg.cookie_http_only);
    }

    #[test]
    fn from_env_config_generates_random_secret_when_missing() {
        let env = EnvConfig::default();

        let first = CsrfConfig::from_env_config(&env);
        let second = CsrfConfig::from_env_config(&env);

        assert_eq!(first.secret.len(), 32);
        assert_eq!(second.secret.len(), 32);
        assert_ne!(first.secret, second.secret);
    }

    #[test]
    fn from_env_config_derives_stable_secret_when_configured() {
        let env = EnvConfig::from_pairs([("CSRF_SECRET", "my-top-secret")]);

        let first = CsrfConfig::from_env_config(&env);
        let second = CsrfConfig::from_env_config(&env);

        assert_eq!(first.secret, second.secret);
        assert_eq!(first.secret, derive_secret_from_string("my-top-secret"));
    }

    #[test]
    fn from_env_config_defaults_cookie_flags_to_true() {
        let env = EnvConfig::from_pairs([("CSRF_SECRET", "my-top-secret")]);

        let cfg = CsrfConfig::from_env_config(&env);

        assert!(cfg.cookie_secure);
        assert!(cfg.cookie_http_only);
    }

    #[test]
    fn from_env_config_defaults_cookie_name_when_missing() {
        let env = EnvConfig::default();

        let cfg = CsrfConfig::from_env_config(&env);

        assert_eq!(cfg.cookie_name, DEFAULT_CSRF_COOKIE_NAME);
    }

    #[test]
    fn from_env_config_loads_and_trims_cookie_name() {
        let env = EnvConfig::from_pairs([("CSRF_COOKIE_NAME", "  public_csrf  ")]);

        let cfg = CsrfConfig::from_env_config(&env);

        assert_eq!(cfg.cookie_name, "public_csrf");
    }

    #[test]
    fn from_env_config_defaults_cookie_name_when_empty() {
        for value in ["", " ", "   ", "\t", "\n"] {
            let env = EnvConfig::from_pairs([("CSRF_COOKIE_NAME", value)]);

            let cfg = CsrfConfig::from_env_config(&env);

            assert_eq!(
                cfg.cookie_name, DEFAULT_CSRF_COOKIE_NAME,
                "Expected the default cookie name for {value:?}"
            );
        }
    }

    #[test]
    fn from_env_config_accepts_truthy_cookie_flags() {
        for value in ["1", "true", "TRUE", "Yes", " on  "] {
            let env = EnvConfig::from_pairs([
                ("CSRF_COOKIE_SECURE", value),
                ("CSRF_COOKIE_HTTP_ONLY", value),
            ]);

            let cfg = CsrfConfig::from_env_config(&env);

            assert!(
                cfg.cookie_secure,
                "Expected {value:?} to enable cookie_secure"
            );

            assert!(
                cfg.cookie_http_only,
                "Expected {value:?} to enable cookie_http_only"
            );
        }
    }

    #[test]
    fn from_env_config_accepts_falsy_cookie_flags() {
        for value in ["0", "false", "no", "off", "", "  "] {
            let env = EnvConfig::from_pairs([
                ("CSRF_COOKIE_SECURE", value),
                ("CSRF_COOKIE_HTTP_ONLY", value),
            ]);

            let cfg = CsrfConfig::from_env_config(&env);

            assert!(
                !cfg.cookie_secure,
                "Expected {value:?} to disable cookie_secure"
            );

            assert!(
                !cfg.cookie_http_only,
                "Expected {value:?} to disable cookie_http_only"
            );
        }
    }

    #[test]
    fn from_env_config_preserves_legacy_behavior_for_invalid_flags() {
        let env = EnvConfig::from_pairs([
            ("CSRF_COOKIE_SECURE", "invalid"),
            ("CSRF_COOKIE_HTTP_ONLY", "invalid"),
        ]);

        let cfg = CsrfConfig::from_env_config(&env);

        assert!(!cfg.cookie_secure);
        assert!(!cfg.cookie_http_only);
    }

    #[test]
    fn from_env_with_uses_defaults_when_missing() {
        let cfg = CsrfConfig::from_env_with(|_| None);

        assert_eq!(cfg.secret.len(), 32);
        assert_eq!(cfg.cookie_name, DEFAULT_CSRF_COOKIE_NAME);
        assert!(cfg.cookie_secure);
        assert!(cfg.cookie_http_only);
    }

    #[test]
    fn from_env_with_respects_secret_and_flags() {
        let env = EnvConfig::from_pairs([
            ("CSRF_SECRET", "my-top-secret"),
            ("CSRF_COOKIE_NAME", "public_csrf"),
            ("CSRF_COOKIE_SECURE", "false"),
            ("CSRF_COOKIE_HTTP_ONLY", "0"),
        ]);

        let cfg = CsrfConfig::from_env_with(|key| env.get_string(key));

        assert_eq!(cfg.secret, derive_secret_from_string("my-top-secret"));
        assert_eq!(cfg.cookie_name, "public_csrf");
        assert!(!cfg.cookie_secure);
        assert!(!cfg.cookie_http_only);
    }

    #[test]
    fn from_env_config_supports_legacy_http_only_name() {
        let env = EnvConfig::from_pairs([("CSRF_COOKIE_HTTPONLY", "false")]);

        let cfg = CsrfConfig::from_env_config(&env);

        assert!(!cfg.cookie_http_only);
    }

    #[test]
    fn canonical_http_only_name_takes_precedence_over_legacy_name() {
        let env = EnvConfig::from_pairs([
            ("CSRF_COOKIE_HTTP_ONLY", "false"),
            ("CSRF_COOKIE_HTTPONLY", "true"),
        ]);

        let cfg = CsrfConfig::from_env_config(&env);

        assert!(!cfg.cookie_http_only);
    }

    #[test]
    fn derive_secret_function_is_stable() {
        let first = derive_secret_from_string("abc");
        let second = derive_secret_from_string("abc");

        assert_eq!(first, second);

        let different = derive_secret_from_string("xyz");

        assert_ne!(first, different);
    }

    #[test]
    fn random_secret_function_returns_32_bytes() {
        let secret = random_secret();

        assert_eq!(secret.len(), 32);
    }

    #[test]
    fn random_secret_function_varies_across_calls() {
        let first = random_secret();
        let second = random_secret();

        assert_ne!(first, second);
    }

    #[test]
    fn is_truthy_variants() {
        for value in ["1", "true", "TRUE", "Yes", " on  "] {
            assert!(is_truthy(value), "Expected {value:?} to be truthy");
        }

        for value in ["0", "false", "no", "off", "", "  ", "invalid"] {
            assert!(!is_truthy(value), "Expected {value:?} to be falsy");
        }
    }

    #[test]
    fn from_env_config_generates_random_secret_when_secret_is_empty() {
        for value in ["", " ", "   ", "\t", "\n"] {
            let env = EnvConfig::from_pairs([("CSRF_SECRET", value)]);

            let first = CsrfConfig::from_env_config(&env);
            let second = CsrfConfig::from_env_config(&env);

            assert_ne!(
                first.secret, second.secret,
                "Expected random secrets for {value:?}"
            );
        }
    }

    #[test]
    fn debug_redacts_csrf_secret() {
        let env = EnvConfig::from_pairs([
            ("CSRF_SECRET", "super-secret-csrf-value"),
            ("CSRF_COOKIE_NAME", "public_csrf"),
            ("CSRF_COOKIE_SECURE", "false"),
            ("CSRF_COOKIE_HTTP_ONLY", "true"),
        ]);

        let cfg = CsrfConfig::from_env_config(&env);
        let debug = format!("{cfg:?}");

        assert!(debug.contains("CsrfConfig"));
        assert!(debug.contains("[REDACTED]"));

        assert!(!debug.contains("super-secret-csrf-value"));

        assert!(debug.contains("cookie_name"));
        assert!(debug.contains("public_csrf"));

        assert!(debug.contains("cookie_secure"));
        assert!(debug.contains("false"));

        assert!(debug.contains("cookie_http_only"));
        assert!(debug.contains("true"));
    }

    #[test]
    fn debug_does_not_expose_derived_csrf_secret_bytes() {
        let secret = derive_secret_from_string("super-secret-csrf-value");

        let cfg = CsrfConfig {
            secret,
            cookie_name: DEFAULT_CSRF_COOKIE_NAME.to_string(),
            cookie_secure: true,
            cookie_http_only: true,
        };

        let debug = format!("{cfg:?}");

        assert!(debug.contains("[REDACTED]"));

        assert!(!debug.contains(&format!("{secret:?}")));
    }
}
