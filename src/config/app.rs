//! # Application Configuration Loader
//!
//! Provides a unified configuration loader for application settings,
//! including **database**, **HTTP**, **CORS**, **CSRF**, and **feature toggles**.
//!
//! Automatically loads `.env` files for non-production environments.
//! It checks for a custom `DOTENV_FILE` path first, then falls back to
//! `.env.{APP_ENV}` or `.env`.
//!
//! This configuration is typically initialized once at application startup
//! and shared throughout the entire system via dependency injection.
//!
//! # Environment Variables
//! | Variable | Description | Default |
//! |-----------|-------------|----------|
//! | `APP_ENV` | Current environment (`development`, `production`, etc.) | `"development"` |
//! | `DOTENV_FILE` | Optional path to a custom dotenv file | *none* |
//! | `DATABASE_URL` | MySQL connection URL | *required* |
//! | `JWT_SECRET` | Secret used to sign JWTs | `""` |
//! | `HTML_PATH` | Path to HTML template file | `""` |
//! | `HTTP_MAX_BODY_BYTES` | Maximum request body size (bytes) | derived from `HTTP_MAX_BODY_MB` |
//! | `HTTP_MAX_BODY_MB` | Max body size in megabytes (if bytes not set) | `5` |
//! | `CSRF_SECRET` | CSRF signing secret (auto-generated if missing) | random |
//! | `GRAPHIQL` | Enable GraphiQL IDE (development only) | `false` |
//! | `CORS_ORIGINS` | Allowed origins for CORS | `""` |
//! | `CORS_CREDENTIALS` | Allow credentials in CORS requests | `false` |
//! | `UPLOAD_ROOT` | Root directory for uploads | `"./var/uploads"` |
//! | `UPLOAD_IMAGE_DIR` | Subdirectory for image uploads | `"images"` |
//! | `UPLOAD_FILE_DIR` | Subdirectory for other file uploads | `"files"` |
//! | `IMAGE_MAX_WIDTH` | Max allowed image width (px) | `1280` |
//! | `IMAGE_MAX_HEIGHT` | Max allowed image height (px) | `1280` |
//! | `SMTP_HOST` | SMTP server hostname | *none* |
//! | `SMTP_PORT` | SMTP server port | *none* |
//! | `SMTP_USERNAME` | SMTP authentication username | *none* |
//! | `SMTP_PASSWORD` | SMTP authentication password | *none* |
//! | `SMTP_FROM_EMAIL` | Sender email address | *none* |
//! | `SMTP_FROM_NAME` | Sender display name | `"Notifier"` |
//! | `NOTIFY_TO_EMAIL` | Notification recipients (comma-separated) | empty |
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
use std::path::PathBuf;

use crate::config::{
    csrf::CsrfConfig,
    db::DbConfig,
    env::*,
    image::ImageConfig,
    mail::MailConfig,
    upload::UploadConfig,
    web::{CorsConfig, HttpConfig},
};

/// Top-level application configuration.
///
/// This struct aggregates all configuration domains:
/// - **Database connection**
/// - **HTTP server and request limits**
/// - **CSRF & CORS security**
/// - **Image processing & upload directories**
/// - **Feature flags (e.g. GraphiQL enablement)**
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
    /// Image dimension constraints.
    pub image: ImageConfig,
    /// File and image upload directory configuration.
    pub upload: UploadConfig,
    /// Optional mail (SMTP) configuration.
    pub mail: Option<MailConfig>,
    /// Whether the GraphiQL IDE is enabled (typically only in development).
    pub enable_graphiql: bool,
    /// JWT signing secret.
    ///
    /// - Empty string if `JWT_SECRET` is not set.
    /// - Validation is responsibility of the caller.
    pub jwt_secret: String,
    /// Path to the HTML template file.
    ///
    /// - Empty string if `HTML_PATH` is not set.
    /// - File loading is responsibility of the caller.
    pub html_path: String,
}

impl AppConfig {
    /// Loads application configuration from environment variables and optional `.env` files.
    ///
    /// ## Behavior
    /// - Reads `APP_ENV` (defaults to `"development"`).
    /// - If not in production, attempts to load:
    ///   1. `DOTENV_FILE` (if defined), or
    ///   2. `.env.{APP_ENV}`, or
    ///   3. fallback to `.env`.
    /// - Parses known environment variables into structured configuration.
    /// - Falls back to safe defaults for optional parameters.
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
        // Determine environment (e.g., development, production)
        let app_env = env::var("APP_ENV").unwrap_or_else(|_| "development".into());

        // Automatically load .env file for non-production environments
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

        // --- Image configuration ---
        let max_w = read_u32("IMAGE_MAX_WIDTH", 1280);
        let max_h = read_u32("IMAGE_MAX_HEIGHT", 1280);

