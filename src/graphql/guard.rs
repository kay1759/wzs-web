use async_graphql::{Response, ServerError};
use axum::http::HeaderMap;
use axum_extra::extract::cookie::CookieJar;

use crate::config::csrf::CsrfConfig;
use crate::web::csrf;

/// Validate CSRF token for a GraphQL request.
///
/// This function performs CSRF validation only.
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
// Returning `async_graphql::Response` directly keeps this guard convenient
// for GraphQL handlers. The error path is only used when CSRF validation
// fails, so boxing the response would add API complexity for little benefit.
#[allow(clippy::result_large_err)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use axum_extra::extract::cookie::CookieJar;

    use crate::config::csrf::{CsrfConfig, DEFAULT_CSRF_COOKIE_NAME};

    fn empty_headers() -> HeaderMap {
        HeaderMap::new()
    }

    fn empty_jar() -> CookieJar {
        CookieJar::new()
    }

    fn test_csrf_config() -> CsrfConfig {
        // Keep every value deterministic so failures in guard behavior are
        // independent of environment variables and random secret generation.
        CsrfConfig {
            secret: [0u8; 32],
            cookie_name: DEFAULT_CSRF_COOKIE_NAME.to_string(),
            cookie_secure: false,
            cookie_http_only: true,
        }
    }

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
}
