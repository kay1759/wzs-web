//! # File Upload Handler
//!
//! Provides an Axum-compatible HTTP endpoint for multipart file uploads,
//! with optional CSRF protection and a unified JSON response format.
//!
//! This module is intentionally structured so that:
//!
//! - the public Axum handler stays small
//! - multipart parsing and upload execution are testable in isolation
//! - HTTP-level tests can verify request/response behavior without touching real storage

use std::sync::Arc;

use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use axum_extra::extract::{cookie::CookieJar, Multipart};
use serde::Serialize;

use crate::config::csrf::CsrfConfig;
use crate::web::csrf;
use crate::web::upload::uploader::{UploadImageParamsInput, UploadService};

/// JSON response returned after a successful upload.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadResp {
    /// Public path corresponding to the stored key.
    path: String,
    /// Original file name received from the multipart field.
    original_filename: String,
    /// Final saved byte size.
    bytes: u64,
    /// Final content type returned by the upload service.
    content_type: String,
}

/// HTTP handler for multipart file uploads.
///
/// Behavior:
///
/// - validates CSRF when enabled
/// - reads the `file` multipart field
/// - optionally reads image resize parameters
/// - delegates the actual upload to [`UploadService`]
/// - returns a JSON response on success
///
/// # Returns
///
/// - `200 OK` with JSON on success
/// - `400 BAD REQUEST` for malformed multipart data or invalid image params
/// - `401 UNAUTHORIZED` when CSRF validation fails
/// - `500 INTERNAL SERVER ERROR` when the upload service fails
pub async fn upload_handler(
    Extension(upload_uc): Extension<Arc<UploadService>>,
    Extension(enable_csrf): Extension<bool>,
    Extension(csrf_cfg): Extension<CsrfConfig>,
    jar: CookieJar,
    headers: HeaderMap,
    multipart: Multipart,
) -> impl IntoResponse {
    if enable_csrf && !csrf::validate_csrf(&headers, &jar, &csrf_cfg) {
        return (StatusCode::UNAUTHORIZED, "CSRF token missing or invalid").into_response();
    }

    run_upload(upload_uc.as_ref(), multipart).await
}

/// A small trait used to make the upload execution path testable.
///
/// The production implementation is [`UploadService`], while tests can provide
/// a lightweight mock implementation without requiring real file storage or
/// image processing.
trait UploadUsecase: Send + Sync {
    /// Performs the upload and returns the upload result on success.
    fn upload(
        &self,
        filename: &str,
        content_type: &str,
        bytes: &[u8],
        image_params: Option<crate::web::upload::uploader::UploadImageParams>,
    ) -> anyhow::Result<crate::web::upload::uploader::UploadResult>;
}

impl UploadUsecase for UploadService {
    fn upload(
        &self,
        filename: &str,
        content_type: &str,
        bytes: &[u8],
        image_params: Option<crate::web::upload::uploader::UploadImageParams>,
    ) -> anyhow::Result<crate::web::upload::uploader::UploadResult> {
        UploadService::upload(self, filename, content_type, bytes, image_params)
    }
}