        // --- Upload configuration ---
        let upload_root: PathBuf = env::var("UPLOAD_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| "./var/uploads".into());
        let image_dir = env::var("UPLOAD_IMAGE_DIR").unwrap_or_else(|_| "images".into());
        let file_dir = env::var("UPLOAD_FILE_DIR").unwrap_or_else(|_| "files".into());

        // --- Mail configuration (optional) ---
        //
        // Mail configuration is enabled only when SMTP_HOST is present.
        // If any required SMTP variables are missing or invalid,
        // MailConfig::from_env() returns an error and mail config is disabled.
        let mail = if env::var("SMTP_HOST").is_ok() {
            MailConfig::from_env().ok()
        } else {
            None
        };

        let enable_graphiql = read_flag("GRAPHIQL", false);

        // JWT & HTML
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "".to_string());
        let html_path = env::var("HTML_PATH").unwrap_or_else(|_| "".to_string());

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
            image: ImageConfig {
                max_width: max_w,
                max_height: max_h,
            },
            upload: UploadConfig {
                root: upload_root,
                image_dir,
                file_dir,
            },
            mail,
            enable_graphiql,
            jwt_secret,
            html_path,
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

    #[test]
    fn from_env_uses_defaults_when_missing_optional() {
        let vars = vec![
            ("APP_ENV", Some("production")),
            ("GRAPHIQL", None),
            ("CORS_ORIGINS", None),
            ("CORS_CREDENTIALS", None),
            ("IMAGE_MAX_WIDTH", None),
            ("IMAGE_MAX_HEIGHT", None),
            ("HTTP_MAX_BODY_BYTES", None),
            ("HTTP_MAX_BODY_MB", None),
            ("UPLOAD_ROOT", None),
            ("UPLOAD_IMAGE_DIR", None),
            ("UPLOAD_FILE_DIR", None),
        ];

        temp_env::with_vars(vars, || {
            let cfg = AppConfig::from_env();

            assert!(!cfg.enable_graphiql);

            assert_eq!(cfg.image.max_width, 1280);
            assert_eq!(cfg.image.max_height, 1280);

            assert_eq!(cfg.upload.root, PathBuf::from("./var/uploads"));
            assert_eq!(cfg.upload.image_dir, "images");
            assert_eq!(cfg.upload.file_dir, "files");

            assert_eq!(cfg.cors.env, "");
            assert_eq!(cfg.cors.credentials, false);

            assert_eq!(cfg.http.max_body_bytes, 5 * 1024 * 1024);
        });
    }

    #[test]
    fn from_env_overrides_all_fields() {
        let vars = vec![
            ("APP_ENV", Some("production")),
            ("GRAPHIQL", Some("true")),
            ("UPLOAD_ROOT", Some("/data/uploads")),
            ("UPLOAD_IMAGE_DIR", Some("pics")),
            ("UPLOAD_FILE_DIR", Some("docs")),
            (
                "CORS_ORIGINS",
                Some("https://a.example.com,https://b.example.com"),
            ),
            ("CORS_CREDENTIALS", Some("true")),
            ("IMAGE_MAX_WIDTH", Some("2048")),
            ("IMAGE_MAX_HEIGHT", Some("1536")),
            ("HTTP_MAX_BODY_BYTES", Some("3145728")),
            ("HTTP_MAX_BODY_MB", Some("99")),
        ];

        temp_env::with_vars(vars, || {
            let cfg = AppConfig::from_env();

            assert!(cfg.enable_graphiql);

            assert_eq!(cfg.upload.root, PathBuf::from("/data/uploads"));
            assert_eq!(cfg.upload.image_dir, "pics");
            assert_eq!(cfg.upload.file_dir, "docs");

            assert_eq!(cfg.cors.env, "https://a.example.com,https://b.example.com");
            assert!(cfg.cors.credentials);

            assert_eq!(cfg.image.max_width, 2048);
            assert_eq!(cfg.image.max_height, 1536);

            assert_eq!(cfg.http.max_body_bytes, 3 * 1024 * 1024);
        });
    }

    #[test]
    fn http_body_size_falls_back_to_mb_when_bytes_absent() {
        let vars = vec![
            ("APP_ENV", Some("production")),
            ("HTTP_MAX_BODY_BYTES", None),
            ("HTTP_MAX_BODY_MB", Some("7")),
        ];

        temp_env::with_vars(vars, || {
            let cfg = AppConfig::from_env();
            assert_eq!(cfg.http.max_body_bytes, 7 * 1024 * 1024);
        });
    }

    #[test]
    fn malformed_numbers_use_defaults_where_applicable() {
        let vars = vec![
            ("APP_ENV", Some("production")),
            ("IMAGE_MAX_WIDTH", Some("NaN")),
            ("IMAGE_MAX_HEIGHT", Some("oops")),
            ("HTTP_MAX_BODY_BYTES", None),
            ("HTTP_MAX_BODY_MB", Some("not-a-number")),
        ];

        temp_env::with_vars(vars, || {
            let cfg = AppConfig::from_env();
            assert_eq!(cfg.image.max_width, 1280);
            assert_eq!(cfg.image.max_height, 1280);
            assert_eq!(cfg.http.max_body_bytes, 5 * 1024 * 1024);
        });
    }

    #[test]
    fn jwt_secret_defaults_to_empty() {
        temp_env::with_vars(vec![("JWT_SECRET", None::<&str>)], || {
            let cfg = AppConfig::from_env();
            assert_eq!(cfg.jwt_secret, "");
        });
    }

