use std::sync::Arc;

use axum::{
    response::{Html, IntoResponse},
    Extension,
};
use axum_extra::extract::cookie::CookieJar;

use crate::config::csrf::CsrfConfig;
use crate::web::csrf::{generate_csrf_token, set_csrf_cookie};

/// SPA (Single Page Application) entry-point handler with CSRF protection.
///
/// This handler is intentionally **application-agnostic** and provides
/// only technical concerns required for serving an SPA entry HTML:
///
/// - Generate a CSRF token
/// - Store the CSRF token in a cookie
/// - Inject the CSRF token into an HTML template
///
/// It does **not** depend on any business domain concepts
/// (e.g. registration, members, admin).
///
/// # Responsibilities
///
/// - CSRF token generation
/// - CSRF cookie attachment
/// - HTML template token replacement
///
/// # Expected HTML template
///
/// The provided HTML template must contain the placeholder:
///
/// ```text
/// {{ csrf_token }}
/// ```
///
/// which will be replaced with the generated CSRF token.
///
/// # Required Extensions
///
/// The following `Extension`s must be injected into the router:
///
/// - `CsrfConfig`
/// - `Arc<String>` (HTML template string)
///
/// # Example
///
/// ```no_run
/// use axum::{Router, routing::get};
/// use wzs_web::web::spa::spa_entry_handler;
///
/// let app = Router::<()>::new()
///     .nest(
///         "/members",
///         Router::new()
///             .route("/", get(spa_entry_handler))
///             .fallback(spa_entry_handler),
///     );
/// ```
///
/// # Returns
///
/// - An HTML response containing the injected CSRF token
/// - A `Set-Cookie` header storing the CSRF token
pub async fn spa_entry_handler(
    Extension(csrf_cfg): Extension<CsrfConfig>,
    Extension(template_html): Extension<Arc<String>>,
    jar: CookieJar,
) -> impl IntoResponse {
    // Generate a new CSRF token
    let token = generate_csrf_token(&csrf_cfg);

    // Store CSRF token in a cookie
    let jar = set_csrf_cookie(jar, &csrf_cfg, &token);

    // Replace CSRF placeholder in HTML template
    let html_with_token = template_html.replace("{{ csrf_token }}", &token);

    (jar, Html(html_with_token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Extension;
    use axum_extra::extract::cookie::CookieJar;

    fn test_csrf_config() -> CsrfConfig {
        // Deterministic CSRF configuration for testing
        CsrfConfig {
            secret: [0u8; 32],
            cookie_secure: false,
            cookie_http_only: true,
        }
    }

    #[tokio::test]
    async fn spa_entry_handler_replaces_csrf_placeholder() {
        let csrf_cfg = test_csrf_config();
        let template_html = Arc::new("<html><body>{{ csrf_token }}</body></html>".to_string());

        let jar = CookieJar::new();

        let response = spa_entry_handler(Extension(csrf_cfg), Extension(template_html), jar)
            .await
            .into_response();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();

        // Placeholder must be replaced
        assert!(
            !body_str.contains("{{ csrf_token }}"),
            "CSRF token placeholder should be replaced"
        );

        // Token should be injected into the HTML body
        assert!(
            body_str.contains("<body>") && body_str.contains("</body>"),
            "HTML body should contain injected CSRF token"
        );
    }

    #[tokio::test]
    async fn spa_entry_handler_sets_csrf_cookie() {
        let csrf_cfg = test_csrf_config();
        let template_html = Arc::new("{{ csrf_token }}".to_string());

        let jar = CookieJar::new();

        let response = spa_entry_handler(Extension(csrf_cfg), Extension(template_html), jar)
            .await
            .into_response();

        let headers = response.headers();

        let has_csrf_cookie = headers
            .get_all(axum::http::header::SET_COOKIE)
            .iter()
            .any(|v| v.to_str().unwrap_or("").to_lowercase().contains("csrf"));

        assert!(
            has_csrf_cookie,
            "Response should contain a CSRF Set-Cookie header"
        );
    }
}
