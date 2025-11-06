//! # HTTP and CORS Configuration
//!
//! Defines basic configuration structures for HTTP request handling and
//! CORS (Cross-Origin Resource Sharing) behavior.
//!
//! These are typically included within [`AppConfig`](crate::config::app::AppConfig)
//! or other service-specific configuration layers.
//!
//! # Examples
//! ```rust
//! use wzs_web::config::web::{HttpConfig, CorsConfig};
//!
//! let http = HttpConfig { max_body_bytes: 10 * 1024 * 1024 };
//! let cors = CorsConfig {
//!     env: "http://localhost:5173".into(),
//!     credentials: true,
//! };
//!
//! assert!(http.max_body_bytes > 1_000_000);
//! assert_eq!(cors.env, "http://localhost:5173");
//! ```

/// HTTP-related configuration.
///
/// Typically controls request body size limits or other HTTP-layer constraints.
///
/// # Example
/// ```rust
/// use wzs_web::config::web::HttpConfig;
///
/// let cfg = HttpConfig { max_body_bytes: 5 * 1024 * 1024 };
/// assert_eq!(cfg.max_body_bytes, 5 * 1024 * 1024);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct HttpConfig {
    pub max_body_bytes: usize,
}

/// CORS (Cross-Origin Resource Sharing) configuration.
///
/// Defines allowed origin and credential policy used by the HTTP server.
///
/// # Example
/// ```rust
/// use wzs_web::config::web::CorsConfig;
///
/// let cors = CorsConfig {
///     env: "http://localhost:5173".into(),
///     credentials: true,
/// };
///
/// assert!(cors.credentials);
/// assert_eq!(cors.env, "http://localhost:5173");
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct CorsConfig {
    pub env: String,
    pub credentials: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_config_holds_value() {
        let cfg = HttpConfig {
            max_body_bytes: 10 * 1024 * 1024,
        };
        assert_eq!(cfg.max_body_bytes, 10 * 1024 * 1024);
    }

    #[test]
    fn cors_config_holds_values() {
        let cfg = CorsConfig {
            env: "http://localhost:5173".into(),
            credentials: true,
        };
        assert_eq!(cfg.env, "http://localhost:5173");
        assert!(cfg.credentials);

        let cfg2 = CorsConfig {
            env: "https://example.com".into(),
            credentials: false,
        };
        assert_eq!(cfg2.env, "https://example.com");
        assert!(!cfg2.credentials);
    }

    #[test]
    fn http_and_cors_configs_are_clone_and_debug() {
        let http_cfg = HttpConfig {
            max_body_bytes: 123,
        };
        let http_clone = http_cfg.clone();
        assert_eq!(http_cfg, http_clone);

        let cors_cfg = CorsConfig {
            env: "dev".into(),
            credentials: false,
        };
        let cors_clone = cors_cfg.clone();
        assert_eq!(cors_cfg, cors_clone);

        let dbg = format!("{:?}", cors_cfg);
        assert!(dbg.contains("dev"));
    }
}
