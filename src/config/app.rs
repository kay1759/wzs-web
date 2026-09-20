//! # Application Configuration Loader
//!
//! Provides a unified configuration loader for application-wide settings
//! and independently configured API entry points.
//!
//! The process environment is captured into [`EnvConfig`] once during
//! application startup. Typed configuration structures are then built from
//! that same immutable environment snapshot.
//!
//! Applications may expose multiple API entry points, such as public and
//! administrative GraphQL APIs. API-specific configuration is loaded from
//! `PUBLIC_*` and `ADMIN_*` environment variables.
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
//! 5. Builds public and administrative API configuration from prefixed
//!    environment views.
//!
//! Application-specific variables that are not known to `wzs-web` remain
//! available through [`AppConfig::env`].

use std::env;

use crate::config::{
    api::ApiConfig, db::DbConfig, env::EnvConfig, image::ImageConfig, mail::MailConfig,
    upload::UploadConfig, web::HttpConfig,
};

/// Default JWT cookie name for the public API.
const DEFAULT_PUBLIC_JWT_COOKIE_NAME: &str = "public_token";

/// Default JWT cookie name for the administrative API.
const DEFAULT_ADMIN_JWT_COOKIE_NAME: &str = "admin_token";

/// Top-level application configuration.
///
/// `AppConfig` contains:
///
/// - application-wide typed configuration,
/// - independently configured public and administrative APIs, and
/// - the complete [`EnvConfig`] snapshot used to construct them.
///
/// API-specific configuration is read from prefixed environment variables:
///
/// - `PUBLIC_*` for [`AppConfig::public_api`]
/// - `ADMIN_*` for [`AppConfig::admin_api`]
#[derive(Clone, Debug)]
pub struct AppConfig {
    /// Snapshot of all environment variables captured at startup.
    ///
    /// Application-specific settings that are not known by `wzs-web`
    /// remain available through this snapshot.
    pub env: EnvConfig,

    /// Database configuration shared by the application.
    pub db: DbConfig,

    /// HTTP server configuration shared by all API entry points.
    pub http: HttpConfig,

    /// Image processing configuration.
    pub image: ImageConfig,

    /// Upload directory configuration.
    pub upload: UploadConfig,

    /// Optional SMTP configuration.
    pub mail: Option<MailConfig>,

    /// Configuration for the public API.
    ///
    /// Values are loaded from `PUBLIC_*` environment variables.
    pub public_api: ApiConfig,

    /// Configuration for the administrative API.
    ///
    /// Values are loaded from `ADMIN_*` environment variables.
    pub admin_api: ApiConfig,

    /// Whether the GraphiQL IDE is enabled.
    pub enable_graphiql: bool,

    /// Path to the HTML template file.
    ///
    /// Empty when `HTML_PATH` is not configured.
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
    /// Public and administrative API settings are constructed from
    /// `PUBLIC_*` and `ADMIN_*` environment variables respectively.
    ///
    /// Unknown and application-specific values remain available through
    /// [`AppConfig::env`].
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

