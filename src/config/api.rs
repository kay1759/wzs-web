//! # API Configuration
//!
//! Provides configuration shared by an individual application API.
//!
//! An application may expose any number of API entry points with different
//! security and transport policies. The application determines the names and
//! environment-variable prefixes used by those APIs.
//!
//! For example:
//!
//! ```text
//! PUBLIC_*
//! ADMIN_*
//! PICKUP_*
//! ```
//!
//! Each API can independently configure:
//!
//! - CORS behavior
//! - CSRF protection
//! - authentication
//! - GraphQL authentication transport
//!
//! HTTP server-wide configuration such as request body limits remains
//! outside `ApiConfig` because it applies to the HTTP server rather than
//! to an individual API entry point.

use crate::config::{auth::AuthConfig, csrf::CsrfConfig, env::EnvConfig, web::CorsConfig};
use crate::graphql::config::GraphqlAuthConfig;

/// Configuration for an individual API entry point.
///
/// `ApiConfig` is application-agnostic. It does not assume any particular
/// API names or prefixes.
///
/// Applications may construct as many API configurations as required by
/// creating prefixed environment views or by using
/// [`ApiConfig::from_prefixed_env`].
#[derive(Clone, Debug)]
pub struct ApiConfig {
    /// Cross-Origin Resource Sharing configuration.
    pub cors: CorsConfig,

    /// Whether CSRF protection is enabled.
    ///
    /// CSRF protection is enabled when `CSRF_SECRET` contains a
    /// non-empty value in the API's scoped environment.
    pub enable_csrf: bool,

    /// CSRF signing and cookie configuration.
    pub csrf: CsrfConfig,

    /// Application authentication configuration.
    pub auth: AuthConfig,

    /// GraphQL authentication transport configuration.
    pub graphql_auth: GraphqlAuthConfig,
}

impl ApiConfig {
    /// Builds API configuration from an [`EnvConfig`] snapshot.
    ///
    /// The supplied environment is expected to contain API configuration
    /// without an application-level prefix:
    ///
    /// - `CORS_ENABLED`
    /// - `CORS_ORIGINS`
    /// - `CORS_CREDENTIALS`
    /// - `CSRF_SECRET`
    /// - `JWT_SECRET`
    /// - `JWT_COOKIE_NAME`
    ///
    /// `default_jwt_cookie_name` is used when `JWT_COOKIE_NAME` is missing,
    /// empty, or contains only whitespace.
    pub fn from_env_config(env: &EnvConfig, default_jwt_cookie_name: impl Into<String>) -> Self {
        let default_jwt_cookie_name = default_jwt_cookie_name.into();

        let cors = CorsConfig::from_env_config(env);

        let enable_csrf = env
            .get("CSRF_SECRET")
            .is_some_and(|value| !value.trim().is_empty());

        let csrf = CsrfConfig::from_env_config(env);
        let auth = AuthConfig::from_env_config(env);

        let jwt_cookie_name = env
            .get_string("JWT_COOKIE_NAME")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(default_jwt_cookie_name);

        let graphql_auth = GraphqlAuthConfig::new(jwt_cookie_name);

        Self {
            cors,
            enable_csrf,
            csrf,
            auth,
            graphql_auth,
        }
    }

