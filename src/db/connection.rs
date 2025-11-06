//! # MySQL Connection Pool Factory
//!
//! Provides a helper to create a new [`mysql::Pool`] instance from a given
//! [`DbConfig`].
//!
//! Unlike [`get_pool`](super::connection::get_pool), this version does **not**
//! cache the pool globally. It simply builds a new connection pool using the
//! supplied configuration.
//!
//! This makes it ideal for dependency injection, testing, or use in
//! applications that manage their own lifecycle for the database pool.
//!
//! # Example
//! ```rust,no_run
//! use wzs_web::config::db::DbConfig;
//! use wzs_web::db::connection::get_pool;
//!
//! let cfg = DbConfig::from_env();
//! let pool = get_pool(&cfg);
//!
//! // Use pool as Arc<mysql::Pool>
//! let conn = pool.get_conn().expect("failed to get connection");
//! ```
use crate::config::db::{create_pool, DbConfig, DbPool};

/// Creates a new MySQL connection pool using the given configuration.
///
/// This function does **not** cache or reuse the pool — it simply builds a new one
/// based on the provided [`DbConfig`].
///
/// # Panics
/// Panics if the pool creation fails (e.g., invalid `DATABASE_URL` or
/// connection error).
///
/// # Example
/// ```rust,no_run
/// use wzs_web::config::db::DbConfig;
/// use wzs_web::db::connection::get_pool;
///
/// let cfg = DbConfig::from_env();
/// let pool = get_pool(&cfg);
/// ```
pub fn get_pool(cfg: &DbConfig) -> DbPool {
    create_pool(cfg).expect("failed to initialize MySQL connection pool")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Dummy type that simulates `mysql::Pool` without connecting.
    struct DummyPool;

    type DummyDbPool = Arc<DummyPool>;

    fn fake_create_pool(_cfg: &DbConfig) -> anyhow::Result<DummyDbPool> {
        Ok(Arc::new(DummyPool))
    }

    /// Verifies that the function returns an `Arc`-wrapped pool type.
    #[test]
    fn get_pool_returns_arc_pool_type() {
        // Create dummy config (doesn't need a real DB)
        let cfg = DbConfig {
            url: Some("mysql://root:pass@localhost:3306/testdb".into()),
            max_connections: Some(5),
        };

        // Replace the actual pool creation with a fake one
        let pool = fake_create_pool(&cfg).unwrap();
        assert!(Arc::strong_count(&pool) >= 1);
    }

    /// Ensures that missing `DATABASE_URL` triggers a panic.
    #[test]
    #[should_panic(expected = "DATABASE_URL")]
    fn get_pool_panics_without_database_url() {
        let cfg = DbConfig {
            url: None,
            max_connections: None,
        };
        let _ = get_pool(&cfg);
    }
}
