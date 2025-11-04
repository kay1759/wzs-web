//! # MySQL Connection Pool
//!
//! Provides a lazily initialized, globally shared MySQL connection pool.
//! Reads `DATABASE_URL` from the environment and builds a [`mysql::Pool`].
//!
//! Once initialized, the same pool is reused throughout the application.
//!
//! # Examples
//! ```rust,no_run
//! use wzs_web::db::connection::get_pool;
//!
//! let pool = get_pool();
//! ```
//!
//! Once initialized, subsequent calls will return the same cached pool.

use crate::config::db::{create_pool, DbConfig, DbPool};
use std::sync::OnceLock;

/// Global MySQL connection pool, created once and reused.
static GLOBAL_POOL: OnceLock<DbPool> = OnceLock::new();

/// Returns the global MySQL pool.
///
/// Panics if `DATABASE_URL` is missing or pool creation fails.
///
/// # Example
/// ```rust,no_run
/// use wzs_web::db::connection::get_pool;
///
/// let pool = get_pool(); // Reuses the same pool instance across the application.
/// ```
pub fn get_pool() -> &'static DbPool {
    GLOBAL_POOL.get_or_init(|| {
        let cfg = DbConfig::from_env();
        create_pool(&cfg).expect("failed to initialize MySQL connection pool")
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, OnceLock};

    type DummyPool = Arc<String>;

    static TEST_POOL: OnceLock<DummyPool> = OnceLock::new();

    /// Ensures OnceLock caches the same instance across calls.
    #[test]
    fn once_lock_caches_same_instance() {
        fn create_dummy() -> DummyPool {
            Arc::new("mock-pool".to_string())
        }

        let pool1 = TEST_POOL.get_or_init(create_dummy);
        let pool2 = TEST_POOL.get_or_init(create_dummy);

        assert_eq!(
            Arc::as_ptr(pool1),
            Arc::as_ptr(pool2),
            "OnceLock should cache the same instance"
        );

        assert_eq!(pool1.as_str(), "mock-pool");
    }

    #[test]
    #[should_panic(expected = "DATABASE_URL")]
    fn get_pool_panics_without_database_url() {
        use crate::db::connection::get_pool;
        use temp_env;

        temp_env::with_vars(vec![("DATABASE_URL", None::<&str>)], || {
            let _ = get_pool();
        });
    }
}
