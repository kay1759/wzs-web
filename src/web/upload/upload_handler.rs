//! # File Upload Handler
//!
//! Provides an Axum-compatible HTTP endpoint for multipart file uploads,
//! supporting optional CSRF protection and unified response format.
//!
//! ## Features
//! - Accepts `multipart/form-data` requests with file fields
//! - Integrates with [`UploadService`] for storage and image processing
//! - Supports CSRF validation when enabled via [`CsrfConfig`]
//! - Returns structured JSON response (`path`, `filename`, `bytes`, `content_type`)
//!
//! ## Example
//! ```rust,ignore
//! use axum::{Router, routing::post, Extension};
//! use std::sync::Arc;
//! use wzs_web::web::upload::uploader::UploadService;
//! use wzs_web::web::upload::upload_handler;
//!
//! let upload_service = Arc::new(UploadService::default_stub());
//!
//! let app = Router::new()
//!     .route("/api/upload", post(upload_handler))
//!     .layer(Extension(upload_service))
//! ```

use std::sync::Arc;

use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use axum_extra::extract::{cookie::CookieJar, Multipart};
use serde::Serialize;

use crate::config::csrf::CsrfConfig; // ★ 追加
use crate::web::csrf;
use crate::web::upload::uploader::UploadService;

/// JSON response returned after a successful file upload.
#[derive(Serialize)]
struct UploadResp {
    /// Path (usually relative to `/uploads/` or media root)
    path: String,
    /// Original filename provided by the client
    original_filename: String,
    /// File size in bytes
    bytes: u64,
    /// Content type detected from the multipart field
    content_type: String,
}

