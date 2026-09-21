//! # Application Configuration Loader
//!
//! Provides a unified configuration loader for application-wide settings.
//!
//! The process environment is captured into [`EnvConfig`] once during
//! application startup. Typed application-wide configuration structures are
//! then built from that same immutable environment snapshot.
//!
//! API-specific configuration is intentionally not constructed by
//! [`AppConfig`]. Applications may create any number of API configurations
//! from [`AppConfig::env`] using `ApiConfig`.
//!
//! For non-production environments, `.env` files are loaded before the
//! snapshot is created.
//!
//! # Environment Loading
//!
//! [`AppConfig::from_env`] performs the following steps:
//!
//! 1. Reads `APP_ENV` from the process environment.
//! 2. Loads an appropriate `.env` file when not in production.
//! 3. Captures all environment variables into [`EnvConfig`].
//! 4. Builds application-wide configuration from that snapshot.
//!
//! Application-specific and API-specific variables that are not directly
//! interpreted by `wzs-web` remain available through [`AppConfig::env`].

use std::env;

use crate::config::{
    db::DbConfig, env::EnvConfig, image::ImageConfig, mail::MailConfig, upload::UploadConfig,
    web::HttpConfig,
};

/// Top-level application configuration.
///
/// `AppConfig` contains application-wide typed configuration and the complete
/// [`EnvConfig`] snapshot captured at startup.
///
/// API configuration is deliberately not stored here. Applications can create
/// any number of independent `ApiConfig` values from [`AppConfig::env`].
#[derive(Clone, Debug)]
pub struct AppConfig {
    /// Snapshot of all environment variables captured at startup.
    ///
    /// Application-specific and API-specific settings that are not directly
    /// interpreted by `wzs-web` remain available through this snapshot.
    pub env: EnvConfig,

    /// Database configuration shared by the application.
    pub db: DbConfig,

    /// HTTP server configuration shared by the application.
    pub http: HttpConfig,

    /// Image processing configuration.
    pub image: ImageConfig,

    /// Upload directory configuration.
    pub upload: UploadConfig,

    /// Optional SMTP configuration.
    pub mail: Option<MailConfig>,

    /// Whether the GraphiQL IDE is enabled.
    pub enable_graphiql: bool,

    /// Path to the HTML template file.
    ///
    /// Empty when `HTML_PATH` is not configured.
    ///
    /// This field retains the existing `HTML_PATH` behavior for backward
    /// compatibility.
    pub html_path: String,
}

impl AppConfig {
    /// Loads application configuration from the process environment.
    ///
    /// In non-production environments, this method first attempts to load a
    /// dotenv file. The complete process environment is then captured into an
    /// [`EnvConfig`] snapshot and passed to [`AppConfig::from_env_config`].
    ///
    /// # Dotenv loading order
    ///
    /// When `APP_ENV` is not `"production"`:
    ///
    /// 1. `DOTENV_FILE`, when explicitly configured;
    /// 2. `.env.{APP_ENV}`;
    /// 3. `.env`.
    pub fn from_env() -> Self {
        load_dotenv();

        let env = EnvConfig::from_env();

        Self::from_env_config(env)
    }

    /// Builds application configuration from an [`EnvConfig`] snapshot.
    ///
    /// This constructor does not access or modify the process environment.
    ///
    /// API-specific configuration is not constructed here. The complete
    /// environment snapshot remains available through [`AppConfig::env`] so
    /// applications can construct any number of independently configured APIs.
    pub fn from_env_config(env: EnvConfig) -> Self {
        let db = DbConfig::from_env_config(&env);
        let http = HttpConfig::from_env_config(&env);
        let image = ImageConfig::from_env_config(&env);
        let upload = UploadConfig::from_env_config(&env);

        let mail = if env.contains_key("SMTP_HOST") {
            MailConfig::from_env_config(&env).ok()
        } else {
            None
        };

        let enable_graphiql = read_flag(&env, "GRAPHIQL", false);

        let html_path = env.get_string("HTML_PATH").unwrap_or_default();

        Self {
            env,
            db,
            http,
            image,
            upload,
            mail,
            enable_graphiql,
            html_path,
        }
    }
}

