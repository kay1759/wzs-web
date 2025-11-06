//! # CSRF Configuration
//!
//! Provides configuration for CSRF (Cross-Site Request Forgery) protection,
//! including secret key management and cookie security flags.
//!
//! The configuration reads from environment variables:
//! - `CSRF_SECRET` — base string used to derive a 32-byte secret (if missing, random key is generated)
//! - `CSRF_COOKIE_SECURE` — enables `Secure` cookie flag (default: `true`)
//! - `CSRF_COOKIE_HTTPONLY` — enables `HttpOnly` cookie flag (default: `true`)
//!
//! # Examples
//! ```rust
//! use wzs_web::config::csrf::CsrfConfig;
//!
//! let cfg = CsrfConfig::from_env();
//! assert!(cfg.cookie_secure);
//! assert_eq!(cfg.secret.len(), 32);
//! ```

use std::env as std_env;

use rand::RngCore;
use sha2::{Digest, Sha256};

/// Configuration for CSRF protection.
///
/// Controls secret key generation and cookie security flags.
///
/// # Example
/// ```rust
/// use wzs_web::config::csrf::CsrfConfig;
///
/// let cfg = CsrfConfig::from_env();
/// assert!(cfg.cookie_http_only);
/// assert!(cfg.cookie_secure);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CsrfConfig {
    pub secret: [u8; 32],
    pub cookie_secure: bool,
    pub cookie_http_only: bool,
}

impl CsrfConfig {
    /// Loads configuration from environment variables.
    ///
    /// # Environment variables
    /// - `CSRF_SECRET`
    /// - `CSRF_COOKIE_SECURE`
    /// - `CSRF_COOKIE_HTTPONLY`
    pub fn from_env() -> Self {
        Self::from_env_with(|k| std_env::var(k).ok())
    }

    /// Loads configuration using a custom key provider (for testing/mocking).
    pub fn from_env_with<F>(get: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let secret = match get("CSRF_SECRET") {
            Some(s) => derive_secret_from_string(&s),
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
    /// By default, CSRF is considered **enabled** if `CSRF_SECRET`
    /// was provided (i.e., not randomly generated).
    pub fn is_enabled(&self) -> bool {
        // Note: if the key was generated randomly, it means no explicit secret
        std_env::var("CSRF_SECRET").is_ok()
    }
}

/// Returns `true` if a string represents a truthy value.
///
/// Accepts (case-insensitive): `"1"`, `"true"`, `"yes"`, `"on"`.
fn is_truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Derives a deterministic 32-byte secret key from a string.
///
/// Useful for converting an environment string (e.g. `CSRF_SECRET`)
/// into a fixed-length HMAC key.
pub fn derive_secret_from_string(s: &str) -> [u8; 32] {
    let digest = Sha256::digest(s.as_bytes());
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest[..32]);
    key
}

/// Generates a new random 32-byte secret key.
pub fn random_secret() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::rng().fill_bytes(&mut key);
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use temp_env;

    #[test]
    fn from_env_with_uses_defaults_when_missing() {
        let get = |_k: &str| -> Option<String> { None };
        let cfg = CsrfConfig::from_env_with(get);

        assert_eq!(cfg.secret.len(), 32);
        assert!(cfg.cookie_secure);
        assert!(cfg.cookie_http_only);
    }

    #[test]
    fn from_env_with_respects_secret_and_flags() {
        let mut fake = HashMap::<String, String>::new();
        fake.insert("CSRF_SECRET".into(), "my-top-secret".into());
        fake.insert("CSRF_COOKIE_SECURE".into(), "false".into());
        fake.insert("CSRF_COOKIE_HTTPONLY".into(), "0".into());

        let cfg = CsrfConfig::from_env_with(|k| fake.get(k).cloned());

        let expected = derive_secret_from_string("my-top-secret");
        assert_eq!(cfg.secret, expected);

        assert!(!cfg.cookie_secure);
        assert!(!cfg.cookie_http_only);
    }

    #[test]
    fn random_secret_has_correct_length_and_varies_across_calls() {
        let a = CsrfConfig::from_env_with(|_| None);
        let b = CsrfConfig::from_env_with(|_| None);

        assert_eq!(a.secret.len(), 32);
        assert_eq!(b.secret.len(), 32);

        assert_ne!(a.secret, b.secret);

        assert!(a.cookie_secure);
        assert!(a.cookie_http_only);
        assert!(b.cookie_secure);
        assert!(b.cookie_http_only);
    }

    #[test]
    fn derive_secret_function_is_stable() {
        let k1 = derive_secret_from_string("abc");
        let k2 = derive_secret_from_string("abc");
        assert_eq!(k1, k2);

        let k3 = derive_secret_from_string("xyz");
        assert_ne!(k1, k3);
    }

    #[test]
    fn random_secret_function_returns_32_bytes() {
        let k = random_secret();
        assert_eq!(k.len(), 32);
    }

    #[test]
    fn is_truthy_variants() {
        for s in ["1", "true", "TRUE", "Yes", " on  "] {
            assert!(is_truthy(s));
        }
        for s in ["0", "false", "no", "off", "", "  "] {
            assert!(!is_truthy(s));
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
