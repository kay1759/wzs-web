//! # Application Configuration Loader
//!
//! Provides a unified configuration loader for application settings,
//! including database, HTTP, CORS, CSRF, and feature toggles.
//!
//! Automatically loads `.env` files for non-production environments.
//! It checks for a custom `DOTENV_FILE` path first, then falls back to
//! `.env.{APP_ENV}` or `.env`.
//!
//! This configuration is typically initialized once at application startup
//! and shared throughout the system.
//!
//! # Environment Variables
//! | Variable | Description | Default |
//! |-----------|-------------|----------|
//! | `APP_ENV` | Current environment (`development`, `production`, etc.) | `"development"` |
//! | `DOTENV_FILE` | Optional path to a custom dotenv file | *none* |
//! | `DATABASE_URL` | MySQL connection URL | *required* |
//! | `HTTP_MAX_BODY_BYTES` | Maximum request body size (bytes) | derived from `HTTP_MAX_BODY_MB` |
//! | `HTTP_MAX_BODY_MB` | Max body size in megabytes (if bytes not set) | `5` |
//! | `CSRF_SECRET` | CSRF signing secret (auto-generated if missing) | random |
//! | `GRAPHIQL` | Enable GraphiQL IDE | `false` |
//! | `CORS_ORIGINS` | Allowed origins for CORS | `""` |
//! | `CORS_CREDENTIALS` | Allow cookies/headers in CORS requests | `false` |
//!
//! # Example
//! ```rust,no_run
//! use wzs_web::config::app::AppConfig;
//!
//! let cfg = AppConfig::from_env();
//! if cfg.is_csrf_enabled() {
//!     println!("CSRF protection is active");
//! }
//! ```

use std::env;

use crate::config::{
    csrf::CsrfConfig,
    db::DbConfig,
    env::*,
    web::{CorsConfig, HttpConfig},
};

/// Top-level application configuration.
#[derive(Clone, Debug)]
pub struct AppConfig {
    /// Database configuration.
    pub db: DbConfig,
    /// HTTP server configuration.
    pub http: HttpConfig,
    /// CSRF-related secret and cookie flags.
    pub csrf: CsrfConfig,
    /// Cross-Origin Resource Sharing configuration.
    pub cors: CorsConfig,
    /// Whether the GraphiQL IDE is enabled (for development use).
    pub enable_graphiql: bool,
}

impl AppConfig {
    /// Loads application configuration from environment variables.
    ///
    /// ## Behavior
    /// - Reads `APP_ENV` (defaults to `"development"`).
    /// - Loads `.env` or `.env.{APP_ENV}` for non-production environments.
    /// - Parses all supported environment variables and falls back to defaults.
    ///
    /// # Example
    /// ```rust,no_run
    /// use wzs_web::config::app::AppConfig;
    ///
    /// let cfg = AppConfig::from_env();
    /// assert!(cfg.db.is_valid());
    /// assert!(cfg.http.max_body_bytes > 0);
    /// ```
    pub fn from_env() -> Self {
        let app_env = env::var("APP_ENV").unwrap_or_else(|_| "development".into());

        if app_env != "production" {
            if let Ok(path) = env::var("DOTENV_FILE") {
                let _ = dotenvy::from_filename(path);
            } else {
                let candidate = format!(".env.{}", app_env);
                dotenvy::from_filename(&candidate)
                    .or_else(|_| dotenvy::dotenv())
                    .ok();
            }
        }

        // HTTP configuration
        let http_max_body_bytes = env::var("HTTP_MAX_BODY_BYTES")
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or_else(|| (read_u32("HTTP_MAX_BODY_MB", 5) as usize) * 1024 * 1024);

        // CORS
        let cors_env = env::var("CORS_ORIGINS").unwrap_or_default();
        let cors_credentials = read_flag("CORS_CREDENTIALS", false);

        let enable_graphiql = read_flag("GRAPHIQL", false);

        AppConfig {
            db: DbConfig::from_env(),
            http: HttpConfig {
                max_body_bytes: http_max_body_bytes,
            },
            csrf: CsrfConfig::from_env(),
            cors: CorsConfig {
                env: cors_env,
                credentials: cors_credentials,
            },
            enable_graphiql,
        }
    }

    /// Returns `true` if CSRF protection is enabled.
    ///
    /// This is automatically determined by the presence of `CSRF_SECRET`.
    pub fn is_csrf_enabled(&self) -> bool {
        self.csrf.is_enabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temp_env;

    #[test]
    fn from_env_includes_db_config() {
        temp_env::with_vars(
            vec![("DATABASE_URL", Some("mysql://root:pass@localhost/db"))],
            || {
                let cfg = AppConfig::from_env();
                assert_eq!(
                    cfg.db.url.as_deref(),
                    Some("mysql://root:pass@localhost/db")
                );
            },
        );
    }

    #[test]
    fn is_csrf_enabled_returns_true_when_secret_is_present() {
        temp_env::with_vars(vec![("CSRF_SECRET", Some("super-secret-key"))], || {
            let cfg = AppConfig::from_env();
            assert!(
                cfg.is_csrf_enabled(),
                "Expected CSRF to be enabled when CSRF_SECRET is set"
            );
        });
    }

    #[test]
    fn is_csrf_enabled_returns_false_when_secret_is_missing() {
        temp_env::with_vars(vec![("CSRF_SECRET", None::<&str>)], || {
            let cfg = AppConfig::from_env();
            assert!(
                !cfg.is_csrf_enabled(),
                "Expected CSRF to be disabled when CSRF_SECRET is missing"
            );
        });
    }
}
