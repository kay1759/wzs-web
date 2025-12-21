use axum::http::HeaderMap;
use axum_extra::extract::cookie::CookieJar;

use crate::auth::jwt::decode_jwt;

/// Extract subject ID from JWT cookie.
///
/// This function is application-agnostic.
/// Cookie name and ID parsing are delegated to the caller.
pub fn extract_jwt_subject<F, T>(
    jar: &CookieJar,
    headers: &HeaderMap,
    jwt_secret: Option<&str>,
    cookie_name: &str,
    parse: F,
) -> Option<T>
where
    F: Fn(&str) -> Option<T>,
{
    let secret = jwt_secret?;

    jar.get(cookie_name)
        .and_then(|c| serde_json::from_str::<serde_json::Value>(c.value()).ok())
        .and_then(|v| v.get("token")?.as_str().map(String::from))
        .and_then(|token| decode_jwt(&token, secret).ok())
        .and_then(|claims| parse(&claims.sub))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use axum_extra::extract::cookie::{Cookie, CookieJar};

    use crate::auth::jwt::create_jwt;

    const JWT_SECRET: &str = "unit-test-secret";
    const COOKIE_NAME: &str = "auth_token";

    fn empty_headers() -> HeaderMap {
        HeaderMap::new()
    }

    fn empty_jar() -> CookieJar {
        CookieJar::new()
    }

    #[test]
    fn returns_none_when_secret_is_missing() {
        let jar = empty_jar();
        let headers = empty_headers();

        let result: Option<i64> = extract_jwt_subject(&jar, &headers, None, COOKIE_NAME, |sub| {
            sub.parse::<i64>().ok()
        });

        assert!(result.is_none());
    }

    #[test]
    fn returns_none_when_cookie_is_missing() {
        let jar = empty_jar();
        let headers = empty_headers();

        let result: Option<i64> =
            extract_jwt_subject(&jar, &headers, Some(JWT_SECRET), COOKIE_NAME, |sub| {
                sub.parse::<i64>().ok()
            });

        assert!(result.is_none());
    }

    #[test]
    fn returns_none_when_cookie_value_is_not_json() {
        let jar = CookieJar::new().add(Cookie::new(COOKIE_NAME, "not-json"));
        let headers = empty_headers();

        let result: Option<i64> =
            extract_jwt_subject(&jar, &headers, Some(JWT_SECRET), COOKIE_NAME, |sub| {
                sub.parse::<i64>().ok()
            });

        assert!(result.is_none());
    }

    #[test]
    fn returns_none_when_token_field_is_missing() {
        let jar = CookieJar::new().add(Cookie::new(COOKIE_NAME, r#"{ "unexpected": "value" }"#));
        let headers = empty_headers();

        let result: Option<i64> =
            extract_jwt_subject(&jar, &headers, Some(JWT_SECRET), COOKIE_NAME, |sub| {
                sub.parse::<i64>().ok()
            });

        assert!(result.is_none());
    }

    #[test]
    fn returns_none_when_jwt_is_invalid() {
        let jar = CookieJar::new().add(Cookie::new(
            COOKIE_NAME,
            r#"{ "token": "invalid.jwt.token" }"#,
        ));
        let headers = empty_headers();

        let result: Option<i64> =
            extract_jwt_subject(&jar, &headers, Some(JWT_SECRET), COOKIE_NAME, |sub| {
                sub.parse::<i64>().ok()
            });

        assert!(result.is_none());
    }

    #[test]
    fn returns_subject_when_jwt_is_valid() {
        let token = create_jwt(42, JWT_SECRET).unwrap();

        let jar = CookieJar::new().add(Cookie::new(
            COOKIE_NAME,
            format!(r#"{{ "token": "{}" }}"#, token),
        ));
        let headers = empty_headers();

        let result: Option<i64> =
            extract_jwt_subject(&jar, &headers, Some(JWT_SECRET), COOKIE_NAME, |sub| {
                sub.parse::<i64>().ok()
            });

        assert_eq!(result, Some(42));
    }

    #[test]
    fn returns_none_when_subject_parsing_fails() {
        let token = create_jwt(42, JWT_SECRET).unwrap();

        let jar = CookieJar::new().add(Cookie::new(
            COOKIE_NAME,
            format!(r#"{{ "token": "{}" }}"#, token),
        ));
        let headers = empty_headers();

        let result: Option<()> = extract_jwt_subject(
            &jar,
            &headers,
            Some(JWT_SECRET),
            COOKIE_NAME,
            |_| None, // intentionally fail parsing
        );

        assert!(result.is_none());
    }
}