/// Reads multipart fields, validates image parameters, delegates upload logic,
/// and converts the result into an HTTP response.
///
/// This function contains the main body of the handler so tests can reuse the
/// same logic with a mock upload use case.
async fn run_upload(
    upload_uc: &dyn UploadUsecase,
    mut multipart: Multipart,
) -> axum::response::Response {
    let mut file_name = String::from("upload.bin");
    let mut content_type = String::new();
    let mut file_bytes: Option<Vec<u8>> = None;

    let mut image_params = UploadImageParamsInput::default();

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or_default().to_string();

        match field_name.as_str() {
            "file" => {
                content_type = field
                    .content_type()
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                file_name = field
                    .file_name()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "upload.bin".into());

                match field.bytes().await {
                    Ok(b) => file_bytes = Some(b.to_vec()),
                    Err(e) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            format!("read file body error: {e}"),
                        )
                            .into_response();
                    }
                }
            }
            "maxWidth" => match field.text().await {
                Ok(v) => image_params.max_width = Some(v),
                Err(e) => {
                    return (StatusCode::BAD_REQUEST, format!("read maxWidth error: {e}"))
                        .into_response();
                }
            },
            "maxHeight" => match field.text().await {
                Ok(v) => image_params.max_height = Some(v),
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        format!("read maxHeight error: {e}"),
                    )
                        .into_response();
                }
            },
            "upscale" => match field.text().await {
                Ok(v) => image_params.upscale = Some(v),
                Err(e) => {
                    return (StatusCode::BAD_REQUEST, format!("read upscale error: {e}"))
                        .into_response();
                }
            },
            "resizeMode" => match field.text().await {
                Ok(v) => image_params.resize_mode = Some(v),
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        format!("read resizeMode error: {e}"),
                    )
                        .into_response();
                }
            },
            "background" => match field.text().await {
                Ok(v) => image_params.background = Some(v),
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        format!("read background error: {e}"),
                    )
                        .into_response();
                }
            },
            _ => {
                // Ignore unknown multipart fields for forward compatibility.
            }
        }
    }

    let data = match file_bytes {
        Some(b) => b,
        None => return (StatusCode::BAD_REQUEST, "no file").into_response(),
    };

    let parsed_params = match image_params.parse() {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("invalid image params: {e}"),
            )
                .into_response();
        }
    };

    match upload_uc.upload(&file_name, &content_type, &data, parsed_params) {
        Ok(saved) => {
            let resp = UploadResp {
                path: format!("/{}", saved.key),
                original_filename: file_name,
                bytes: saved.bytes,
                content_type: saved.content_type,
            };
            Json(resp).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("save error: {e}"),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{Arc, Mutex};

    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        routing::post,
        Router,
    };
    use tower::ServiceExt;

    use crate::config::csrf::CsrfConfig;
    use crate::image::processor::{BgColor, ResizeMode};
    use crate::web::upload::uploader::{UploadImageParams, UploadResult};

    /// Mock outcome for the upload use case.
    #[derive(Clone, Debug)]
    enum MockUploadOutcome {
        Ok(UploadResult),
        Err(String),
    }

    /// A lightweight mock upload use case used by HTTP tests.
    struct MockUploadService {
        calls: Mutex<Vec<UploadCall>>,
        outcome: MockUploadOutcome,
    }

    /// Recorded upload invocation.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct UploadCall {
        filename: String,
        content_type: String,
        bytes: Vec<u8>,
        image_params: Option<UploadImageParams>,
    }

    impl MockUploadService {
        /// Creates a successful mock service.
        fn ok(result: UploadResult) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                outcome: MockUploadOutcome::Ok(result),
            }
        }

        /// Creates a failing mock service.
        fn err(message: impl Into<String>) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                outcome: MockUploadOutcome::Err(message.into()),
            }
        }

        /// Returns the recorded calls.
        fn take_calls(&self) -> Vec<UploadCall> {
            self.calls.lock().expect("lock calls").clone()
        }
    }

    impl UploadUsecase for MockUploadService {
        fn upload(
            &self,
            filename: &str,
            content_type: &str,
            bytes: &[u8],
            image_params: Option<UploadImageParams>,
        ) -> anyhow::Result<UploadResult> {
            self.calls.lock().expect("lock calls").push(UploadCall {
                filename: filename.to_string(),
                content_type: content_type.to_string(),
                bytes: bytes.to_vec(),
                image_params,
            });

            match &self.outcome {
                MockUploadOutcome::Ok(v) => Ok(v.clone()),
                MockUploadOutcome::Err(msg) => Err(anyhow::anyhow!(msg.clone())),
            }
        }
    }

    /// Returns a test CSRF configuration.
    fn test_csrf_config() -> CsrfConfig {
        CsrfConfig::from_env_with(|_| None)
    }

    /// Builds a small test app that reuses the same upload execution logic.
    fn make_app_for_test(
        upload_service: Arc<MockUploadService>,
        enable_csrf: bool,
        csrf_cfg: CsrfConfig,
    ) -> Router {
        async fn test_handler(
            Extension(upload_uc): Extension<Arc<MockUploadService>>,
            Extension(enable_csrf): Extension<bool>,
            Extension(csrf_cfg): Extension<CsrfConfig>,
            jar: CookieJar,
            headers: HeaderMap,
            multipart: Multipart,
        ) -> impl IntoResponse {
            if enable_csrf && !crate::web::csrf::validate_csrf(&headers, &jar, &csrf_cfg) {
                return (StatusCode::UNAUTHORIZED, "CSRF token missing or invalid").into_response();
            }

            run_upload(upload_uc.as_ref(), multipart).await
        }

        Router::new()
            .route("/upload", post(test_handler))
            .layer(Extension(upload_service))
            .layer(Extension(enable_csrf))
            .layer(Extension(csrf_cfg))
    }

    /// A simple multipart part description used by tests.
    enum MultipartPart<'a> {
        Text {
            name: &'a str,
            value: &'a str,
        },
        File {
            name: &'a str,
            filename: &'a str,
            content_type: &'a str,
            bytes: &'a [u8],
        },
    }

    /// Builds a raw multipart request body.
    fn make_multipart_body(boundary: &str, parts: &[MultipartPart<'_>]) -> Vec<u8> {
        let mut body = Vec::new();

        for part in parts {
            body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());

            match part {
                MultipartPart::Text { name, value } => {
                    body.extend_from_slice(
                        format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", name)
                            .as_bytes(),
                    );
                    body.extend_from_slice(value.as_bytes());
                    body.extend_from_slice(b"\r\n");
                }
                MultipartPart::File {
                    name,
                    filename,
                    content_type,
                    bytes,
                } => {
                    body.extend_from_slice(
                        format!(
                            "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
                            name, filename
                        )
                        .as_bytes(),
                    );
                    body.extend_from_slice(
                        format!("Content-Type: {}\r\n\r\n", content_type).as_bytes(),
                    );
                    body.extend_from_slice(bytes);
                    body.extend_from_slice(b"\r\n");
                }
            }
        }

        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
        body
    }

    /// Reads the response body as UTF-8 text.
    async fn body_text(resp: axum::response::Response) -> String {
        let bytes = to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("read response body");
        String::from_utf8(bytes.to_vec()).expect("utf8 body")
    }

    /// Returns a successful file upload result.
    fn ok_result() -> UploadResult {
        UploadResult {
            key: "files/202603/test.txt".into(),
            abs_path: "/tmp/files/202603/test.txt".into(),
            bytes: 5,
            content_type: "text/plain".into(),
        }
    }

    /// Returns a successful image upload result.
    fn ok_image_result() -> UploadResult {
        UploadResult {
            key: "images/202603/test.png".into(),
            abs_path: "/tmp/images/202603/test.png".into(),
            bytes: 12,
            content_type: "image/png".into(),
        }
    }

    #[tokio::test]
    async fn upload_handler_uploads_file_without_image_params() {
        let upload_service = Arc::new(MockUploadService::ok(ok_result()));
        let app = make_app_for_test(upload_service.clone(), false, test_csrf_config());

        let boundary = "X-BOUNDARY";
        let body = make_multipart_body(
            boundary,
            &[MultipartPart::File {
                name: "file",
                filename: "hello.txt",
                content_type: "text/plain",
                bytes: b"hello",
            }],
        );

        let req = Request::builder()
            .method("POST")
            .uri("/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);

        let body = body_text(resp).await;
        assert!(body.contains("\"path\":\"/files/202603/test.txt\""));
        assert!(body.contains("\"originalFilename\":\"hello.txt\""));
        assert!(body.contains("\"bytes\":5"));
        assert!(body.contains("\"contentType\":\"text/plain\""));

        let calls = upload_service.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].filename, "hello.txt");
        assert_eq!(calls[0].content_type, "text/plain");
        assert_eq!(calls[0].bytes, b"hello");
        assert_eq!(calls[0].image_params, None);
    }

    #[tokio::test]
    async fn upload_handler_uploads_image_with_resize_params() {
        let upload_service = Arc::new(MockUploadService::ok(ok_image_result()));
        let app = make_app_for_test(upload_service.clone(), false, test_csrf_config());

        let boundary = "X-BOUNDARY";
        let body = make_multipart_body(
            boundary,
            &[
                MultipartPart::Text {
                    name: "maxWidth",
                    value: "800",
                },
                MultipartPart::Text {
                    name: "maxHeight",
                    value: "600",
                },
                MultipartPart::Text {
                    name: "upscale",
                    value: "true",
                },
                MultipartPart::Text {
                    name: "resizeMode",
                    value: "contain",
                },
                MultipartPart::Text {
                    name: "background",
                    value: "#ffffffff",
                },
                MultipartPart::File {
                    name: "file",
                    filename: "photo.png",
                    content_type: "image/png",
                    bytes: b"png-bytes",
                },
            ],
        );

        let req = Request::builder()
            .method("POST")
            .uri("/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);

        let body = body_text(resp).await;
        assert!(body.contains("\"path\":\"/images/202603/test.png\""));
        assert!(body.contains("\"originalFilename\":\"photo.png\""));
        assert!(body.contains("\"bytes\":12"));
        assert!(body.contains("\"contentType\":\"image/png\""));

        let calls = upload_service.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].filename, "photo.png");
        assert_eq!(calls[0].content_type, "image/png");
        assert_eq!(calls[0].bytes, b"png-bytes");
        assert_eq!(
            calls[0].image_params,
            Some(UploadImageParams {
                max_width: 800,
                max_height: 600,
                upscale: true,
                resize_mode: ResizeMode::Contain,
                background: BgColor::white(),
            })
        );
    }

    #[tokio::test]
    async fn upload_handler_returns_bad_request_when_file_is_missing() {
        let upload_service = Arc::new(MockUploadService::ok(ok_result()));
        let app = make_app_for_test(upload_service.clone(), false, test_csrf_config());

        let boundary = "X-BOUNDARY";
        let body = make_multipart_body(
            boundary,
            &[MultipartPart::Text {
                name: "maxWidth",
                value: "800",
            }],
        );

        let req = Request::builder()
            .method("POST")
            .uri("/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body = body_text(resp).await;
        assert_eq!(body, "no file");

        let calls = upload_service.take_calls();
        assert!(calls.is_empty());
    }

    #[tokio::test]
    async fn upload_handler_returns_bad_request_for_invalid_image_params() {
        let upload_service = Arc::new(MockUploadService::ok(ok_result()));
        let app = make_app_for_test(upload_service.clone(), false, test_csrf_config());

        let boundary = "X-BOUNDARY";
        let body = make_multipart_body(
            boundary,
            &[
                MultipartPart::Text {
                    name: "maxWidth",
                    value: "800",
                },
                MultipartPart::File {
                    name: "file",
                    filename: "photo.png",
                    content_type: "image/png",
                    bytes: b"png-bytes",
                },
            ],
        );

        let req = Request::builder()
            .method("POST")
            .uri("/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body = body_text(resp).await;
        assert!(body.contains("invalid image params"));

        let calls = upload_service.take_calls();
        assert!(calls.is_empty());
    }

    #[tokio::test]
    async fn upload_handler_returns_internal_server_error_when_upload_fails() {
        let upload_service = Arc::new(MockUploadService::err("disk full"));
        let app = make_app_for_test(upload_service.clone(), false, test_csrf_config());

        let boundary = "X-BOUNDARY";
        let body = make_multipart_body(
            boundary,
            &[MultipartPart::File {
                name: "file",
                filename: "hello.txt",
                content_type: "text/plain",
                bytes: b"hello",
            }],
        );

        let req = Request::builder()
            .method("POST")
            .uri("/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = body_text(resp).await;
        assert!(body.contains("save error: disk full"));

        let calls = upload_service.take_calls();
        assert_eq!(calls.len(), 1);
    }

    #[tokio::test]
    async fn upload_handler_rejects_when_csrf_enabled_and_token_missing() {
        let upload_service = Arc::new(MockUploadService::ok(ok_result()));
        let app = make_app_for_test(upload_service.clone(), true, test_csrf_config());

        let boundary = "X-BOUNDARY";
        let body = make_multipart_body(
            boundary,
            &[MultipartPart::File {
                name: "file",
                filename: "hello.txt",
                content_type: "text/plain",
                bytes: b"hello",
            }],
        );

        let req = Request::builder()
            .method("POST")
            .uri("/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let body = body_text(resp).await;
        assert_eq!(body, "CSRF token missing or invalid");

        let calls = upload_service.take_calls();
        assert!(calls.is_empty());
    }

    #[tokio::test]
    async fn upload_handler_ignores_unknown_fields() {
        let upload_service = Arc::new(MockUploadService::ok(ok_result()));
        let app = make_app_for_test(upload_service.clone(), false, test_csrf_config());

        let boundary = "X-BOUNDARY";
        let body = make_multipart_body(
            boundary,
            &[
                MultipartPart::Text {
                    name: "unusedField",
                    value: "ignored",
                },
                MultipartPart::File {
                    name: "file",
                    filename: "hello.txt",
                    content_type: "text/plain",
                    bytes: b"hello",
                },
            ],
        );

        let req = Request::builder()
            .method("POST")
            .uri("/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .expect("request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);

        let calls = upload_service.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].filename, "hello.txt");
    }
}
