use std::fmt;

use anyhow::{Context, Result};

use crate::config::env::EnvConfig;

/// Configuration for sending emails.
///
/// `MailConfig` can be constructed from either the current process environment
/// with [`MailConfig::from_env`] or an existing [`EnvConfig`] snapshot with
/// [`MailConfig::from_env_config`].
///
/// # Required environment variables
///
/// - `SMTP_HOST`
/// - `SMTP_PORT`
/// - `SMTP_USERNAME`
/// - `SMTP_PASSWORD`
/// - `SMTP_FROM_EMAIL`
///
/// # Optional environment variables
///
/// - `SMTP_FROM_NAME` — defaults to `"Notifier"`
/// - `NOTIFY_TO_EMAIL` — comma-separated notification recipient addresses
///
/// ## `NOTIFY_TO_EMAIL` format
///
/// Single address:
///
/// ```text
/// NOTIFY_TO_EMAIL=user@example.com
/// ```
///
/// Multiple addresses:
///
/// ```text
/// NOTIFY_TO_EMAIL=user@example.com,user2@example.com
/// ```
///
/// Whitespace around addresses is trimmed, and empty entries are ignored.
///
/// # Security
///
/// `SMTP_PASSWORD` is intentionally omitted from the [`Debug`] representation
/// of `MailConfig`.
#[derive(Clone)]
pub struct MailConfig {
    /// SMTP server host name or IP address.
    pub host: String,

    /// SMTP server port number.
    pub port: u16,

    /// Username for SMTP authentication.
    pub username: String,

    /// Password for SMTP authentication.
    pub password: String,

    /// Sender email address.
    pub from_email: String,

    /// Sender display name.
    ///
    /// Defaults to `"Notifier"`.
    pub from_name: String,

    /// Notification recipient email addresses.
    ///
    /// When empty, no explicit notification recipient is configured.
    pub notify_to: Vec<String>,
}

impl MailConfig {
    /// Builds a [`MailConfig`] from the current process environment.
    ///
    /// This method is retained for backward compatibility.
    ///
    /// New configuration code should generally capture the environment once
    /// with [`EnvConfig::from_env`] and use [`MailConfig::from_env_config`].
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - a required environment variable is missing, or
    /// - `SMTP_PORT` cannot be parsed as a `u16`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use wzs_web::config::mail::MailConfig;
    ///
    /// let config = MailConfig::from_env()?;
    ///
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn from_env() -> Result<Self> {
        let env = EnvConfig::from_env();

        Self::from_env_config(&env)
    }

    /// Builds a [`MailConfig`] from an [`EnvConfig`] snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when one of the following required variables is
    /// missing:
    ///
    /// - `SMTP_HOST`
    /// - `SMTP_PORT`
    /// - `SMTP_USERNAME`
    /// - `SMTP_PASSWORD`
    /// - `SMTP_FROM_EMAIL`
    ///
    /// An error is also returned when `SMTP_PORT` cannot be parsed as a
    /// `u16`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use wzs_web::config::env::EnvConfig;
    /// use wzs_web::config::mail::MailConfig;
    ///
    /// let env = EnvConfig::from_pairs([
    ///     ("SMTP_HOST", "smtp.example.com"),
    ///     ("SMTP_PORT", "587"),
    ///     ("SMTP_USERNAME", "user"),
    ///     ("SMTP_PASSWORD", "password"),
    ///     ("SMTP_FROM_EMAIL", "noreply@example.com"),
    /// ]);
    ///
    /// let config = MailConfig::from_env_config(&env)
    ///     .expect("failed to load mail configuration");
    ///
    /// assert_eq!(config.host, "smtp.example.com");
    /// assert_eq!(config.port, 587);
    /// assert_eq!(config.from_name, "Notifier");
    /// ```
    pub fn from_env_config(env: &EnvConfig) -> Result<Self> {
        let host = env.get_string("SMTP_HOST").context("SMTP_HOST not set")?;

        let port = env
            .get("SMTP_PORT")
            .context("SMTP_PORT not set")?
            .parse::<u16>()
            .context("SMTP_PORT parse error")?;

        let username = env
            .get_string("SMTP_USERNAME")
            .context("SMTP_USERNAME not set")?;

        let password = env
            .get_string("SMTP_PASSWORD")
            .context("SMTP_PASSWORD not set")?;

        let from_email = env
            .get_string("SMTP_FROM_EMAIL")
            .context("SMTP_FROM_EMAIL not set")?;

        let from_name = env
            .get_string("SMTP_FROM_NAME")
            .unwrap_or_else(|| "Notifier".to_string());

        let notify_to = env
            .get_string("NOTIFY_TO_EMAIL")
            .map(parse_notify_to)
            .unwrap_or_default();

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

/// Provides a safe debug representation of [`MailConfig`].
///
/// The SMTP password is intentionally redacted so that secrets are not
/// accidentally written to logs.
impl fmt::Debug for MailConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MailConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("from_email", &self.from_email)
            .field("from_name", &self.from_name)
            .field("notify_to", &self.notify_to)
            .finish()
    }
}

