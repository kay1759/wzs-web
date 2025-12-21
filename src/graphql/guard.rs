use async_graphql::{Response, ServerError};
use axum::http::HeaderMap;
use axum_extra::extract::cookie::CookieJar;

use crate::auth::jwt::decode_jwt;
use crate::config::csrf::CsrfConfig;
use crate::web::csrf;

/// Validate CSRF token for a GraphQL request.
///
/// This function performs CSRF validation only.
/// JWT validation is intentionally separated to keep responsibilities clear.
///
/// # Arguments
/// - `enable_csrf`: Whether CSRF validation is enabled
/// - `headers`: HTTP request headers
/// - `jar`: Cookie jar extracted from the request
/// - `csrf_cfg`: CSRF configuration
///
/// # Returns
/// - `Ok(())` if validation passes or CSRF is disabled
/// - `Err(Response)` if CSRF validation fails
pub fn validate_csrf_guard(
    enable_csrf: bool,
    headers: &HeaderMap,
    jar: &CookieJar,
    csrf_cfg: &CsrfConfig,
) -> Result<(), Response> {
    if enable_csrf && !csrf::validate_csrf(headers, jar, csrf_cfg) {
        let err = ServerError::new("CSRF token missing or invalid", None);
        return Err(Response::from_errors(vec![err]));
    }

    Ok(())
}

/// Validate a JWT stored in a cookie and extract its subject.
///
/// This function is application-agnostic:
/// - Cookie name is supplied by the caller
/// - Subject parsing is delegated to the caller
///
/// # Arguments
/// - `jar`: Cookie jar extracted from the request
/// - `jwt_secret`: Secret key used to validate the JWT
/// - `cookie_name`: Name of the cookie storing the JWT JSON payload
/// - `parse_subject`: Closure to parse the `sub` claim into a domain type
///
/// # Returns
/// - `Some(T)` if JWT exists and is valid
/// - `None` if JWT is missing, invalid, or parsing fails
///
/// # Example
/// ```ignore
/// let member_id: Option<i64> = validate_jwt_guard(
///     &jar,
///     jwt_secret.as_deref(),
///     "wizis_token",
///     |sub| sub.parse::<i64>().ok(),
/// );
/// ```
pub fn validate_jwt_guard<T, F>(
    jar: &CookieJar,
    jwt_secret: Option<&str>,
    cookie_name: &str,
    parse_subject: F,
) -> Option<T>
where
    F: Fn(&str) -> Option<T>,
{
    let secret = jwt_secret?;

    let cookie = jar.get(cookie_name)?;
    let json = serde_json::from_str::<serde_json::Value>(cookie.value()).ok()?;
    let token = json.get("token")?.as_str()?;

    let claims = decode_jwt(token, secret).ok()?;
    parse_subject(&claims.sub)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use axum_extra::extract::cookie::{Cookie, CookieJar};

    use crate::auth::jwt::create_jwt;
    use crate::config::csrf::CsrfConfig;

    const JWT_SECRET: &str = "unit-test-secret";

    fn empty_headers() -> HeaderMap {
        HeaderMap::new()
    }

    fn empty_jar() -> CookieJar {
        CookieJar::new()
    }

    fn test_csrf_config() -> CsrfConfig {
        // Deterministic secret for testing purposes.
        // The actual value does not matter as long as it is 32 bytes.
        CsrfConfig {
            secret: [0u8; 32],
            cookie_secure: false,
            cookie_http_only: true,
        }
    }

    // ----------------------------
    // CSRF guard tests
    // ----------------------------

    #[test]
    fn csrf_guard_passes_when_disabled() {
        let headers = empty_headers();
        let jar = empty_jar();
        let cfg = test_csrf_config();

        let result = validate_csrf_guard(false, &headers, &jar, &cfg);
        assert!(result.is_ok());
    }

    #[test]
    fn csrf_guard_fails_when_enabled_and_token_is_missing() {
        let headers = empty_headers();
        let jar = empty_jar();
        let cfg = test_csrf_config();

        let result = validate_csrf_guard(true, &headers, &jar, &cfg);
        assert!(result.is_err());

        let response = result.err().unwrap();
        assert!(!response.errors.is_empty());

        assert!(
            response.errors[0].message.to_lowercase().contains("csrf"),
            "expected CSRF error message"
        );
    }

    // ----------------------------
    // JWT guard tests
    // ----------------------------

    #[test]
    fn jwt_guard_returns_none_when_secret_is_missing() {
        let jar = empty_jar();

        let result: Option<i64> =
            validate_jwt_guard(&jar, None, "wizis_token", |sub| sub.parse().ok());

        assert!(result.is_none());
    }

    #[test]
    fn jwt_guard_returns_none_when_cookie_is_missing() {
        let jar = empty_jar();

        let result: Option<i64> =
            validate_jwt_guard(&jar, Some(JWT_SECRET), "wizis_token", |sub| {
                sub.parse().ok()
            });

        assert!(result.is_none());
    }

    #[test]
    fn jwt_guard_returns_none_when_cookie_is_malformed() {
        let jar = CookieJar::new().add(Cookie::new("wizis_token", "not-json"));

        let result: Option<i64> =
            validate_jwt_guard(&jar, Some(JWT_SECRET), "wizis_token", |sub| {
                sub.parse().ok()
            });

        assert!(result.is_none());
    }

    #[test]
    fn jwt_guard_returns_none_when_token_is_invalid() {
        let jar = CookieJar::new().add(Cookie::new(
            "wizis_token",
            r#"{ "token": "invalid.jwt.token" }"#,
        ));

        let result: Option<i64> =
            validate_jwt_guard(&jar, Some(JWT_SECRET), "wizis_token", |sub| {
                sub.parse().ok()
            });

        assert!(result.is_none());
    }

    #[test]
    fn jwt_guard_returns_subject_when_token_is_valid() {
        let token = create_jwt(42, JWT_SECRET).unwrap();

        let jar = CookieJar::new().add(Cookie::new(
            "wizis_token",
            format!(r#"{{ "token": "{}" }}"#, token),
        ));

        let result: Option<i64> =
            validate_jwt_guard(&jar, Some(JWT_SECRET), "wizis_token", |sub| {
                sub.parse::<i64>().ok()
            });

        assert_eq!(result, Some(42));
    }

    #[test]
    fn jwt_guard_returns_none_when_subject_parsing_fails() {
        let token = create_jwt(42, JWT_SECRET).unwrap();

        let jar = CookieJar::new().add(Cookie::new(
            "wizis_token",
            format!(r#"{{ "token": "{}" }}"#, token),
        ));

        let result: Option<()> =
            validate_jwt_guard(&jar, Some(JWT_SECRET), "wizis_token", |_| None);

        assert!(result.is_none());
    }
}
