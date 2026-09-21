//! # API Configuration
//!
//! Provides configuration shared by an individual application API.
//!
//! An application may expose multiple API entry points with different
//! security and transport policies. For example:
//!
//! ```text
//! /public/graphql
//! /admin/graphql
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
/// `ApiConfig` groups settings that may differ between APIs exposed by
/// the same application.
///
/// For example, public and administrative APIs may use different CORS,
/// CSRF, authentication, and authentication-cookie policies.
#[derive(Clone, Debug)]
pub struct ApiConfig {
    /// Cross-Origin Resource Sharing configuration.
    pub cors: CorsConfig,

    /// Whether CSRF protection is enabled.
    ///
    /// CSRF protection is enabled when `CSRF_SECRET` contains a
    /// non-empty value.
    pub enable_csrf: bool,

    /// CSRF signing and cookie configuration.
    pub csrf: CsrfConfig,

    /// Application authentication configuration.
    pub auth: AuthConfig,

    /// GraphQL authentication transport configuration.
    ///
    /// This currently defines the cookie name used to obtain the JWT
    /// payload from GraphQL requests.
    pub graphql_auth: GraphqlAuthConfig,
}

impl ApiConfig {
    /// Builds API configuration from an [`EnvConfig`] snapshot.
    ///
    /// This constructor currently reads the existing unprefixed
    /// configuration variables.
    ///
    /// Prefix support such as `PUBLIC_*` and `ADMIN_*` will be introduced
    /// separately so that the migration can remain incremental.
    pub fn from_env_config(env: &EnvConfig, jwt_cookie_name: impl Into<String>) -> Self {
        let cors = CorsConfig::from_env_config(env);

        let enable_csrf = env
            .get("CSRF_SECRET")
            .is_some_and(|value| !value.trim().is_empty());

        let csrf = CsrfConfig::from_env_config(env);
        let auth = AuthConfig::from_env_config(env);

        let graphql_auth = GraphqlAuthConfig::new(jwt_cookie_name);

        Self {
            cors,
            enable_csrf,
            csrf,
            auth,
            graphql_auth,
        }
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
    fn stores_graphql_authentication_cookie_name() {
        let env = EnvConfig::default();

        let cfg = ApiConfig::from_env_config(&env, "wizis_token");

        assert_eq!(cfg.graphql_auth.jwt_cookie_name, "wizis_token");
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
}
