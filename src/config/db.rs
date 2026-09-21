//! # Database Configuration and Pool Factory
//!
//! Provides database connection configuration ([`DbConfig`]) and a helper
//! to create a reusable MySQL connection pool ([`DbPool`]).
//!
//! Database configuration can be built either from the current process
//! environment or from an [`EnvConfig`] environment snapshot.
//!
//! The following environment variables are supported:
//!
//! - `DATABASE_URL` — MySQL connection URL
//! - `DATABASE_MAX_CONN` — optional maximum pool size
//!
//! # Examples
//!
//! ## From the process environment
//!
//! ```rust,no_run
//! use wzs_web::config::db::{create_pool, DbConfig};
//!
//! let cfg = DbConfig::from_env();
//!
//! if cfg.is_valid() {
//!     let pool = create_pool(&cfg).expect("failed to create pool");
//!     // use pool...
//! }
//! ```
//!
//! ## From an environment snapshot
//!
//! ```rust
//! use wzs_web::config::db::DbConfig;
//! use wzs_web::config::env::EnvConfig;
//!
//! let env = EnvConfig::from_pairs([
//!     ("DATABASE_URL", "mysql://root:pass@localhost:3306/testdb"),
//!     ("DATABASE_MAX_CONN", "20"),
//! ]);
//!
//! let cfg = DbConfig::from_env_config(&env);
//!
//! assert_eq!(
//!     cfg.url.as_deref(),
//!     Some("mysql://root:pass@localhost:3306/testdb")
//! );
//! assert_eq!(cfg.max_connections, Some(20));
//! ```

use std::sync::Arc;

use mysql::{Opts, Pool};

use crate::config::env::EnvConfig;

/// Database connection configuration.
///
/// The following environment variables are supported:
///
/// - `DATABASE_URL` — MySQL connection URL
/// - `DATABASE_MAX_CONN` — optional maximum pool size
///
/// `DbConfig` can be constructed from either the current process environment
/// with [`DbConfig::from_env`] or an existing [`EnvConfig`] snapshot with
/// [`DbConfig::from_env_config`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbConfig {
    /// MySQL connection URL.
    ///
    /// `None` when `DATABASE_URL` is not configured.
    pub url: Option<String>,

    /// Optional maximum number of database connections.
    ///
    /// `None` when `DATABASE_MAX_CONN` is missing or cannot be parsed as a
    /// `u32`.
    pub max_connections: Option<u32>,
}

impl DbConfig {
    /// Builds a [`DbConfig`] from the current process environment.
    ///
    /// This method is retained for backward compatibility.
    ///
    /// New configuration code should generally capture the environment once
    /// with [`EnvConfig::from_env`] and use [`DbConfig::from_env_config`].
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use wzs_web::config::db::DbConfig;
    ///
    /// let cfg = DbConfig::from_env();
    /// ```
    pub fn from_env() -> Self {
        let env = EnvConfig::from_env();

        Self::from_env_config(&env)
    }

    /// Builds a [`DbConfig`] from an [`EnvConfig`] snapshot.
    ///
    /// This is the preferred constructor when configuration is managed through
    /// a shared environment snapshot.
    ///
    /// `DATABASE_MAX_CONN` is parsed as a `u32`. If it is missing or invalid,
    /// `max_connections` is set to `None`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use wzs_web::config::db::DbConfig;
    /// use wzs_web::config::env::EnvConfig;
    ///
    /// let env = EnvConfig::from_pairs([
    ///     ("DATABASE_URL", "mysql://root:pass@localhost:3306/testdb"),
    ///     ("DATABASE_MAX_CONN", "20"),
    /// ]);
    ///
    /// let cfg = DbConfig::from_env_config(&env);
    ///
    /// assert_eq!(
    ///     cfg.url.as_deref(),
    ///     Some("mysql://root:pass@localhost:3306/testdb")
    /// );
    /// assert_eq!(cfg.max_connections, Some(20));
    /// ```
    pub fn from_env_config(env: &EnvConfig) -> Self {
        Self {
            url: env.get_string("DATABASE_URL"),
            max_connections: env.get_u32("DATABASE_MAX_CONN"),
        }
    }