/// Axum handler that processes multipart uploads.
///
/// This endpoint accepts a multipart request containing a file field,
/// performs optional CSRF validation, and saves the file using [`UploadService`].
///
/// ## Behavior
/// - When CSRF is enabled, the handler validates both the cookie and header token.
/// - Each file field is processed once; the first valid file ends the response.
/// - Returns a JSON body with saved file information on success.
/// - Returns `400` or `401` on error.
///
/// ## Returns
/// - `200 OK` with JSON if upload succeeded
/// - `401 UNAUTHORIZED` if CSRF is invalid or missing
/// - `400 BAD REQUEST` if no valid file was found or body is malformed
/// - `500 INTERNAL SERVER ERROR` for write errors
///
/// ## Example
/// ```text
/// POST /api/upload
/// Content-Type: multipart/form-data; boundary=----
///
/// ----
/// Content-Disposition: form-data; name="file"; filename="hello.txt"
/// Content-Type: text/plain
///
/// hello world
/// ----
/// ```
pub async fn upload_handler(
    Extension(upload_uc): Extension<Arc<UploadService>>,
    Extension(enable_csrf): Extension<bool>,
    Extension(csrf_cfg): Extension<CsrfConfig>,
    jar: CookieJar,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // --- CSRF Validation ---
    if enable_csrf && !csrf::validate_csrf(&headers, &jar, &csrf_cfg) {
        return (StatusCode::UNAUTHORIZED, "CSRF token missing or invalid").into_response();
    }

    // --- Multipart parsing ---
    while let Ok(Some(field)) = multipart.next_field().await {
        let ct = field
            .content_type()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let fname = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "upload.bin".into());

        let data = match field.bytes().await {
            Ok(b) => b,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("read body error: {e}")).into_response();
            }
        };

        match upload_uc.upload(&fname, &ct, &data) {
            Ok((key, _abs, n, out_ct)) => {
                let resp = UploadResp {
                    path: format!("/{}", key),
                    original_filename: fname,
                    bytes: n,
                    content_type: out_ct,
                };
                return Json(resp).into_response();
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("save error: {e}"),
                )
                    .into_response();
            }
        }
    }

    (StatusCode::BAD_REQUEST, "no file").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
        routing::post,
        Extension, Router,
    };
    use http_body_util::BodyExt;
    use serde_json::Value as Json;
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::config::csrf::derive_secret_from_string;
    use crate::image::processor::{ImageProcessor, ResizeOpts};
    use crate::web::upload::{
        storage::FileStorage,
        uploader::{MediaDirs, UploadService},
    };

    #[derive(Default)]
    struct StubStorage;
    impl FileStorage for StubStorage {
        fn save(&self, rel_path: &str, _bytes: &[u8]) -> anyhow::Result<String> {
            Ok(format!("/abs/{}", rel_path))
        }
    }

    #[derive(Default)]
    struct StubImage;
    impl ImageProcessor for StubImage {
        fn is_supported(&self, _content_type: &str) -> bool {
            true
        }
        fn resize_same_format(
            &self,
            img_bytes: &[u8],
            _content_type: &str,
            _max_w: u32,
            _max_h: u32,
        ) -> anyhow::Result<Vec<u8>> {
            Ok(img_bytes.to_vec())
        }
    }

    fn make_upload_uc() -> Arc<UploadService> {
        let storage: Arc<dyn FileStorage> = Arc::new(StubStorage::default());
        let image: Arc<dyn ImageProcessor> = Arc::new(StubImage::default());
        Arc::new(UploadService::with_dirs(
            storage,
            image,
            MediaDirs {
                image_dir: "images".into(),
                file_dir: "files".into(),
            },
            ResizeOpts {
                max_w: 1280,
                max_h: 1280,
            },
        ))
    }

    fn test_csrf_cfg() -> CsrfConfig {
        CsrfConfig {
            secret: derive_secret_from_string("test-fixed-secret"),
            cookie_secure: true,
            cookie_http_only: true,
        }
    }

    fn build_router(enable_csrf: bool, csrf_cfg: CsrfConfig) -> Router {
        Router::new()
            .route("/api/upload", post(super::upload_handler))
            .layer(Extension(make_upload_uc()))
            .layer(Extension(enable_csrf))
            .layer(Extension(csrf_cfg))
    }

    fn build_multipart(
        boundary: &str,
        name: &str,
        filename: &str,
        content_type: &str,
        data: &[u8],
    ) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
                name, filename
            )
            .as_bytes(),
        );
        body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", content_type).as_bytes());
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
        body
    }

    #[tokio::test]
    async fn upload_succeeds_without_csrf_when_disabled() {
        let app = build_router(false, test_csrf_cfg());

        let boundary = "XBOUND";
        let bytes = build_multipart(boundary, "file", "hello.txt", "text/plain", b"world");
        let req = Request::builder()
            .method("POST")
            .uri("/api/upload")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(bytes))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let json: Json = serde_json::from_slice(&body).unwrap();

        assert_eq!(json.get("original_filename").unwrap(), "hello.txt");
        assert_eq!(json.get("bytes").unwrap(), 5);
        assert_eq!(json.get("content_type").unwrap(), "text/plain");
        assert!(json.get("path").unwrap().as_str().unwrap().starts_with("/"));
    }

    #[tokio::test]
    async fn upload_blocks_without_csrf_when_enabled() {
        let app = build_router(true, test_csrf_cfg());

        let boundary = "XBOUND";
        let bytes = build_multipart(boundary, "file", "hello.txt", "text/plain", b"world");
        let req = Request::builder()
            .method("POST")
            .uri("/api/upload")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(bytes))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8_lossy(&body);
        assert!(s.contains("CSRF token missing or invalid"));
    }

    #[tokio::test]
    async fn upload_allows_with_valid_csrf_when_enabled() {
        use crate::web::csrf as csrf_mod;

        let csrf_cfg = test_csrf_cfg();
        let token = csrf_mod::generate_csrf_token(&csrf_cfg);

        let app = build_router(true, csrf_cfg.clone());

        let boundary = "XBOUND";
        let bytes = build_multipart(boundary, "file", "hello.txt", "text/plain", b"world");

        let req = Request::builder()
            .method("POST")
            .uri("/api/upload")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={}", boundary),
            )
            .header(
                header::COOKIE,
                format!("{}={}", csrf_mod::CSRF_COOKIE_NAME, token),
            )
            .header(csrf_mod::CSRF_HEADER_NAME, token)
            .body(Body::from(bytes))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let json: Json = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.get("original_filename").unwrap(), "hello.txt");
        assert_eq!(json.get("bytes").unwrap(), 5);
    }

    #[tokio::test]
    async fn upload_returns_400_when_no_file() {
        let app = build_router(false, test_csrf_cfg());

        let boundary = "XBOUND";
        let bytes = format!("--{}--\r\n", boundary).into_bytes();

        let req = Request::builder()
            .method("POST")
            .uri("/api/upload")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(bytes))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8_lossy(&body);
        assert!(s.contains("no file"), "actual body: {}", s);
    }

    #[tokio::test]
    async fn upload_returns_400_when_read_error() {
        let app = build_router(false, test_csrf_cfg());

        let req = Request::builder()
            .method("POST")
            .uri("/api/upload")
            .header(header::CONTENT_TYPE, "multipart/form-data; boundary=BAD")
            .body(Body::from("not a valid multipart body"))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();

        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
