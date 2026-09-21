//! # Authentication Configuration
//!
//! Provides application-level authentication configuration.
//!
//! Authentication is optional. JWT authentication is enabled only when
//! `JWT_SECRET` contains a non-empty value.
//!
//! Missing, empty, or whitespace-only `JWT_SECRET` values disable JWT
//! authentication.
//!
//! # Example
//!
//! ```rust
//! use wzs_web::config::auth::AuthConfig;
//! use wzs_web::config::env::EnvConfig;
//!
//! let env = EnvConfig::from_pairs([
//!     ("JWT_SECRET", "my-secret"),
//! ]);
//!
//! let cfg = AuthConfig::from_env_config(&env);
//!
//! assert!(cfg.is_enabled());
//! assert_eq!(cfg.jwt_secret(), Some("my-secret"));
//! ```

use std::fmt;

use crate::config::env::EnvConfig;

/// Configuration for application authentication.
///
/// JWT authentication is represented explicitly with an optional secret:
///
/// - `Some(secret)` — JWT authentication is enabled.
/// - `None` — JWT authentication is disabled.
///
/// A missing, empty, or whitespace-only `JWT_SECRET` produces `None`.
///
/// The JWT secret is intentionally not exposed through [`Debug`].
#[derive(Clone, PartialEq, Eq)]
pub struct AuthConfig {
    jwt_secret: Option<String>,
}

impl AuthConfig {
    /// Builds authentication configuration from the current process
    /// environment.
    ///
    /// This method is provided for consistency with other configuration
    /// types.
    ///
    /// New application configuration should generally capture the
    /// environment once with [`EnvConfig::from_env`] and use
    /// [`AuthConfig::from_env_config`].
    pub fn from_env() -> Self {
        let env = EnvConfig::from_env();

        Self::from_env_config(&env)
    }

    /// Builds authentication configuration from an [`EnvConfig`] snapshot.
    ///
    /// JWT authentication is disabled when `JWT_SECRET` is:
    ///
    /// - missing,
    /// - empty, or
    /// - whitespace-only.
    ///
    /// When the secret contains at least one non-whitespace character,
    /// the original value is preserved exactly. Leading and trailing
    /// whitespace is used only to determine whether the value is empty;
    /// it is not removed from the actual secret.
    ///
    /// # Example
    ///
    /// ```rust
    /// use wzs_web::config::auth::AuthConfig;
    /// use wzs_web::config::env::EnvConfig;
    ///
    /// let env = EnvConfig::from_pairs([
    ///     ("JWT_SECRET", "unit-test-secret"),
    /// ]);
    ///
    /// let cfg = AuthConfig::from_env_config(&env);
    ///
    /// assert!(cfg.is_enabled());
    /// assert_eq!(cfg.jwt_secret(), Some("unit-test-secret"));
    /// ```
    pub fn from_env_config(env: &EnvConfig) -> Self {
        let jwt_secret = env
            .get_string("JWT_SECRET")
            .filter(|value| !value.trim().is_empty());

        Self { jwt_secret }
    }

    /// Returns `true` when JWT authentication is enabled.
    ///
    /// Authentication is enabled when a non-empty `JWT_SECRET`
    /// was configured.
    pub fn is_enabled(&self) -> bool {
        self.jwt_secret.is_some()
    }

    /// Returns the configured JWT secret.
    ///
    /// Returns `None` when authentication is disabled.
    ///
    /// The returned value borrows the secret stored in this configuration
    /// and does not allocate a new `String`.
    pub fn jwt_secret(&self) -> Option<&str> {
        self.jwt_secret.as_deref()
    }
}

/// Provides a redacted debug representation.
///
/// The JWT secret must not be written to logs or other debug output.
/// When a secret is configured, only `[REDACTED]` is displayed.
impl fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthConfig")
            .field(
                "jwt_secret",
                &self.jwt_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authentication_is_enabled_when_secret_is_present() {
        let env = EnvConfig::from_pairs([("JWT_SECRET", "unit-test-secret")]);

        let cfg = AuthConfig::from_env_config(&env);

        assert!(cfg.is_enabled());
        assert_eq!(cfg.jwt_secret(), Some("unit-test-secret"));
    }

    #[test]
    fn authentication_is_disabled_when_secret_is_missing() {
        let env = EnvConfig::default();

        let cfg = AuthConfig::from_env_config(&env);

        assert!(!cfg.is_enabled());
        assert_eq!(cfg.jwt_secret(), None);
    }

    #[test]
    fn authentication_is_disabled_when_secret_is_empty() {
        for value in ["", " ", "   ", "\t", "\n"] {
            let env = EnvConfig::from_pairs([("JWT_SECRET", value)]);

            let cfg = AuthConfig::from_env_config(&env);

            assert!(
                !cfg.is_enabled(),
                "Expected authentication to be disabled for {value:?}"
            );

            assert_eq!(cfg.jwt_secret(), None);
        }
    }

    #[test]
    fn secret_whitespace_is_preserved_when_non_empty() {
        let env = EnvConfig::from_pairs([("JWT_SECRET", "  secret  ")]);

        let cfg = AuthConfig::from_env_config(&env);

        assert!(cfg.is_enabled());
        assert_eq!(cfg.jwt_secret(), Some("  secret  "));
    }

    #[test]
    fn configs_with_same_secret_are_equal() {
        let first_env = EnvConfig::from_pairs([("JWT_SECRET", "secret")]);

        let second_env = EnvConfig::from_pairs([("JWT_SECRET", "secret")]);

        let first = AuthConfig::from_env_config(&first_env);
        let second = AuthConfig::from_env_config(&second_env);

        assert_eq!(first, second);
    }

    #[test]
    fn configs_with_different_secrets_are_not_equal() {
        let first_env = EnvConfig::from_pairs([("JWT_SECRET", "first-secret")]);

        let second_env = EnvConfig::from_pairs([("JWT_SECRET", "second-secret")]);

        let first = AuthConfig::from_env_config(&first_env);
        let second = AuthConfig::from_env_config(&second_env);

        assert_ne!(first, second);
    }

    #[test]
    fn config_is_cloneable() {
        let env = EnvConfig::from_pairs([("JWT_SECRET", "secret")]);

        let cfg = AuthConfig::from_env_config(&env);
        let cloned = cfg.clone();

        assert_eq!(cfg, cloned);
        assert_eq!(cloned.jwt_secret(), Some("secret"));
    }

    #[test]
    fn debug_redacts_jwt_secret() {
        let env = EnvConfig::from_pairs([("JWT_SECRET", "super-secret-value")]);

        let cfg = AuthConfig::from_env_config(&env);
        let debug = format!("{cfg:?}");

        assert!(debug.contains("AuthConfig"));
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("super-secret-value"));
    }

    #[test]
    fn debug_represents_disabled_authentication_without_secret() {
        let env = EnvConfig::default();

        let cfg = AuthConfig::from_env_config(&env);
        let debug = format!("{cfg:?}");

        assert!(debug.contains("AuthConfig"));
        assert!(debug.contains("None"));
        assert!(!debug.contains("[REDACTED]"));
    }
}