    /// Returns `true` if `DATABASE_URL` is present.
    ///
    /// This method checks only whether the configuration contains a database
    /// URL. It does not validate the URL itself or attempt a database
    /// connection.
    pub fn is_valid(&self) -> bool {
        self.url.is_some()
    }
}

/// Shared database pool type alias.
///
/// The underlying MySQL pool is wrapped in an [`Arc`] so that it can be
/// cheaply cloned and shared across application components.
pub type DbPool = Arc<Pool>;

/// Creates a new [`DbPool`] using the given configuration.
///
/// # Errors
///
/// Returns an error if:
///
/// - `DATABASE_URL` is missing,
/// - the database URL is invalid, or
/// - the MySQL pool cannot be created.
///
/// # Example
///
/// ```rust,no_run
/// use wzs_web::config::db::{create_pool, DbConfig};
///
/// let cfg = DbConfig::from_env();
/// let pool = create_pool(&cfg).expect("failed to initialize pool");
/// ```
pub fn create_pool(cfg: &DbConfig) -> anyhow::Result<DbPool> {
    let url = cfg
        .url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("DATABASE_URL is not set"))?;

    let opts = Opts::from_url(url)?;
    let pool = Pool::new(opts)?;

    Ok(Arc::new(pool))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::TypeId;

    #[test]
    fn dbpool_aliases_arc_pool() {
        assert_eq!(TypeId::of::<DbPool>(), TypeId::of::<Arc<Pool>>());
    }

    #[test]
    fn dbpool_deref_target_is_pool() {
        fn accepts_arc_pool<T: std::ops::Deref<Target = Pool>>() {}

        accepts_arc_pool::<DbPool>();
    }

    #[test]
    fn dbconfig_reads_from_env_config() {
        let env = EnvConfig::from_pairs([
            ("DATABASE_URL", "mysql://root:pass@localhost:3306/testdb"),
            ("DATABASE_MAX_CONN", "20"),
        ]);

        let cfg = DbConfig::from_env_config(&env);

        assert_eq!(
            cfg.url.as_deref(),
            Some("mysql://root:pass@localhost:3306/testdb")
        );
        assert_eq!(cfg.max_connections, Some(20));
    }

    #[test]
    fn dbconfig_allows_missing_database_url() {
        let env = EnvConfig::default();

        let cfg = DbConfig::from_env_config(&env);

        assert_eq!(cfg.url, None);
        assert_eq!(cfg.max_connections, None);
        assert!(!cfg.is_valid());
    }

    #[test]
    fn dbconfig_is_valid_when_database_url_exists() {
        let env =
            EnvConfig::from_pairs([("DATABASE_URL", "mysql://root:pass@localhost:3306/testdb")]);

        let cfg = DbConfig::from_env_config(&env);

        assert!(cfg.is_valid());
    }

    #[test]
    fn dbconfig_allows_missing_max_connections() {
        let env =
            EnvConfig::from_pairs([("DATABASE_URL", "mysql://root:pass@localhost:3306/testdb")]);

        let cfg = DbConfig::from_env_config(&env);

        assert_eq!(cfg.max_connections, None);
    }

    #[test]
    fn dbconfig_ignores_invalid_max_connections() {
        let env = EnvConfig::from_pairs([
            ("DATABASE_URL", "mysql://root:pass@localhost:3306/testdb"),
            ("DATABASE_MAX_CONN", "invalid"),
        ]);

        let cfg = DbConfig::from_env_config(&env);

        assert_eq!(cfg.max_connections, None);
    }

    #[test]
    fn dbconfig_parses_max_connections_with_whitespace() {
        let env = EnvConfig::from_pairs([
            ("DATABASE_URL", "mysql://root:pass@localhost:3306/testdb"),
            ("DATABASE_MAX_CONN", " 20 "),
        ]);

        let cfg = DbConfig::from_env_config(&env);

        assert_eq!(cfg.max_connections, Some(20));
    }
}
