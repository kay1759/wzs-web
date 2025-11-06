//! # CSRF (Cross-Site Request Forgery) Utilities
//!
//! Provides secure CSRF token generation, validation, and cookie handling
//! for [Axum](https://crates.io/crates/axum) applications.
//!
//! Tokens are HMAC-SHA256 signed using a secret from [`CsrfConfig`] and follow the format:
//!
//! ```text
//! v1.<nonce_b64>.<mac_b64>
//! ```
//!
//! - Nonce and MAC are 32 bytes each
//! - Encoded using Base64 (URL-safe, no padding)
//! - Tokens are stored in both a cookie and an HTTP header for verification
//!
//! # Endpoints
//! The included [`csrf_handler`] can be mounted at `/csrf` to issue or refresh CSRF tokens.
//!
//! # Example
//! ```rust,no_run
//! use axum::{Router, routing::get};
//! use wzs_web::web::csrf::{csrf_handler, CSRF_HEADER_NAME, validate_csrf};
//! use wzs_web::config::csrf::CsrfConfig;
//!
//! let cfg = CsrfConfig::from_env();
//! let app: Router = Router::new()
//!     .route("/csrf", get(csrf_handler))
//!     .layer(axum::Extension(cfg));
//!
//! // In a protected handler:
//! // 1. Read header "X-CSRF-Token"
//! // 2. Validate against the cookie
//! ```