    #[test]
    fn jwt_secret_is_loaded_from_env() {
        temp_env::with_vars(vec![("JWT_SECRET", Some("test-secret"))], || {
            let cfg = AppConfig::from_env();
            assert_eq!(cfg.jwt_secret, "test-secret");
        });
    }

    #[test]
    fn html_path_defaults_to_empty() {
        temp_env::with_vars(vec![("HTML_PATH", None::<&str>)], || {
            let cfg = AppConfig::from_env();
            assert_eq!(cfg.html_path, "");
        });
    }

    #[test]
    fn html_path_is_loaded_from_env() {
        temp_env::with_vars(vec![("HTML_PATH", Some("/tmp/index.html"))], || {
            let cfg = AppConfig::from_env();
            assert_eq!(cfg.html_path, "/tmp/index.html");
        });
    }

    #[test]
    fn mail_config_is_none_when_smtp_is_not_configured() {
        temp_env::with_vars(
            vec![
                ("SMTP_HOST", None::<&str>),
                ("SMTP_PORT", None::<&str>),
                ("SMTP_USERNAME", None::<&str>),
                ("SMTP_PASSWORD", None::<&str>),
                ("SMTP_FROM_EMAIL", None::<&str>),
                ("NOTIFY_TO_EMAIL", None::<&str>),
            ],
            || {
                let cfg = AppConfig::from_env();
                assert!(
                    cfg.mail.is_none(),
                    "Expected mail config to be None when SMTP is not configured"
                );
            },
        );
    }

    #[test]
    fn mail_config_is_none_when_required_smtp_vars_are_incomplete() {
        temp_env::with_vars(
            vec![
                // SMTP_HOST exists, but others are missing
                ("SMTP_HOST", Some("smtp.example.com")),
                ("SMTP_PORT", None::<&str>),
                ("SMTP_USERNAME", None::<&str>),
                ("SMTP_PASSWORD", None::<&str>),
                ("SMTP_FROM_EMAIL", None::<&str>),
            ],
            || {
                let cfg = AppConfig::from_env();
                assert!(
                    cfg.mail.is_none(),
                    "Expected mail config to be None when SMTP vars are incomplete"
                );
            },
        );
    }

    #[test]
    fn mail_config_is_loaded_when_all_required_smtp_vars_are_present() {
        temp_env::with_vars(
            vec![
                ("SMTP_HOST", Some("smtp.example.com")),
                ("SMTP_PORT", Some("587")),
                ("SMTP_USERNAME", Some("user")),
                ("SMTP_PASSWORD", Some("pass")),
                ("SMTP_FROM_EMAIL", Some("noreply@example.com")),
                ("SMTP_FROM_NAME", Some("Notifier")),
                ("NOTIFY_TO_EMAIL", Some("notify@example.com")),
            ],
            || {
                let cfg = AppConfig::from_env();
                let mail = cfg.mail.expect("mail config should be present");

                assert_eq!(mail.host, "smtp.example.com");
                assert_eq!(mail.port, 587);
                assert_eq!(mail.username, "user");
                assert_eq!(mail.password, "pass");
                assert_eq!(mail.from_email, "noreply@example.com");
                assert_eq!(mail.from_name, "Notifier");

                assert_eq!(mail.notify_to, vec!["notify@example.com"]);
            },
        );
    }

    #[test]
    fn mail_config_uses_defaults_for_optional_fields() {
        temp_env::with_vars(
            vec![
                ("SMTP_HOST", Some("smtp.example.com")),
                ("SMTP_PORT", Some("25")),
                ("SMTP_USERNAME", Some("user")),
                ("SMTP_PASSWORD", Some("pass")),
                ("SMTP_FROM_EMAIL", Some("noreply@example.com")),
                // Optional values unset
                ("SMTP_FROM_NAME", None),
                ("NOTIFY_TO_EMAIL", None),
            ],
            || {
                let cfg = AppConfig::from_env();
                let mail = cfg.mail.expect("mail config should be present");

                assert_eq!(mail.from_name, "Notifier");
                assert!(
                    mail.notify_to.is_empty(),
                    "Expected notify_to to be empty when NOTIFY_TO_EMAIL is not set"
                );
            },
        );
    }

    #[test]
    fn mail_config_supports_multiple_notify_to_addresses() {
        temp_env::with_vars(
            vec![
                ("SMTP_HOST", Some("smtp.example.com")),
                ("SMTP_PORT", Some("587")),
                ("SMTP_USERNAME", Some("user")),
                ("SMTP_PASSWORD", Some("pass")),
                ("SMTP_FROM_EMAIL", Some("noreply@example.com")),
                (
                    "NOTIFY_TO_EMAIL",
                    Some("notify1@example.com, notify2@example.com"),
                ),
            ],
            || {
                let cfg = AppConfig::from_env();
                let mail = cfg.mail.expect("mail config should be present");

                assert_eq!(
                    mail.notify_to,
                    vec!["notify1@example.com", "notify2@example.com"]
                );
            },
        );
    }
}
