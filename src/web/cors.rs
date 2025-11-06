//! # CORS (Cross-Origin Resource Sharing) Utilities
//!
//! Provides a configurable [`CorsLayer`] builder for Axum applications.
//!
//! CORS settings are derived from [`CorsConfig`], allowing runtime customization of
//! allowed origins, credentials, and headers such as `X-CSRF-Token`.
//!
//! If no origins are configured, defaults to allowing `http://localhost:5173`
//! — suitable for local frontend development.
//!
//! # Example
//! ```rust,no_run
//! use axum::{routing::get, Router};
//! use wzs_web::config::web::CorsConfig;
//! use wzs_web::web::cors::build_cors;
//!
//! let cfg = CorsConfig {
//!     env: "http://example.com".into(),
//!     credentials: true,
//! };
//!
//! let app: Router = Router::new()
//!     .route("/api/hello", get(|| async { "Hello" }))
//!     .layer(build_cors(&cfg));
//! ```
//!
//! This setup will allow cross-origin requests from `http://example.com`
//! and include `Access-Control-Allow-Credentials: true` in responses.

use axum::http::{header, HeaderName, HeaderValue, Method};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::config::web::CorsConfig;

/// Parses a comma-separated list of origins from environment configuration.
///
/// Invalid or empty entries are ignored.
fn parse_origins_from_env(cors_env: String) -> Vec<HeaderValue> {
    cors_env
        .split(',')
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                HeaderValue::from_str(s).ok()
            }
        })
        .collect()
}

/// Builds a [`CorsLayer`] configured from [`CorsConfig`].
///
/// - Allows `GET`, `POST`, and `OPTIONS` methods.
/// - Always includes `Content-Type` and `X-CSRF-Token` headers.
/// - Defaults to `http://localhost:5173` when no origins are provided.
/// - Enables credentials when `CorsConfig.credentials` is `true`.
///
/// # Example
/// ```rust,no_run
/// use wzs_web::config::web::CorsConfig;
/// use wzs_web::web::cors::build_cors;
///
/// let cors = CorsConfig {
///     env: "https://frontend.example".into(),
///     credentials: false,
/// };
/// let layer = build_cors(&cors);
/// ```
pub fn build_cors(cors: &CorsConfig) -> CorsLayer {
    let origins = parse_origins_from_env(cors.env.clone());

    // Allowed origins — "*" cannot be used when credentials=true
    let origin_cfg = if origins.is_empty() {
        // Default to local dev port if not specified
        AllowOrigin::list([HeaderValue::from_static("http://localhost:5173")])
    } else {
        AllowOrigin::list(origins)
    };

    let mut layer = CorsLayer::new()
        .allow_origin(origin_cfg)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            header::CONTENT_TYPE,
            HeaderName::from_static("x-csrf-token"),
        ]);

    if cors.credentials {
        layer = layer.allow_credentials(true);
    }

    layer
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::{get, options},
        Router,
    };
    use tower::ServiceExt;

    #[test]
    fn parse_origins_trims_and_allows_headervalues() {
        let input = "  http://a.com , ,  http://bad host  , https://b.com ".to_string();
        let out = super::parse_origins_from_env(input);

        let strings: Vec<String> = out
            .iter()
            .map(|h| h.to_str().unwrap().to_string())
            .collect();

        assert_eq!(
            strings,
            vec!["http://a.com", "http://bad host", "https://b.com"]
        );
    }

    #[tokio::test]
    async fn cors_preflight_allows_configured_origin_and_headers() {
        let cfg = CorsConfig {
            env: "http://example.com".into(),
            credentials: true,
        };

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .route("/test", options(|| async { StatusCode::NO_CONTENT }))
            .layer(build_cors(&cfg));

        let req = Request::builder()
            .method("OPTIONS")
            .uri("/test")
            .header("Origin", "http://example.com")
            .header("Access-Control-Request-Method", "POST")
            .header(
                "Access-Control-Request-Headers",
                "x-csrf-token, content-type",
            )
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();

        assert!(
            matches!(res.status(), StatusCode::NO_CONTENT | StatusCode::OK),
            "unexpected status: {}",
            res.status()
        );

        assert_eq!(
            res.headers()
                .get("access-control-allow-origin")
                .unwrap()
                .to_str()
                .unwrap(),
            "http://example.com"
        );

        let allow_headers = res
            .headers()
            .get("access-control-allow-headers")
            .unwrap()
            .to_str()
            .unwrap()
            .to_ascii_lowercase();

        assert!(allow_headers.contains("x-csrf-token"));
        assert!(allow_headers.contains("content-type"));
    }

    #[tokio::test]
    async fn cors_defaults_to_localhost_when_env_empty() {
        let cfg = CorsConfig {
            env: "".into(),
            credentials: false,
        };

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .route("/test", options(|| async { StatusCode::NO_CONTENT }))
            .layer(build_cors(&cfg));

        let pre = Request::builder()
            .method("OPTIONS")
            .uri("/test")
            .header("Origin", "http://localhost:5173")
            .header("Access-Control-Request-Method", "GET")
            .body(Body::empty())
            .unwrap();

        let pre_res = app.clone().oneshot(pre).await.unwrap();

        assert!(
            matches!(pre_res.status(), StatusCode::NO_CONTENT | StatusCode::OK),
            "unexpected status: {}",
            pre_res.status()
        );

        assert_eq!(
            pre_res
                .headers()
                .get("access-control-allow-origin")
                .unwrap()
                .to_str()
                .unwrap(),
            "http://localhost:5173"
        );

        let req = Request::builder()
            .method("GET")
            .uri("/test")
            .header("Origin", "http://localhost:5173")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers()
                .get("access-control-allow-origin")
                .unwrap()
                .to_str()
                .unwrap(),
            "http://localhost:5173"
        );
        assert!(res
            .headers()
            .get("access-control-allow-credentials")
            .is_none());
    }

    #[tokio::test]
    async fn cors_actual_request_adds_credentials_header_when_enabled() {
        let cfg = CorsConfig {
            env: "http://example.com".into(),
            credentials: true,
        };

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(build_cors(&cfg));

        // 実リクエスト（GET）
        let req = Request::builder()
            .method("GET")
            .uri("/test")
            .header("Origin", "http://example.com")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        assert_eq!(
            res.headers()
                .get("access-control-allow-origin")
                .unwrap()
                .to_str()
                .unwrap(),
            "http://example.com"
        );

        assert_eq!(
            res.headers()
                .get("access-control-allow-credentials")
                .unwrap()
                .to_str()
                .unwrap(),
            "true"
        );
    }
}
