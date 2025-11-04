//! # Database Configuration and Pool Factory
//!
//! Provides database connection configuration (`DbConfig`) and a helper
//! to create a reusable MySQL connection pool (`DbPool`).
//!
//! The connection URL and maximum pool size are typically loaded from
//! environment variables (`DATABASE_URL`, `DATABASE_MAX_CONN`).
//!
//! # Examples
//! ```rust,no_run
//! use wzs_web::config::db::{DbConfig, create_pool};
//!
//! let cfg = DbConfig::from_env();
//! if cfg.is_valid() {
//!     let pool = create_pool(&cfg).expect("failed to create pool");
//!     // use pool...
//! }
//! ```

use std::{env, sync::Arc};

use mysql::{Opts, Pool};

/// Database connection configuration.
///
/// Reads from environment variables:
/// - `DATABASE_URL` — MySQL connection URL
/// - `DATABASE_MAX_CONN` — optional maximum pool size
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbConfig {
    pub url: Option<String>,
    pub max_connections: Option<u32>,
}

impl DbConfig {
    /// Builds a [`DbConfig`] from environment variables.
    pub fn from_env() -> Self {
        let url = env::var("DATABASE_URL").ok();
        let max_connections = env::var("DATABASE_MAX_CONN")
            .ok()
            .and_then(|s| s.parse::<u32>().ok());
        Self {
            url,
            max_connections,
        }
    }

    /// Returns `true` if `DATABASE_URL` is present.
    pub fn is_valid(&self) -> bool {
        self.url.is_some()
    }
}

/// Shared database pool type alias (`Arc<mysql::Pool>`).
pub type DbPool = Arc<Pool>;

/// Creates a new [`DbPool`] using the given configuration.
///
/// # Errors
/// Returns an error if:
/// - `DATABASE_URL` is missing
/// - the URL is invalid
/// - the pool cannot be created
///
/// # Example
/// ```rust,no_run
/// use wzs_web::config::db::{DbConfig, create_pool};
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
    use temp_env;

    #[test]
    fn dbpool_aliases_arc_pool() {
        assert_eq!(TypeId::of::<DbPool>(), TypeId::of::<Arc<Pool>>());
    }

    #[test]
    fn dbconfig_reads_from_env() {
        temp_env::with_vars(
            vec![
                (
                    "DATABASE_URL",
                    Some("mysql://root:pass@localhost:3306/testdb"),
                ),
                ("DATABASE_MAX_CONN", Some("20")),
            ],
            || {
                let cfg = DbConfig::from_env();
                assert_eq!(
                    cfg.url.as_deref(),
                    Some("mysql://root:pass@localhost:3306/testdb")
                );
                assert_eq!(cfg.max_connections, Some(20));
            },
        );
    }

    #[test]
    fn dbpool_deref_target_is_pool() {
        fn accepts_arc_pool<T: std::ops::Deref<Target = Pool>>() {}
        accepts_arc_pool::<DbPool>();
    }
}