        let public_jwt_cookie_name = env
            .get_string("PUBLIC_JWT_COOKIE_NAME")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_PUBLIC_JWT_COOKIE_NAME.to_string());

        let admin_jwt_cookie_name = env
            .get_string("ADMIN_JWT_COOKIE_NAME")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_ADMIN_JWT_COOKIE_NAME.to_string());

        let public_env = env.with_prefix("PUBLIC_");
        let admin_env = env.with_prefix("ADMIN_");

        let public_api = ApiConfig::from_env_config(&public_env, public_jwt_cookie_name);

        let admin_api = ApiConfig::from_env_config(&admin_env, admin_jwt_cookie_name);

        Self {
            env,
            db,
            http,
            image,
            upload,
            mail,
            public_api,
            admin_api,
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
        let env = EnvConfig::from_iter([("DATABASE_URL", "mysql://root:pass@localhost/db")]);

        let cfg = AppConfig::from_env_config(env);

        assert_eq!(
            cfg.db.url.as_deref(),
            Some("mysql://root:pass@localhost/db")
        );
    }

    #[test]
    fn from_env_config_preserves_unknown_variables() {
        let env = EnvConfig::from_iter([
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

        assert!(!cfg.public_api.cors.enabled);
        assert!(!cfg.public_api.enable_csrf);
        assert!(!cfg.public_api.auth.is_enabled());

        assert!(!cfg.admin_api.cors.enabled);
        assert!(!cfg.admin_api.enable_csrf);
        assert!(!cfg.admin_api.auth.is_enabled());

        assert_eq!(
            cfg.public_api.graphql_auth.jwt_cookie_name,
            DEFAULT_PUBLIC_JWT_COOKIE_NAME
        );

        assert_eq!(
            cfg.admin_api.graphql_auth.jwt_cookie_name,
            DEFAULT_ADMIN_JWT_COOKIE_NAME
        );
    }

    #[test]
    fn application_wide_fields_are_loaded() {
        let env = EnvConfig::from_iter([
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
    fn public_api_is_loaded_from_public_prefix() {
        let env = EnvConfig::from_iter([
            ("PUBLIC_CORS_ENABLED", "true"),
            ("PUBLIC_CORS_ORIGINS", "https://public.example.com"),
            ("PUBLIC_CORS_CREDENTIALS", "true"),
            ("PUBLIC_CSRF_SECRET", "public-csrf-secret"),
            ("PUBLIC_JWT_SECRET", "public-jwt-secret"),
        ]);

        let cfg = AppConfig::from_env_config(env);

        assert!(cfg.public_api.cors.enabled);

        assert_eq!(cfg.public_api.cors.env, "https://public.example.com");

        assert!(cfg.public_api.cors.credentials);
        assert!(cfg.public_api.enable_csrf);

        assert!(cfg.public_api.auth.is_enabled());

        assert_eq!(cfg.public_api.auth.jwt_secret(), Some("public-jwt-secret"));
    }

    #[test]
    fn admin_api_is_loaded_from_admin_prefix() {
        let env = EnvConfig::from_iter([
            ("ADMIN_CORS_ENABLED", "true"),
            ("ADMIN_CORS_ORIGINS", "https://admin.example.com"),
            ("ADMIN_CORS_CREDENTIALS", "true"),
            ("ADMIN_CSRF_SECRET", "admin-csrf-secret"),
            ("ADMIN_JWT_SECRET", "admin-jwt-secret"),
        ]);

        let cfg = AppConfig::from_env_config(env);

        assert!(cfg.admin_api.cors.enabled);

        assert_eq!(cfg.admin_api.cors.env, "https://admin.example.com");

        assert!(cfg.admin_api.cors.credentials);
        assert!(cfg.admin_api.enable_csrf);

        assert!(cfg.admin_api.auth.is_enabled());

        assert_eq!(cfg.admin_api.auth.jwt_secret(), Some("admin-jwt-secret"));
    }

    #[test]
    fn public_and_admin_api_configs_are_independent() {
        let env = EnvConfig::from_iter([
            ("PUBLIC_CORS_ENABLED", "true"),
            ("PUBLIC_JWT_SECRET", "public-secret"),
            ("PUBLIC_CSRF_SECRET", "public-csrf"),
            ("ADMIN_CORS_ENABLED", "false"),
            ("ADMIN_JWT_SECRET", "admin-secret"),
        ]);

        let cfg = AppConfig::from_env_config(env);

        assert!(cfg.public_api.cors.enabled);
        assert!(cfg.public_api.enable_csrf);

        assert_eq!(cfg.public_api.auth.jwt_secret(), Some("public-secret"));

        assert!(!cfg.admin_api.cors.enabled);
        assert!(!cfg.admin_api.enable_csrf);

        assert_eq!(cfg.admin_api.auth.jwt_secret(), Some("admin-secret"));
    }

    #[test]
    fn unprefixed_api_settings_are_not_used() {
        let env = EnvConfig::from_iter([
            ("CORS_ENABLED", "true"),
            ("CSRF_SECRET", "legacy-csrf-secret"),
            ("JWT_SECRET", "legacy-jwt-secret"),
        ]);

        let cfg = AppConfig::from_env_config(env);

        assert!(!cfg.public_api.cors.enabled);
        assert!(!cfg.public_api.enable_csrf);
        assert!(!cfg.public_api.auth.is_enabled());

        assert!(!cfg.admin_api.cors.enabled);
        assert!(!cfg.admin_api.enable_csrf);
        assert!(!cfg.admin_api.auth.is_enabled());
    }

    #[test]
    fn public_and_admin_cookie_names_use_defaults() {
        let cfg = AppConfig::from_env_config(EnvConfig::default());

        assert_eq!(cfg.public_api.graphql_auth.jwt_cookie_name, "public_token");

        assert_eq!(cfg.admin_api.graphql_auth.jwt_cookie_name, "admin_token");
    }

    #[test]
    fn public_and_admin_cookie_names_can_be_overridden() {
        let env = EnvConfig::from_iter([
            ("PUBLIC_JWT_COOKIE_NAME", "my_public_token"),
            ("ADMIN_JWT_COOKIE_NAME", "my_admin_token"),
        ]);

        let cfg = AppConfig::from_env_config(env);

        assert_eq!(
            cfg.public_api.graphql_auth.jwt_cookie_name,
            "my_public_token"
        );

        assert_eq!(cfg.admin_api.graphql_auth.jwt_cookie_name, "my_admin_token");
    }

    #[test]
    fn empty_cookie_names_use_defaults() {
        for value in ["", " ", "   ", "\t", "\n"] {
            let env = EnvConfig::from_iter([
                ("PUBLIC_JWT_COOKIE_NAME", value),
                ("ADMIN_JWT_COOKIE_NAME", value),
            ]);

            let cfg = AppConfig::from_env_config(env);

            assert_eq!(
                cfg.public_api.graphql_auth.jwt_cookie_name,
                DEFAULT_PUBLIC_JWT_COOKIE_NAME
            );

            assert_eq!(
                cfg.admin_api.graphql_auth.jwt_cookie_name,
                DEFAULT_ADMIN_JWT_COOKIE_NAME
            );
        }
    }

    #[test]
    fn public_authentication_is_disabled_when_secret_is_empty() {
        for secret in ["", " ", "   ", "\t", "\n"] {
            let env = EnvConfig::from_iter([("PUBLIC_JWT_SECRET", secret)]);

            let cfg = AppConfig::from_env_config(env);

            assert!(
                !cfg.public_api.auth.is_enabled(),
                "Expected public authentication to be disabled for {secret:?}"
            );
        }
    }

    #[test]
    fn admin_authentication_is_disabled_when_secret_is_empty() {
        for secret in ["", " ", "   ", "\t", "\n"] {
            let env = EnvConfig::from_iter([("ADMIN_JWT_SECRET", secret)]);

            let cfg = AppConfig::from_env_config(env);

            assert!(
                !cfg.admin_api.auth.is_enabled(),
                "Expected admin authentication to be disabled for {secret:?}"
            );
        }
    }

    #[test]
    fn public_csrf_is_disabled_when_secret_is_empty() {
        for secret in ["", " ", "   ", "\t", "\n"] {
            let env = EnvConfig::from_iter([("PUBLIC_CSRF_SECRET", secret)]);

            let cfg = AppConfig::from_env_config(env);

            assert!(
                !cfg.public_api.enable_csrf,
                "Expected public CSRF to be disabled for {secret:?}"
            );
        }
    }

    #[test]
    fn admin_csrf_is_disabled_when_secret_is_empty() {
        for secret in ["", " ", "   ", "\t", "\n"] {
            let env = EnvConfig::from_iter([("ADMIN_CSRF_SECRET", secret)]);

            let cfg = AppConfig::from_env_config(env);

            assert!(
                !cfg.admin_api.enable_csrf,
                "Expected admin CSRF to be disabled for {secret:?}"
            );
        }
    }

    #[test]
    fn http_body_size_falls_back_to_mb_when_bytes_absent() {
        let env = EnvConfig::from_iter([("HTTP_MAX_BODY_MB", "7")]);

        let cfg = AppConfig::from_env_config(env);

        assert_eq!(cfg.http.max_body_bytes, 7 * 1024 * 1024);
    }

    #[test]
    fn malformed_numbers_use_defaults_where_applicable() {
        let env = EnvConfig::from_iter([
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
        let env = EnvConfig::from_iter([("HTML_PATH", "/tmp/index.html")]);

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
        let env = EnvConfig::from_iter([("SMTP_HOST", "smtp.example.com")]);

        let cfg = AppConfig::from_env_config(env);

        assert!(cfg.mail.is_none());
    }

    #[test]
    fn mail_config_is_loaded_when_all_required_smtp_vars_are_present() {
        let env = EnvConfig::from_iter([
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
        let env = EnvConfig::from_iter([
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
        let env = EnvConfig::from_iter([
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
            vec!["notify1@example.com", "notify2@example.com",]
        );
    }

    #[test]
    fn graphiql_accepts_truthy_values() {
        for value in ["1", "true", "TRUE", "yes", "on"] {
            let env = EnvConfig::from_iter([("GRAPHIQL", value)]);

            let cfg = AppConfig::from_env_config(env);

            assert!(cfg.enable_graphiql, "Expected {value:?} to enable GraphiQL");
        }
    }

    #[test]
    fn graphiql_defaults_to_false_for_invalid_value() {
        let env = EnvConfig::from_iter([("GRAPHIQL", "invalid")]);

        let cfg = AppConfig::from_env_config(env);

        assert!(!cfg.enable_graphiql);
    }
}
