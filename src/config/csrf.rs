//! # CSRF Configuration
//!
//! Provides configuration for CSRF (Cross-Site Request Forgery) protection,
//! including secret key management and cookie security flags.
//!
//! CSRF configuration can be built either from the current process environment,
//! from an [`EnvConfig`] environment snapshot, or from a custom environment
//! provider.
//!
//! The following environment variables are supported:
//!
//! - `CSRF_SECRET` — base string used to derive a 32-byte secret
//! - `CSRF_COOKIE_SECURE` — enables the `Secure` cookie flag (default: `true`)
//! - `CSRF_COOKIE_HTTPONLY` — enables the `HttpOnly` cookie flag (default: `true`)
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
//! let env = EnvConfig::from_iter([
//!     ("CSRF_SECRET", "my-top-secret"),
//!     ("CSRF_COOKIE_SECURE", "false"),
//!     ("CSRF_COOKIE_HTTPONLY", "true"),
//! ]);
//!
//! let cfg = CsrfConfig::from_env_config(&env);
//!
//! assert_eq!(
//!     cfg.secret,
//!     derive_secret_from_string("my-top-secret")
//! );
//! assert!(!cfg.cookie_secure);
//! assert!(cfg.cookie_http_only);
//! ```

use std::env as std_env;

use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::config::env::EnvConfig;

/// Configuration for CSRF protection.
///
/// Controls secret key generation and cookie security flags.
///
/// `CsrfConfig` can be constructed from:
///
/// - the current process environment with [`CsrfConfig::from_env`],
/// - an [`EnvConfig`] snapshot with [`CsrfConfig::from_env_config`], or
/// - a custom provider with [`CsrfConfig::from_env_with`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CsrfConfig {
    /// Secret key used for CSRF token signing.
    pub secret: [u8; 32],

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
    /// `CSRF_COOKIE_SECURE` and `CSRF_COOKIE_HTTPONLY` default to `true` when
    /// missing.
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
    /// let env = EnvConfig::from_iter([
    ///     ("CSRF_SECRET", "my-top-secret"),
    ///     ("CSRF_COOKIE_SECURE", "false"),
    ///     ("CSRF_COOKIE_HTTPONLY", "true"),
    /// ]);
    ///
    /// let cfg = CsrfConfig::from_env_config(&env);
    ///
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
    /// Cookie flags default to `true` when missing.
    pub fn from_env_with<F>(get: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let secret = match get("CSRF_SECRET") {
            Some(value) => derive_secret_from_string(&value),
            None => random_secret(),
        };

        let cookie_secure = get("CSRF_COOKIE_SECURE")
            .as_deref()
            .map(is_truthy)
            .unwrap_or(true);

        let cookie_http_only = get("CSRF_COOKIE_HTTPONLY")
            .as_deref()
            .map(is_truthy)
            .unwrap_or(true);

        Self {
            secret,
            cookie_secure,
            cookie_http_only,
        }
    }

    /// Returns `true` if CSRF protection should be active.
    ///
    /// This method currently preserves the existing behavior and checks
    /// whether `CSRF_SECRET` exists in the current process environment.
    ///
    /// Note that this means the result is not derived from an [`EnvConfig`]
    /// snapshot. This behavior is retained temporarily for compatibility and
    /// can be revisited when the top-level configuration model is redesigned.
    pub fn is_enabled(&self) -> bool {
        std_env::var("CSRF_SECRET").is_ok()
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
    use temp_env;

    #[test]
    fn from_env_config_uses_defaults_when_missing() {
        let env = EnvConfig::default();

        let cfg = CsrfConfig::from_env_config(&env);

        assert_eq!(cfg.secret.len(), 32);
        assert!(cfg.cookie_secure);
        assert!(cfg.cookie_http_only);
    }

    #[test]
    fn from_env_config_respects_secret_and_flags() {
        let env = EnvConfig::from_iter([
            ("CSRF_SECRET", "my-top-secret"),
            ("CSRF_COOKIE_SECURE", "false"),
            ("CSRF_COOKIE_HTTPONLY", "0"),
        ]);

        let cfg = CsrfConfig::from_env_config(&env);

        assert_eq!(cfg.secret, derive_secret_from_string("my-top-secret"));
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
        let env = EnvConfig::from_iter([("CSRF_SECRET", "my-top-secret")]);

        let first = CsrfConfig::from_env_config(&env);
        let second = CsrfConfig::from_env_config(&env);

        assert_eq!(first.secret, second.secret);
        assert_eq!(first.secret, derive_secret_from_string("my-top-secret"));
    }

    #[test]
    fn from_env_config_defaults_cookie_flags_to_true() {
        let env = EnvConfig::from_iter([("CSRF_SECRET", "my-top-secret")]);

        let cfg = CsrfConfig::from_env_config(&env);

        assert!(cfg.cookie_secure);
        assert!(cfg.cookie_http_only);
    }

    #[test]
    fn from_env_config_accepts_truthy_cookie_flags() {
        for value in ["1", "true", "TRUE", "Yes", " on  "] {
            let env = EnvConfig::from_iter([
                ("CSRF_COOKIE_SECURE", value),
                ("CSRF_COOKIE_HTTPONLY", value),
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
            let env = EnvConfig::from_iter([
                ("CSRF_COOKIE_SECURE", value),
                ("CSRF_COOKIE_HTTPONLY", value),
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
        let env = EnvConfig::from_iter([
            ("CSRF_COOKIE_SECURE", "invalid"),
            ("CSRF_COOKIE_HTTPONLY", "invalid"),
        ]);

        let cfg = CsrfConfig::from_env_config(&env);

        assert!(!cfg.cookie_secure);
        assert!(!cfg.cookie_http_only);
    }

    #[test]
    fn from_env_with_uses_defaults_when_missing() {
        let cfg = CsrfConfig::from_env_with(|_| None);

        assert_eq!(cfg.secret.len(), 32);
        assert!(cfg.cookie_secure);
        assert!(cfg.cookie_http_only);
    }

    #[test]
    fn from_env_with_respects_secret_and_flags() {
        let env = EnvConfig::from_iter([
            ("CSRF_SECRET", "my-top-secret"),
            ("CSRF_COOKIE_SECURE", "false"),
            ("CSRF_COOKIE_HTTPONLY", "0"),
        ]);

        let cfg = CsrfConfig::from_env_with(|key| env.get_string(key));

        assert_eq!(cfg.secret, derive_secret_from_string("my-top-secret"));
        assert!(!cfg.cookie_secure);
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
    fn is_enabled_returns_true_when_secret_is_set() {
        temp_env::with_vars(vec![("CSRF_SECRET", Some("my-top-secret"))], || {
            let cfg = CsrfConfig::from_env();

            assert!(cfg.is_enabled(), "Expected CSRF to be enabled");
        });
    }

    #[test]
    fn is_enabled_returns_false_when_secret_missing() {
        temp_env::with_vars(vec![("CSRF_SECRET", None::<&str>)], || {
            let cfg = CsrfConfig::from_env();

            assert!(!cfg.is_enabled(), "Expected CSRF to be disabled");
        });
    }
}