    /// Builds API configuration from environment variables beginning with
    /// `prefix`.
    ///
    /// The prefix is removed before the scoped environment is passed to
    /// [`ApiConfig::from_env_config`].
    ///
    /// This allows applications to define any number of independent API
    /// configurations without requiring `wzs-web` to know their names.
    ///
    /// # Example
    ///
    /// ```
    /// use wzs_web::config::api::ApiConfig;
    /// use wzs_web::config::env::EnvConfig;
    ///
    /// let env = EnvConfig::from_pairs([
    ///     ("PICKUP_CORS_ENABLED", "true"),
    ///     ("PICKUP_JWT_SECRET", "pickup-secret"),
    ///     ("PICKUP_JWT_COOKIE_NAME", "pickup_session"),
    /// ]);
    ///
    /// let cfg =
    ///     ApiConfig::from_prefixed_env(&env, "PICKUP_", "pickup_token");
    ///
    /// assert!(cfg.cors.enabled);
    /// assert!(cfg.auth.is_enabled());
    /// assert_eq!(cfg.graphql_auth.jwt_cookie_name, "pickup_session");
    /// ```
    pub fn from_prefixed_env(
        env: &EnvConfig,
        prefix: &str,
        default_jwt_cookie_name: impl Into<String>,
    ) -> Self {
        let scoped_env = env.with_prefix(prefix);

        Self::from_env_config(&scoped_env, default_jwt_cookie_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_defaults_when_environment_is_empty() {
        let env = EnvConfig::default();

        let cfg = ApiConfig::from_env_config(&env, "auth_token");

        assert!(!cfg.cors.enabled);
        assert_eq!(cfg.cors.env, "");
        assert!(!cfg.cors.credentials);

        assert!(!cfg.enable_csrf);

        assert!(!cfg.auth.is_enabled());
        assert_eq!(cfg.auth.jwt_secret(), None);

        assert_eq!(cfg.graphql_auth.jwt_cookie_name, "auth_token");
    }

    #[test]
    fn loads_cors_configuration() {
        let env = EnvConfig::from_pairs([
            ("CORS_ENABLED", "true"),
            ("CORS_ORIGINS", "http://localhost:5173"),
            ("CORS_CREDENTIALS", "true"),
        ]);

        let cfg = ApiConfig::from_env_config(&env, "auth_token");

        assert!(cfg.cors.enabled);
        assert_eq!(cfg.cors.env, "http://localhost:5173");
        assert!(cfg.cors.credentials);
    }

    #[test]
    fn enables_csrf_when_secret_is_present() {
        let env = EnvConfig::from_pairs([("CSRF_SECRET", "csrf-secret")]);

        let cfg = ApiConfig::from_env_config(&env, "auth_token");

        assert!(cfg.enable_csrf);
    }

    #[test]
    fn disables_csrf_when_secret_is_missing() {
        let env = EnvConfig::default();

        let cfg = ApiConfig::from_env_config(&env, "auth_token");

        assert!(!cfg.enable_csrf);
    }

    #[test]
    fn disables_csrf_when_secret_is_empty() {
        for value in ["", " ", "   ", "\t", "\n"] {
            let env = EnvConfig::from_pairs([("CSRF_SECRET", value)]);

            let cfg = ApiConfig::from_env_config(&env, "auth_token");

            assert!(
                !cfg.enable_csrf,
                "Expected CSRF to be disabled for {value:?}"
            );
        }
    }

    #[test]
    fn loads_authentication_configuration() {
        let env = EnvConfig::from_pairs([("JWT_SECRET", "jwt-secret")]);

        let cfg = ApiConfig::from_env_config(&env, "auth_token");

        assert!(cfg.auth.is_enabled());
        assert_eq!(cfg.auth.jwt_secret(), Some("jwt-secret"));
    }

    #[test]
    fn authentication_is_optional() {
        let env = EnvConfig::default();

        let cfg = ApiConfig::from_env_config(&env, "auth_token");

        assert!(!cfg.auth.is_enabled());
        assert_eq!(cfg.auth.jwt_secret(), None);
    }

    #[test]
    fn stores_default_graphql_authentication_cookie_name() {
        let env = EnvConfig::default();

        let cfg = ApiConfig::from_env_config(&env, "wizis_token");

        assert_eq!(cfg.graphql_auth.jwt_cookie_name, "wizis_token");
    }

    #[test]
    fn loads_graphql_authentication_cookie_name_from_environment() {
        let env = EnvConfig::from_pairs([("JWT_COOKIE_NAME", "custom_token")]);

        let cfg = ApiConfig::from_env_config(&env, "default_token");

        assert_eq!(cfg.graphql_auth.jwt_cookie_name, "custom_token");
    }

    #[test]
    fn empty_graphql_authentication_cookie_name_uses_default() {
        for value in ["", " ", "   ", "\t", "\n"] {
            let env = EnvConfig::from_pairs([("JWT_COOKIE_NAME", value)]);

            let cfg = ApiConfig::from_env_config(&env, "default_token");

            assert_eq!(cfg.graphql_auth.jwt_cookie_name, "default_token");
        }
    }

    #[test]
    fn loads_configuration_from_arbitrary_prefix() {
        let env = EnvConfig::from_pairs([
            ("PICKUP_CORS_ENABLED", "true"),
            ("PICKUP_CORS_ORIGINS", "https://pickup.example.com"),
            ("PICKUP_CORS_CREDENTIALS", "true"),
            ("PICKUP_CSRF_SECRET", "pickup-csrf-secret"),
            ("PICKUP_JWT_SECRET", "pickup-jwt-secret"),
            ("PICKUP_JWT_COOKIE_NAME", "pickup_session"),
        ]);

        let cfg = ApiConfig::from_prefixed_env(&env, "PICKUP_", "pickup_token");

        assert!(cfg.cors.enabled);
        assert_eq!(cfg.cors.env, "https://pickup.example.com");
        assert!(cfg.cors.credentials);

        assert!(cfg.enable_csrf);

        assert!(cfg.auth.is_enabled());
        assert_eq!(cfg.auth.jwt_secret(), Some("pickup-jwt-secret"));

        assert_eq!(cfg.graphql_auth.jwt_cookie_name, "pickup_session");
    }

    #[test]
    fn supports_multiple_independent_api_prefixes() {
        let env = EnvConfig::from_pairs([
            ("PUBLIC_JWT_SECRET", "public-secret"),
            ("ADMIN_JWT_SECRET", "admin-secret"),
            ("PICKUP_JWT_SECRET", "pickup-secret"),
        ]);

        let public = ApiConfig::from_prefixed_env(&env, "PUBLIC_", "public_token");

        let admin = ApiConfig::from_prefixed_env(&env, "ADMIN_", "admin_token");

        let pickup = ApiConfig::from_prefixed_env(&env, "PICKUP_", "pickup_token");

        assert_eq!(public.auth.jwt_secret(), Some("public-secret"));
        assert_eq!(admin.auth.jwt_secret(), Some("admin-secret"));
        assert_eq!(pickup.auth.jwt_secret(), Some("pickup-secret"));

        assert_eq!(public.graphql_auth.jwt_cookie_name, "public_token");
        assert_eq!(admin.graphql_auth.jwt_cookie_name, "admin_token");
        assert_eq!(pickup.graphql_auth.jwt_cookie_name, "pickup_token");
    }

    #[test]
    fn prefixed_configuration_does_not_read_other_api_values() {
        let env = EnvConfig::from_pairs([
            ("PUBLIC_JWT_SECRET", "public-secret"),
            ("ADMIN_JWT_SECRET", "admin-secret"),
        ]);

        let pickup = ApiConfig::from_prefixed_env(&env, "PICKUP_", "pickup_token");

        assert!(!pickup.auth.is_enabled());
        assert_eq!(pickup.auth.jwt_secret(), None);
    }

    #[test]
    fn api_config_is_cloneable() {
        let env =
            EnvConfig::from_pairs([("JWT_SECRET", "jwt-secret"), ("CSRF_SECRET", "csrf-secret")]);

        let cfg = ApiConfig::from_env_config(&env, "auth_token");

        let cloned = cfg.clone();

        assert_eq!(cloned.auth.jwt_secret(), Some("jwt-secret"));
        assert!(cloned.enable_csrf);
        assert_eq!(cloned.graphql_auth.jwt_cookie_name, "auth_token");
    }

    #[test]
    fn debug_output_does_not_expose_jwt_secret() {
        let env = EnvConfig::from_pairs([("JWT_SECRET", "super-secret-jwt-value")]);

        let cfg = ApiConfig::from_env_config(&env, "auth_token");

        let debug = format!("{cfg:?}");

        assert!(debug.contains("ApiConfig"));
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("super-secret-jwt-value"));
    }

    #[test]
    fn debug_output_does_not_expose_csrf_secret() {
        let env = EnvConfig::from_pairs([("CSRF_SECRET", "super-secret-csrf-value")]);

        let cfg = ApiConfig::from_env_config(&env, "auth_token");

        let csrf_secret = cfg.csrf.secret;
        let debug = format!("{cfg:?}");

        assert!(debug.contains("ApiConfig"));
        assert!(debug.contains("CsrfConfig"));
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains(&format!("{csrf_secret:?}")));
    }
}