use axum::{
    http::{
        header::{CACHE_CONTROL, CONTENT_TYPE},
        HeaderMap, StatusCode,
    },
    Extension, Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::config::csrf::CsrfConfig;

/// Cookie name used to store the CSRF token.
pub const CSRF_COOKIE_NAME: &str = "csrf";

/// HTTP header name used for CSRF verification.
pub const CSRF_HEADER_NAME: &str = "X-CSRF-Token";

type HmacSha256 = Hmac<Sha256>;

/// Generates a new HMAC-signed CSRF token using the configured secret.
///
/// Format: `v1.<nonce>.<mac>` (Base64-URL encoded)
///
/// # Example
/// ```rust
/// use wzs_web::config::csrf::CsrfConfig;
/// use wzs_web::web::csrf::generate_csrf_token;
///
/// let cfg = CsrfConfig::from_env();
/// let token = generate_csrf_token(&cfg);
/// assert!(token.starts_with("v1."));
/// ```
pub fn generate_csrf_token(cfg: &CsrfConfig) -> String {
    let nonce: [u8; 32] = rand::random();
    let mut mac = HmacSha256::new_from_slice(&cfg.secret).expect("HMAC key");
    mac.update(&nonce);
    let tag = mac.finalize().into_bytes();

    format!(
        "v1.{}.{}",
        URL_SAFE_NO_PAD.encode(nonce),
        URL_SAFE_NO_PAD.encode(tag)
    )
}

/// Verifies a CSRF token’s HMAC signature and format.
///
/// Returns `true` if valid, `false` otherwise.
pub fn verify_token(cfg: &CsrfConfig, token: &str) -> bool {
    let mut parts = token.split('.');
    let (Some(v), Some(nonce_b64), Some(mac_b64)) = (parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    if parts.next().is_some() || v != "v1" {
        return false;
    }

    let Ok(nonce) = URL_SAFE_NO_PAD.decode(nonce_b64) else {
        return false;
    };
    let Ok(mac) = URL_SAFE_NO_PAD.decode(mac_b64) else {
        return false;
    };

    if nonce.len() != 32 || mac.len() != 32 {
        return false;
    }

    let Ok(mut h) = HmacSha256::new_from_slice(&cfg.secret) else {
        return false;
    };
    h.update(&nonce);
    let expected = h.finalize().into_bytes();

    (&expected[..]).ct_eq(&mac).unwrap_u8() == 1
}

/// Sets a signed CSRF cookie using configuration flags (`Secure`, `HttpOnly`).
pub fn set_csrf_cookie(jar: CookieJar, cfg: &CsrfConfig, token: &str) -> CookieJar {
    set_csrf_cookie_with_flags(jar, token, cfg.cookie_secure, cfg.cookie_http_only)
}

/// Adds a CSRF cookie with explicit security flags.
pub fn set_csrf_cookie_with_flags(
    jar: CookieJar,
    token: &str,
    secure: bool,
    http_only: bool,
) -> CookieJar {
    let cookie = Cookie::build((CSRF_COOKIE_NAME, token.to_string()))
        .path("/")
        .same_site(SameSite::Lax)
        .secure(secure)
        .http_only(http_only)
        .build();
    jar.add(cookie)
}

/// Validates a CSRF token pair (header + cookie).
///
/// Returns `true` only if both are present, identical, and correctly signed.
///
/// # Example
/// ```rust,no_run
/// use axum_extra::extract::cookie::{Cookie, CookieJar};
/// use axum::http::{HeaderMap, HeaderValue};
/// use wzs_web::config::csrf::CsrfConfig;
/// use wzs_web::web::csrf::{generate_csrf_token, validate_csrf, CSRF_COOKIE_NAME, CSRF_HEADER_NAME};
///
/// let cfg = CsrfConfig::from_env();
/// let token = generate_csrf_token(&cfg);
///
/// let jar = CookieJar::new().add(Cookie::new(CSRF_COOKIE_NAME, token.clone()));
///
/// let mut headers = HeaderMap::new();
/// headers.insert(CSRF_HEADER_NAME, HeaderValue::from_str(&token).unwrap());
///
/// assert!(validate_csrf(&headers, &jar, &cfg));
/// ```
pub fn validate_csrf(headers: &HeaderMap, jar: &CookieJar, cfg: &CsrfConfig) -> bool {
    let Some(header_token) = headers
        .get(CSRF_HEADER_NAME)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
    else {
        return false;
    };
    let Some(cookie_token) = jar.get(CSRF_COOKIE_NAME).map(|c| c.value().to_string()) else {
        return false;
    };

    if header_token
        .as_bytes()
        .ct_eq(cookie_token.as_bytes())
        .unwrap_u8()
        != 1
    {
        return false;
    }

    verify_token(cfg, &cookie_token)
}

/// JSON response schema returned by [`csrf_handler`].
#[derive(Debug, Serialize)]
pub struct CsrfResponse {
    #[serde(rename = "csrfToken")]
    pub csrf_token: String,
}

/// Axum handler that issues or refreshes a CSRF token.
///
/// - If a valid cookie token exists, it is reused.
/// - Otherwise, a new token is generated and set in a `Set-Cookie` header.
/// - The token is also returned as JSON for the frontend.
///
/// # Example
/// ```rust,no_run
/// use axum::{routing::get, Router, Extension};
/// use wzs_web::config::csrf::CsrfConfig;
/// use wzs_web::web::csrf::csrf_handler;
///
/// let cfg = CsrfConfig::from_env();
/// let app: Router = Router::new()
///     .route("/csrf", get(csrf_handler))
///     .layer(Extension(cfg));
/// ```
pub async fn csrf_handler(
    Extension(cfg): Extension<CsrfConfig>,
    jar: CookieJar,
) -> (CookieJar, (StatusCode, HeaderMap, Json<CsrfResponse>)) {
    let token = match jar
        .get(CSRF_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .filter(|t| verify_token(&cfg, t))
    {
        Some(t) => t,
        None => generate_csrf_token(&cfg),
    };

    let jar = set_csrf_cookie(jar, &cfg, &token);

    let mut headers = HeaderMap::new();
    headers.insert(
        CACHE_CONTROL,
        "no-store, no-cache, must-revalidate".parse().unwrap(),
    );
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());

    let json = Json(CsrfResponse {
        csrf_token: token.clone(),
    });

    (jar, (StatusCode::OK, headers, json))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::csrf::derive_secret_from_string;
    use axum::http::{HeaderMap, HeaderValue, StatusCode};
    use axum::Extension;
    use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};

    fn test_cfg() -> CsrfConfig {
        CsrfConfig {
            secret: derive_secret_from_string("test-fixed-secret"),
            cookie_secure: true,
            cookie_http_only: true,
        }
    }

    fn split_and_decode(token: &str) -> (String, Vec<u8>, Vec<u8>) {
        let mut it = token.split('.');
        let v = it.next().unwrap_or_default().to_string();
        let n_b64 = it.next().unwrap_or_default();
        let m_b64 = it.next().unwrap_or_default();
        let nonce = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(n_b64)
            .unwrap();
        let mac = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(m_b64)
            .unwrap();
        (v, nonce, mac)
    }

    #[test]
    fn generate_token_format_and_lengths() {
        let cfg = test_cfg();
        let t = generate_csrf_token(&cfg);

        assert_eq!(t.split('.').count(), 3);
        assert!(!t.contains('='), "no padding expected");

        let (v, nonce, mac) = split_and_decode(&t);
        assert_eq!(v, "v1");
        assert_eq!(nonce.len(), 32, "nonce must be 32 bytes");
        assert_eq!(mac.len(), 32, "HMAC-SHA256 tag must be 32 bytes");
    }

    #[test]
    fn verify_token_accepts_valid_and_rejects_tampered() {
        let cfg = test_cfg();
        let t = generate_csrf_token(&cfg);
        assert!(verify_token(&cfg, &t), "fresh token should be valid");

        let (v, nonce, mut mac) = split_and_decode(&t);
        mac[0] ^= 1;
        let tampered = format!(
            "{}.{}.{}",
            v,
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&nonce),
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&mac)
        );
        assert!(!verify_token(&cfg, &tampered));

        let (_, nonce, mac) = split_and_decode(&t);
        let wrong_v = format!(
            "v2.{}.{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&nonce),
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&mac)
        );
        assert!(!verify_token(&cfg, &wrong_v));
        assert!(!verify_token(&cfg, "v1.only-two-parts"));
        assert!(!verify_token(&cfg, "v1.**invalid**.also-invalid"));
    }

    #[test]
    fn set_cookie_sets_attributes() {
        let cfg = test_cfg();
        let token = generate_csrf_token(&cfg);

        let jar = CookieJar::new();
        let jar = set_csrf_cookie(jar, &cfg, &token);

        let c = jar.get(CSRF_COOKIE_NAME).expect("cookie set");
        assert_eq!(c.value(), token);
        assert_eq!(c.path(), Some("/"));
        assert_eq!(c.same_site(), Some(SameSite::Lax));
        assert_eq!(c.http_only(), Some(true));
        assert_eq!(c.secure(), Some(true));

        let jar2 = CookieJar::new();
        let jar2 = set_csrf_cookie_with_flags(jar2, &token, false, false);
        let c2 = jar2.get(CSRF_COOKIE_NAME).expect("cookie set (flags)");
        assert_eq!(c2.http_only(), Some(false));
        assert_eq!(c2.secure(), Some(false));
    }

    #[test]
    fn validate_csrf_happy_path() {
        let cfg = test_cfg();
        let token = generate_csrf_token(&cfg);

        let jar = CookieJar::new().add(
            Cookie::build((CSRF_COOKIE_NAME, token.clone()))
                .path("/")
                .same_site(SameSite::Lax)
                .secure(true)
                .http_only(true)
                .build(),
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            CSRF_HEADER_NAME,
            HeaderValue::from_str(&token).expect("header"),
        );

        assert!(validate_csrf(&headers, &jar, &cfg));
    }

    #[test]
    fn validate_csrf_rejects_when_header_cookie_mismatch() {
        let cfg = test_cfg();
        let t1 = generate_csrf_token(&cfg);
        let t2 = generate_csrf_token(&cfg);

        let jar = CookieJar::new().add(Cookie::new(CSRF_COOKIE_NAME, t1));

        let mut headers = HeaderMap::new();
        headers.insert(CSRF_HEADER_NAME, HeaderValue::from_str(&t2).unwrap());

        assert!(!validate_csrf(&headers, &jar, &cfg));
    }

    #[test]
    fn validate_csrf_rejects_missing_or_empty_header() {
        let cfg = test_cfg();
        let token = generate_csrf_token(&cfg);
        let jar = CookieJar::new().add(Cookie::new(CSRF_COOKIE_NAME, token));

        let headers = HeaderMap::new();
        assert!(!validate_csrf(&headers, &jar, &cfg));

        let mut headers = HeaderMap::new();
        headers.insert(CSRF_HEADER_NAME, HeaderValue::from_static(""));
        assert!(!validate_csrf(&headers, &jar, &cfg));
    }

    #[test]
    fn validate_csrf_rejects_invalid_signature_even_if_equal() {
        let cfg = test_cfg();

        let bogus = "v1.".to_string()
            + &base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0u8; 32])
            + "."
            + &base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0u8; 32]);

        let jar = CookieJar::new().add(Cookie::new(CSRF_COOKIE_NAME, bogus.clone()));
        let mut headers = HeaderMap::new();
        headers.insert(CSRF_HEADER_NAME, HeaderValue::from_str(&bogus).unwrap());

        assert!(!validate_csrf(&headers, &jar, &cfg));
    }

    #[test]
    fn validate_csrf_rejects_missing_cookie() {
        let cfg = test_cfg();
        let token = generate_csrf_token(&cfg);
        let jar = CookieJar::new(); // Cookie 無し

        let mut headers = HeaderMap::new();
        headers.insert(CSRF_HEADER_NAME, HeaderValue::from_str(&token).unwrap());

        assert!(!validate_csrf(&headers, &jar, &cfg));
    }

    #[tokio::test]
    async fn csrf_handler_sets_cookie_and_returns_token() {
        let cfg = test_cfg();

        let jar = CookieJar::new();
        let (jar_after, (status, headers, _body)) = csrf_handler(Extension(cfg.clone()), jar).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers
                .get(axum::http::header::CACHE_CONTROL)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default(),
            "no-store, no-cache, must-revalidate"
        );
        assert_eq!(
            headers
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default(),
            "application/json"
        );

        let cookie = jar_after.get(CSRF_COOKIE_NAME).expect("csrf cookie set");
        assert_eq!(cookie.path(), Some("/"));
        assert_eq!(cookie.same_site(), Some(SameSite::Lax));
        assert_eq!(cookie.secure(), Some(cfg.cookie_secure));
        assert_eq!(cookie.http_only(), Some(cfg.cookie_http_only));
    }

    #[tokio::test]
    async fn csrf_handler_reuses_valid_cookie() {
        let cfg = test_cfg();

        let preset = generate_csrf_token(&cfg);
        let jar = CookieJar::new().add(
            Cookie::build((CSRF_COOKIE_NAME, preset.clone()))
                .path("/")
                .same_site(SameSite::Lax)
                .secure(cfg.cookie_secure)
                .http_only(cfg.cookie_http_only)
                .build(),
        );

        let (_jar_after, (_status, _headers, body)) =
            csrf_handler(Extension(cfg.clone()), jar).await;

        assert_eq!(body.csrf_token, preset);
        assert!(verify_token(&cfg, &body.csrf_token));
    }

    #[tokio::test]
    async fn csrf_handler_refreshes_when_cookie_invalid() {
        let cfg = test_cfg();

        let invalid = "v1.".to_string()
            + &base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0u8; 32])
            + "."
            + &base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0u8; 32]);

        let jar = CookieJar::new().add(Cookie::new(CSRF_COOKIE_NAME, invalid));

        let (jar_after, (_status, _headers, body)) =
            csrf_handler(Extension(cfg.clone()), jar).await;

        let cookie = jar_after.get(CSRF_COOKIE_NAME).expect("refreshed cookie");
        assert_eq!(cookie.value(), body.csrf_token);
        assert!(verify_token(&cfg, cookie.value()));
    }
}
