//! # Application Configuration Loader
//!
//! Loads environment-based application settings, including database configuration.
//!
//! Automatically reads `.env` files for non-production environments.
//! It first checks for a custom `DOTENV_FILE` path, then falls back to
//! `.env.{APP_ENV}` or `.env`.
//!
//! Typically used at application startup to construct a global [`AppConfig`].
//!
//! # Examples
//! ```rust,no_run
//! use wzs_web::config::app::AppConfig;
//!
//! let cfg = AppConfig::from_env();
//! println!("Database URL: {:?}", cfg.db.url);
//! ```

use std::env;

use crate::config::db::DbConfig;

/// Top-level application configuration.
///
/// Includes database configuration (`DbConfig`) and manages
/// environment file loading.
#[derive(Clone, Debug)]
pub struct AppConfig {
    pub db: DbConfig,
}

impl AppConfig {
    /// Loads configuration from environment variables.
    ///
    /// Behavior:
    /// - Reads `APP_ENV` (defaults to `"development"`).
    /// - If not in `"production"`, attempts to load environment variables from:
    ///   1. `DOTENV_FILE` (if set)
    ///   2. `.env.{APP_ENV}` or fallback `.env`
    /// - Loads [`DbConfig`] from the resulting environment.
    ///
    /// # Example
    /// ```rust,no_run
    /// use wzs_web::config::app::AppConfig;
    ///
    /// let cfg = AppConfig::from_env();
    /// assert!(cfg.db.is_valid());
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

        AppConfig {
            db: DbConfig::from_env(),
        }
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
}
