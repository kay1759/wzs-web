use std::env;

use anyhow::{Context, Result};

/// Configuration struct for sending emails.
///
/// This configuration is loaded from environment variables.
///
/// Required:
/// - `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`,
///   `SMTP_FROM_EMAIL`
///
/// Optional:
/// - `SMTP_FROM_NAME` (default: `"Notifier"`)
/// - `NOTIFY_TO_EMAIL`
#[derive(Clone, Debug)]
pub struct MailConfig {
    /// SMTP server host name or IP address
    pub host: String,
    /// SMTP server port number
    pub port: u16,
    /// Username for SMTP authentication
    pub username: String,
    /// Password for SMTP authentication
    pub password: String,
    /// Sender email address
    pub from_email: String,
    /// Sender display name (defaults to `"Notifier"`)
    pub from_name: String,
    /// Optional notification recipient email address
    pub notify_to: Option<String>,
}

impl MailConfig {
    /// Creates a `MailConfig` from environment variables.
    ///
    /// # Errors
    /// - When a required environment variable is missing
    /// - When `SMTP_PORT` cannot be parsed as a number
    pub fn from_env() -> Result<Self> {
        let host = env::var("SMTP_HOST").context("SMTP_HOST not set")?;
        let port: u16 = env::var("SMTP_PORT")
            .context("SMTP_PORT not set")?
            .parse()
            .context("SMTP_PORT parse error")?;
        let username = env::var("SMTP_USERNAME").context("SMTP_USERNAME not set")?;
        let password = env::var("SMTP_PASSWORD").context("SMTP_PASSWORD not set")?;
        let from_email = env::var("SMTP_FROM_EMAIL").context("SMTP_FROM_EMAIL not set")?;

        // Optional variables fall back to defaults or None
        let from_name = env::var("SMTP_FROM_NAME").unwrap_or_else(|_| "Notifier".into());
        let notify_to = env::var("NOTIFY_TO_EMAIL").ok();

        Ok(Self {
            host,
            port,
            username,
            password,
            from_email,
            from_name,
            notify_to,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temp_env;

    #[test]
    fn test_from_env_success_with_defaults() {
        temp_env::with_vars(
            vec![
                ("SMTP_HOST", Some("smtp.example.com")),
                ("SMTP_PORT", Some("587")),
                ("SMTP_USERNAME", Some("user")),
                ("SMTP_PASSWORD", Some("pass")),
                ("SMTP_FROM_EMAIL", Some("noreply@example.com")),
                // Optional variables unset
                ("SMTP_FROM_NAME", None),
                ("NOTIFY_TO_EMAIL", None),
            ],
            || {
                let config = MailConfig::from_env().expect("should load config");
                assert_eq!(config.host, "smtp.example.com");
                assert_eq!(config.port, 587);
                assert_eq!(config.username, "user");
                assert_eq!(config.password, "pass");
                assert_eq!(config.from_email, "noreply@example.com");
                assert_eq!(config.from_name, "Notifier"); // default
                assert!(config.notify_to.is_none());
            },
        );
    }

    #[test]
    fn test_from_env_with_overrides() {
        temp_env::with_vars(
            vec![
                ("SMTP_HOST", Some("smtp.example.com")),
                ("SMTP_PORT", Some("587")),
                ("SMTP_USERNAME", Some("user")),
                ("SMTP_PASSWORD", Some("pass")),
                ("SMTP_FROM_EMAIL", Some("noreply@example.com")),
                ("SMTP_FROM_NAME", Some("CustomName")),
                ("NOTIFY_TO_EMAIL", Some("notify@example.com")),
            ],
            || {
                let config = MailConfig::from_env().expect("should load config");
                assert_eq!(config.from_name, "CustomName");
                assert_eq!(config.notify_to.as_deref(), Some("notify@example.com"));
            },
        );
    }

    #[test]
    fn test_missing_required_env() {
        temp_env::with_vars(
            vec![
                // Missing required variable
                ("SMTP_HOST", None),
                ("SMTP_PORT", Some("587")),
                ("SMTP_USERNAME", Some("user")),
                ("SMTP_PASSWORD", Some("pass")),
                ("SMTP_FROM_EMAIL", Some("noreply@example.com")),
            ],
            || {
                let result = MailConfig::from_env();
                assert!(result.is_err(), "expected error when SMTP_HOST is missing");
                let msg = format!("{:?}", result);
                assert!(msg.contains("SMTP_HOST not set"));
            },
        );
    }

    #[test]
    fn test_invalid_port() {
        temp_env::with_vars(
            vec![
                ("SMTP_HOST", Some("smtp.example.com")),
                ("SMTP_PORT", Some("not-a-number")),
                ("SMTP_USERNAME", Some("user")),
                ("SMTP_PASSWORD", Some("pass")),
                ("SMTP_FROM_EMAIL", Some("noreply@example.com")),
            ],
            || {
                let result = MailConfig::from_env();
                assert!(result.is_err(), "expected error when port is invalid");
                let msg = format!("{:?}", result);
                assert!(msg.contains("SMTP_PORT parse error"));
            },
        );
    }
}