/// Loads an appropriate dotenv file for the current environment.
///
/// `APP_ENV` and `DOTENV_FILE` must be read before the environment snapshot is
/// created because loading a dotenv file modifies the process environment.
fn load_dotenv() {
    let app_env = env::var("APP_ENV").unwrap_or_else(|_| "development".into());

    if app_env == "production" {
        return;
    }

    if let Ok(path) = env::var("DOTENV_FILE") {
        let _ = dotenvy::from_filename(path);
        return;
    }

    let candidate = format!(".env.{app_env}");

    dotenvy::from_filename(&candidate)
        .or_else(|_| dotenvy::dotenv())
        .ok();
}

/// Reads a boolean flag while preserving the existing configuration behavior.
fn read_flag(env: &EnvConfig, key: &str, default: bool) -> bool {
    match env.get(key) {
        Some(value) => is_truthy(value),
        None => default,
    }
}

/// Returns whether a string represents a truthy configuration value.
fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn from_env_config_includes_db_config() {
        let env = EnvConfig::from_pairs([("DATABASE_URL", "mysql://root:pass@localhost/db")]);

        let cfg = AppConfig::from_env_config(env);

        assert_eq!(
            cfg.db.url.as_deref(),
            Some("mysql://root:pass@localhost/db")
        );
    }

    #[test]
    fn from_env_config_preserves_unknown_variables() {
        let env = EnvConfig::from_pairs([
            ("DATABASE_URL", "mysql://localhost/db"),
            ("BCRYPT_COST", "4"),
            ("PUBLIC_WEB_BASE_URL", "http://localhost:5173"),
            ("RESERVATION_MAX_DAYS", "60"),
        ]);

        let cfg = AppConfig::from_env_config(env);

        assert_eq!(cfg.env.get_u32("BCRYPT_COST"), Some(4));

        assert_eq!(
            cfg.env.get("PUBLIC_WEB_BASE_URL"),
            Some("http://localhost:5173")
        );

        assert_eq!(cfg.env.get_u32("RESERVATION_MAX_DAYS"), Some(60));
    }

    #[test]
    fn from_env_config_preserves_arbitrary_api_variables() {
        let env = EnvConfig::from_pairs([
            ("PUBLIC_JWT_SECRET", "public-secret"),
            ("ADMIN_JWT_SECRET", "admin-secret"),
            ("PICKUP_JWT_SECRET", "pickup-secret"),
        ]);

        let cfg = AppConfig::from_env_config(env);

        assert_eq!(cfg.env.get("PUBLIC_JWT_SECRET"), Some("public-secret"));

        assert_eq!(cfg.env.get("ADMIN_JWT_SECRET"), Some("admin-secret"));

        assert_eq!(cfg.env.get("PICKUP_JWT_SECRET"), Some("pickup-secret"));
    }

    #[test]
    fn from_env_config_uses_defaults_when_optional_values_are_missing() {
        let cfg = AppConfig::from_env_config(EnvConfig::default());

        assert!(!cfg.enable_graphiql);

        assert_eq!(cfg.image.max_width, 1280);
        assert_eq!(cfg.image.max_height, 1280);

        assert_eq!(cfg.upload.root, PathBuf::from("./var/uploads"));
        assert_eq!(cfg.upload.image_dir, "images");
        assert_eq!(cfg.upload.file_dir, "files");

        assert_eq!(cfg.http.max_body_bytes, 5 * 1024 * 1024);

        assert_eq!(cfg.html_path, "");
        assert!(cfg.mail.is_none());
    }

    #[test]
    fn application_wide_fields_are_loaded() {
        let env = EnvConfig::from_pairs([
            ("GRAPHIQL", "true"),
            ("UPLOAD_ROOT", "/data/uploads"),
            ("UPLOAD_IMAGE_DIR", "pics"),
            ("UPLOAD_FILE_DIR", "docs"),
            ("IMAGE_MAX_WIDTH", "2048"),
            ("IMAGE_MAX_HEIGHT", "1536"),
            ("HTTP_MAX_BODY_BYTES", "3145728"),
            ("HTTP_MAX_BODY_MB", "99"),
            ("HTML_PATH", "/tmp/index.html"),
        ]);

        let cfg = AppConfig::from_env_config(env);

        assert!(cfg.enable_graphiql);

        assert_eq!(cfg.upload.root, PathBuf::from("/data/uploads"));
        assert_eq!(cfg.upload.image_dir, "pics");
        assert_eq!(cfg.upload.file_dir, "docs");

        assert_eq!(cfg.image.max_width, 2048);
        assert_eq!(cfg.image.max_height, 1536);

        assert_eq!(cfg.http.max_body_bytes, 3 * 1024 * 1024);

        assert_eq!(cfg.html_path, "/tmp/index.html");
    }

    #[test]
    fn http_body_size_falls_back_to_mb_when_bytes_absent() {
        let env = EnvConfig::from_pairs([("HTTP_MAX_BODY_MB", "7")]);

        let cfg = AppConfig::from_env_config(env);

        assert_eq!(cfg.http.max_body_bytes, 7 * 1024 * 1024);
    }

    #[test]
    fn malformed_numbers_use_defaults_where_applicable() {
        let env = EnvConfig::from_pairs([
            ("IMAGE_MAX_WIDTH", "NaN"),
            ("IMAGE_MAX_HEIGHT", "oops"),
            ("HTTP_MAX_BODY_MB", "not-a-number"),
        ]);

        let cfg = AppConfig::from_env_config(env);

        assert_eq!(cfg.image.max_width, 1280);
        assert_eq!(cfg.image.max_height, 1280);

        assert_eq!(cfg.http.max_body_bytes, 5 * 1024 * 1024);
    }

    #[test]
    fn html_path_defaults_to_empty() {
        let cfg = AppConfig::from_env_config(EnvConfig::default());

        assert_eq!(cfg.html_path, "");
    }

    #[test]
    fn html_path_is_loaded_from_env_config() {
        let env = EnvConfig::from_pairs([("HTML_PATH", "/tmp/index.html")]);

        let cfg = AppConfig::from_env_config(env);

        assert_eq!(cfg.html_path, "/tmp/index.html");
    }

    #[test]
    fn mail_config_is_none_when_smtp_is_not_configured() {
        let cfg = AppConfig::from_env_config(EnvConfig::default());

        assert!(cfg.mail.is_none());
    }

    #[test]
    fn mail_config_is_none_when_required_smtp_vars_are_incomplete() {
        let env = EnvConfig::from_pairs([("SMTP_HOST", "smtp.example.com")]);

        let cfg = AppConfig::from_env_config(env);

        assert!(cfg.mail.is_none());
    }

    #[test]
    fn mail_config_is_loaded_when_all_required_smtp_vars_are_present() {
        let env = EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_PORT", "587"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "pass"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
            ("SMTP_FROM_NAME", "Notifier"),
            ("NOTIFY_TO_EMAIL", "notify@example.com"),
        ]);

        let cfg = AppConfig::from_env_config(env);

        let mail = cfg.mail.expect("mail config should be present");

        assert_eq!(mail.host, "smtp.example.com");
        assert_eq!(mail.port, 587);
        assert_eq!(mail.username, "user");
        assert_eq!(mail.password, "pass");
        assert_eq!(mail.from_email, "noreply@example.com");
        assert_eq!(mail.from_name, "Notifier");
        assert_eq!(mail.notify_to, vec!["notify@example.com"]);
    }

    #[test]
    fn mail_config_uses_defaults_for_optional_fields() {
        let env = EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_PORT", "25"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "pass"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
        ]);

        let cfg = AppConfig::from_env_config(env);

        let mail = cfg.mail.expect("mail config should be present");

        assert_eq!(mail.from_name, "Notifier");
        assert!(mail.notify_to.is_empty());
    }

    #[test]
    fn mail_config_supports_multiple_notify_to_addresses() {
        let env = EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_PORT", "587"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "pass"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
            (
                "NOTIFY_TO_EMAIL",
                "notify1@example.com, notify2@example.com",
            ),
        ]);

        let cfg = AppConfig::from_env_config(env);

        let mail = cfg.mail.expect("mail config should be present");

        assert_eq!(
            mail.notify_to,
            vec!["notify1@example.com", "notify2@example.com"]
        );
    }

    #[test]
    fn graphiql_accepts_truthy_values() {
        for value in ["1", "true", "TRUE", "yes", "on"] {
            let env = EnvConfig::from_pairs([("GRAPHIQL", value)]);

            let cfg = AppConfig::from_env_config(env);

            assert!(cfg.enable_graphiql, "Expected {value:?} to enable GraphiQL");
        }
    }

    #[test]
    fn graphiql_defaults_to_false_for_invalid_value() {
        let env = EnvConfig::from_pairs([("GRAPHIQL", "invalid")]);

        let cfg = AppConfig::from_env_config(env);

        assert!(!cfg.enable_graphiql);
    }
}
