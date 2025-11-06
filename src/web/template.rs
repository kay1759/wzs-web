//! # Askama Template Rendering Helpers
//!
//! Provides utility functions for rendering [Askama](https://crates.io/crates/askama)
//! templates into [Axum](https://crates.io/crates/axum) HTML responses.
//!
//! These helpers simplify returning `text/html` responses from route handlers,
//! automatically setting the appropriate content type and handling render errors.
//!
//! # Examples
//! ```rust,no_run
//! use askama::Template;
//! use axum::{response::Response, http::StatusCode};
//! use wzs_web::web::template::{render_template, render_template_with_status};
//!
//! #[derive(Template)]
//! #[template(source = "<h1>Hello {{ name }}</h1>", ext = "html")]
//! struct HelloTemplate<'a> {
//!     name: &'a str,
//! }
//!
//! fn example() -> Response {
//!     let tmpl = HelloTemplate { name: "Alice" };
//!     render_template_with_status(tmpl, StatusCode::OK)
//! }
//! ```

use askama::Template;
use axum::{
    http::{Response, StatusCode},
    response::Response as AxumResponse,
};

/// Renders an [`Askama::Template`] into an HTML [`AxumResponse`].
///
/// On success, returns a response with status `200 OK` and content type `text/html`.
/// On failure (template render error), returns `500 Internal Server Error`.
///
/// # Example
/// ```rust,no_run
/// use askama::Template;
/// use wzs_web::web::template::render_template;
///
/// #[derive(Template)]
/// #[template(source = "<p>{{ name }}</p>", ext = "html")]
/// struct Hello<'a> { name: &'a str }
///
/// let html = Hello { name: "World" };
/// let resp = render_template(html);
/// assert_eq!(resp.status(), axum::http::StatusCode::OK);
/// ```
pub fn render_template<T: Template>(template: T) -> AxumResponse {
    match template.render() {
        Ok(html) => Response::builder()
            .header("Content-Type", "text/html")
            .body(axum::body::Body::from(html))
            .unwrap(),
        Err(_) => Response::builder()
            .status(500)
            .body(axum::body::Body::from("Internal Server Error"))
            .unwrap(),
    }
}

/// Renders an [`Askama::Template`] with a custom HTTP status code.
///
/// This function first renders the template via [`render_template`],
/// then replaces its status code with the given one.
///
/// # Example
/// ```rust,no_run
/// use askama::Template;
/// use axum::http::StatusCode;
/// use wzs_web::web::template::render_template_with_status;
///
/// #[derive(Template)]
/// #[template(source = "<p>{{ msg }}</p>", ext = "html")]
/// struct Message<'a> { msg: &'a str }
///
/// let tmpl = Message { msg: "Created" };
/// let resp = render_template_with_status(tmpl, StatusCode::CREATED);
/// assert_eq!(resp.status(), StatusCode::CREATED);
/// ```
pub fn render_template_with_status<T: Template>(template: T, status: StatusCode) -> AxumResponse {
    let mut resp = render_template(template);
    *resp.status_mut() = status;
    resp
}

#[cfg(test)]
mod tests {
    use askama::Template;
    use axum::http::{header::CONTENT_TYPE, StatusCode};

    use super::*;

    #[derive(Template)]
    #[template(source = "<h1>Hello {{ name }}</h1>", ext = "html")]
    struct HelloTemplate<'a> {
        name: &'a str,
    }

    #[test]
    fn render_template_returns_html_response_on_success() {
        let tmpl = HelloTemplate { name: "Alice" };
        let resp = render_template(tmpl);

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers().get(CONTENT_TYPE).unwrap(), "text/html");

        let body = body_to_string(resp);
        assert!(body.contains("Hello Alice"));
    }

    #[test]
    fn render_template_returns_500_on_render_error() {
        let resp = Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(axum::body::Body::from("Internal Server Error"))
            .unwrap();

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = body_to_string(resp);
        assert!(body.contains("Internal Server Error"));
    }

    #[test]
    fn render_template_with_status_overrides_status_code() {
        let tmpl = HelloTemplate { name: "Bob" };
        let resp = render_template_with_status(tmpl, StatusCode::CREATED);

        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = body_to_string(resp);
        assert!(body.contains("Hello Bob"));
    }

    fn body_to_string(resp: AxumResponse) -> String {
        use futures::executor::block_on;
        use http_body_util::BodyExt;

        let collected = block_on(resp.into_body().collect()).unwrap();
        String::from_utf8(collected.to_bytes().to_vec()).unwrap()
    }
}
