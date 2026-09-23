use std::sync::Arc;

use axum::{
    Extension,
    response::{Html, IntoResponse},
};
use axum_extra::extract::cookie::CookieJar;

use crate::config::api::ApiConfig;
use crate::web::csrf::{generate_csrf_token, set_csrf_cookie};

/// SPA (Single Page Application) entry-point handler with CSRF protection.
///
/// This handler is intentionally **application-agnostic** and provides
/// only the technical concerns required for serving an SPA entry HTML:
///
/// - Generate a CSRF token
/// - Store the CSRF token in a cookie
/// - Inject the CSRF token into an HTML template
///
/// It does **not** depend on application-specific concepts such as
/// members, administrators, pickup screens, or registration flows.
///
/// # Configuration
///
/// API-specific configuration is supplied through [`ApiConfig`].
///
/// The handler uses the [`ApiConfig::csrf`] configuration when generating
/// and storing the CSRF token. This keeps configuration injection
/// consistent with the other HTTP handlers provided by `wzs-web`.
///
/// # Responsibilities
///
/// - CSRF token generation
/// - CSRF cookie attachment
/// - HTML template token replacement
///
/// # Expected HTML template
///
/// The provided HTML template may contain the placeholder:
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
/// - [`ApiConfig`]
/// - `Arc<String>` containing the HTML template
///
/// # Example
///
/// ```no_run
/// use std::sync::Arc;
///
/// use axum::{
///     routing::get,
///     Extension,
///     Router,
/// };
/// use wzs_web::config::api::ApiConfig;
/// use wzs_web::web::spa::entry::spa_entry_handler;
///
/// fn build_spa_router(
///     api_config: ApiConfig,
///     html: Arc<String>,
/// ) -> Router {
///     Router::new()
///         .route("/", get(spa_entry_handler))
///         .fallback(spa_entry_handler)
///         .layer(Extension(html))
///         .layer(Extension(api_config))
/// }
/// ```
///
/// # Returns
///
/// - An HTML response containing the injected CSRF token
/// - A `Set-Cookie` header storing the CSRF token
pub async fn spa_entry_handler(
    Extension(api_config): Extension<ApiConfig>,
    Extension(template_html): Extension<Arc<String>>,
    jar: CookieJar,
) -> impl IntoResponse {
    let csrf_cfg = &api_config.csrf;

    // Generate a new CSRF token using the configuration belonging
    // to the API that owns this SPA.
    let token = generate_csrf_token(csrf_cfg);

    // Store the same token in the CSRF cookie.
    let jar = set_csrf_cookie(jar, csrf_cfg, &token);

    // Inject the generated token into the SPA entry HTML.
    let html_with_token = template_html.replace("{{ csrf_token }}", &token);

    (jar, Html(html_with_token))
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::Extension;
    use axum_extra::extract::cookie::CookieJar;

    use crate::config::api::ApiConfig;
    use crate::config::env::EnvConfig;

    /// Construct an API configuration suitable for SPA handler tests.
    ///
    /// Configuration is created through the same public API used by
    /// applications. This avoids duplicating the internal structure of
    /// `ApiConfig` in tests.
    fn test_api_config() -> ApiConfig {
        let env = EnvConfig::from_pairs([
            ("TEST_CSRF_SECRET", "test-csrf-secret"),
            ("TEST_CSRF_COOKIE_SECURE", "false"),
            ("TEST_CSRF_COOKIE_HTTP_ONLY", "true"),
        ]);

        ApiConfig::from_prefixed_env(&env, "TEST_", "test_token")
    }

    #[tokio::test]
    async fn spa_entry_handler_replaces_csrf_placeholder() {
        let api_config = test_api_config();

        let template_html = Arc::new("<html><body>{{ csrf_token }}</body></html>".to_string());

        let jar = CookieJar::new();

        let response = spa_entry_handler(Extension(api_config), Extension(template_html), jar)
            .await
            .into_response();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();

        let body_str = std::str::from_utf8(&body).unwrap();

        // The template placeholder must not remain in the response.
        assert!(
            !body_str.contains("{{ csrf_token }}"),
            "CSRF token placeholder should be replaced",
        );

        // The generated token must have been injected into the HTML.
        assert!(
            body_str.contains("<body>") && body_str.contains("</body>"),
            "HTML body should contain injected CSRF token",
        );
    }

    #[tokio::test]
    async fn spa_entry_handler_sets_csrf_cookie() {
        let api_config = test_api_config();

        let template_html = Arc::new("{{ csrf_token }}".to_string());

        let jar = CookieJar::new();

        let response = spa_entry_handler(Extension(api_config), Extension(template_html), jar)
            .await
            .into_response();

        let headers = response.headers();

        let has_csrf_cookie = headers
            .get_all(axum::http::header::SET_COOKIE)
            .iter()
            .any(|value| value.to_str().unwrap_or("").to_lowercase().contains("csrf"));

        assert!(
            has_csrf_cookie,
            "Response should contain a CSRF Set-Cookie header",
        );
    }
}
