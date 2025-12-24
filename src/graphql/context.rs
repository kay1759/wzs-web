use axum::http::HeaderMap;
use axum_extra::extract::cookie::CookieJar;

use crate::auth::jwt::decode_jwt;
use crate::auth::CurrentUser;

/// Extract an authenticated principal (`CurrentUser`) from a JWT stored in a cookie.
///
/// # Overview
///
/// This function is **application-agnostic** and performs only authentication:
///
/// - Reads a JWT from a cookie
/// - Verifies it using the provided secret
/// - Extracts the `sub` (subject) claim
/// - Wraps it in [`CurrentUser`]
///
/// It does **not**:
///
/// - Interpret the subject
/// - Perform authorization
/// - Map the subject to any domain concept (user / member / admin)
///
/// # Parameters
///
/// - `jar`:
///   The cookie jar containing the JWT cookie.
/// - `headers`:
///   Currently unused, but kept for forward compatibility
///   (e.g. Authorization header support).
/// - `jwt_secret`:
///   The secret used to verify the JWT.
///   If `None`, authentication is disabled and this function always returns `None`.
/// - `cookie_name`:
///   The name of the cookie containing the JWT payload.
///
/// # Returns
///
/// - `Some(CurrentUser)` if a valid JWT is found and verified
/// - `None` otherwise
///
/// # Design Notes
///
/// This function represents the **authentication boundary** of `wzs-web`.
/// All authorization and domain-specific interpretation must be done
/// by the application layer.
///
/// # Example
///
/// ```ignore
/// use wzs_web::graphql::context::extract_current_user;
///
/// let user = extract_current_user(
///     &jar,
///     &headers,
///     Some("secret"),
///     "auth_token",
/// );
/// ```
pub fn extract_current_user(
    jar: &CookieJar,
    _headers: &HeaderMap,
    jwt_secret: Option<&str>,
    cookie_name: &str,
) -> Option<CurrentUser> {
    let secret = jwt_secret?;

    jar.get(cookie_name)
        .and_then(|cookie| serde_json::from_str::<serde_json::Value>(cookie.value()).ok())
        .and_then(|value| value.get("token")?.as_str().map(String::from))
        .and_then(|token| decode_jwt(&token, secret).ok())
        .map(|claims| CurrentUser::new(claims.sub))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use axum_extra::extract::cookie::{Cookie, CookieJar};

    use crate::auth::jwt::create_jwt;

    const JWT_SECRET: &str = "unit-test-secret";
    const COOKIE_NAME: &str = "auth_token";

    fn headers() -> HeaderMap {
        HeaderMap::new()
    }

    fn jar_with_token(token: &str) -> CookieJar {
        CookieJar::new().add(Cookie::new(
            COOKIE_NAME,
            format!(r#"{{ "token": "{}" }}"#, token),
        ))
    }

    #[test]
    fn returns_none_when_jwt_secret_is_none() {
        let jar = CookieJar::new();

        let user = extract_current_user(&jar, &headers(), None, COOKIE_NAME);

        assert!(user.is_none());
    }

    #[test]
    fn returns_none_when_cookie_is_missing() {
        let jar = CookieJar::new();

        let user = extract_current_user(&jar, &headers(), Some(JWT_SECRET), COOKIE_NAME);

        assert!(user.is_none());
    }

    #[test]
    fn returns_none_when_jwt_is_invalid() {
        let jar = jar_with_token("invalid.jwt.token");

        let user = extract_current_user(&jar, &headers(), Some(JWT_SECRET), COOKIE_NAME);

        assert!(user.is_none());
    }

    #[test]
    fn returns_current_user_when_jwt_is_valid() {
        let token = create_jwt(42, JWT_SECRET).unwrap();
        let jar = jar_with_token(&token);

        let user = extract_current_user(&jar, &headers(), Some(JWT_SECRET), COOKIE_NAME).unwrap();

        assert_eq!(user.subject, "42");
    }
}