/// Parses a `NOTIFY_TO_EMAIL` value into a list of email address strings.
///
/// The input is:
///
/// - split by comma,
/// - trimmed,
/// - filtered to remove empty entries.
///
/// This function does not validate whether each entry is a syntactically
/// valid email address.
fn parse_notify_to(value: String) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates the minimum valid environment required by [`MailConfig`].
    fn valid_env() -> EnvConfig {
        EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_PORT", "587"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "pass"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
        ])
    }

    #[test]
    fn from_env_config_success_with_defaults() {
        let env = valid_env();

        let config = MailConfig::from_env_config(&env).expect("should load config");

        assert_eq!(config.host, "smtp.example.com");
        assert_eq!(config.port, 587);
        assert_eq!(config.username, "user");
        assert_eq!(config.password, "pass");
        assert_eq!(config.from_email, "noreply@example.com");
        assert_eq!(config.from_name, "Notifier");
        assert!(config.notify_to.is_empty());
    }

    #[test]
    fn from_env_config_respects_from_name() {
        let env = EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_PORT", "587"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "pass"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
            ("SMTP_FROM_NAME", "Example Service"),
        ]);

        let config = MailConfig::from_env_config(&env).expect("should load config");

        assert_eq!(config.from_name, "Example Service");
    }

    #[test]
    fn from_env_config_with_single_notify_to() {
        let env = EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_PORT", "587"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "pass"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
            ("NOTIFY_TO_EMAIL", "notify@example.com"),
        ]);

        let config = MailConfig::from_env_config(&env).expect("should load config");

        assert_eq!(config.notify_to, vec!["notify@example.com"]);
    }

    #[test]
    fn from_env_config_with_multiple_notify_to() {
        let env = EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_PORT", "587"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "pass"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
            ("NOTIFY_TO_EMAIL", "notify@example.com, notify2@example.com"),
        ]);

        let config = MailConfig::from_env_config(&env).expect("should load config");

        assert_eq!(
            config.notify_to,
            vec!["notify@example.com", "notify2@example.com",]
        );
    }

    #[test]
    fn from_env_config_ignores_empty_notify_to_entries() {
        let env = EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_PORT", "587"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "pass"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
            (
                "NOTIFY_TO_EMAIL",
                "notify@example.com, ,  ,notify2@example.com,",
            ),
        ]);

        let config = MailConfig::from_env_config(&env).expect("should load config");

        assert_eq!(
            config.notify_to,
            vec!["notify@example.com", "notify2@example.com",]
        );
    }

    #[test]
    fn from_env_config_allows_empty_notify_to() {
        let env = EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_PORT", "587"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "pass"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
            ("NOTIFY_TO_EMAIL", ""),
        ]);

        let config = MailConfig::from_env_config(&env).expect("should load config");

        assert!(config.notify_to.is_empty());
    }

    #[test]
    fn from_env_config_errors_when_host_is_missing() {
        let env = EnvConfig::from_pairs([
            ("SMTP_PORT", "587"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "pass"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
        ]);

        let result = MailConfig::from_env_config(&env);

        assert!(result.is_err());
        assert!(format!("{result:?}").contains("SMTP_HOST not set"));
    }

    #[test]
    fn from_env_config_errors_when_port_is_missing() {
        let env = EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "pass"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
        ]);

        let result = MailConfig::from_env_config(&env);

        assert!(result.is_err());
        assert!(format!("{result:?}").contains("SMTP_PORT not set"));
    }

    #[test]
    fn from_env_config_errors_when_username_is_missing() {
        let env = EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_PORT", "587"),
            ("SMTP_PASSWORD", "pass"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
        ]);

        let result = MailConfig::from_env_config(&env);

        assert!(result.is_err());
        assert!(format!("{result:?}").contains("SMTP_USERNAME not set"));
    }

    #[test]
    fn from_env_config_errors_when_password_is_missing() {
        let env = EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_PORT", "587"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
        ]);

        let result = MailConfig::from_env_config(&env);

        assert!(result.is_err());
        assert!(format!("{result:?}").contains("SMTP_PASSWORD not set"));
    }

    #[test]
    fn from_env_config_errors_when_from_email_is_missing() {
        let env = EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_PORT", "587"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "pass"),
        ]);

        let result = MailConfig::from_env_config(&env);

        assert!(result.is_err());
        assert!(format!("{result:?}").contains("SMTP_FROM_EMAIL not set"));
    }

    #[test]
    fn from_env_config_errors_when_port_is_invalid() {
        let env = EnvConfig::from_pairs([
            ("SMTP_HOST", "smtp.example.com"),
            ("SMTP_PORT", "not-a-number"),
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "pass"),
            ("SMTP_FROM_EMAIL", "noreply@example.com"),
        ]);

        let result = MailConfig::from_env_config(&env);

        assert!(result.is_err());
        assert!(format!("{result:?}").contains("SMTP_PORT parse error"));
    }

    #[test]
    fn parse_notify_to_single_address() {
        assert_eq!(
            parse_notify_to("notify@example.com".to_string()),
            vec!["notify@example.com"]
        );
    }

    #[test]
    fn parse_notify_to_multiple_addresses() {
        assert_eq!(
            parse_notify_to("first@example.com, second@example.com".to_string()),
            vec!["first@example.com", "second@example.com",]
        );
    }

    #[test]
    fn parse_notify_to_removes_empty_entries() {
        assert_eq!(
            parse_notify_to("first@example.com, , second@example.com,".to_string()),
            vec!["first@example.com", "second@example.com",]
        );
    }

    #[test]
    fn debug_redacts_password() {
        let config = MailConfig::from_env_config(&valid_env()).expect("valid config");

        let debug = format!("{config:?}");

        assert!(debug.contains("smtp.example.com"));
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("\"pass\""));
    }
}
