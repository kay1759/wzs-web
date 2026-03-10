//! # File Upload Handler
//!
//! Provides an Axum-compatible HTTP endpoint for multipart file uploads,
//! supporting optional CSRF protection and unified response format.

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

/// JSON response returned after a successful file upload.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadResp {
    path: String,
    original_filename: String,
    bytes: u64,
    content_type: String,
}

pub async fn upload_handler(
    Extension(upload_uc): Extension<Arc<UploadService>>,
    Extension(enable_csrf): Extension<bool>,
    Extension(csrf_cfg): Extension<CsrfConfig>,
    jar: CookieJar,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> impl IntoResponse {
    if enable_csrf && !csrf::validate_csrf(&headers, &jar, &csrf_cfg) {
        return (StatusCode::UNAUTHORIZED, "CSRF token missing or invalid").into_response();
    }

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
        body::Body,
        http::{Request, StatusCode},
        routing::post,
        Router,
    };
    use tower::ServiceExt;

    use crate::config::csrf::CsrfConfig;
    use crate::image::processor::{BgColor, ResizeMode};
    use crate::web::upload::uploader::{UploadImageParams, UploadResult};

    #[derive(Clone, Debug)]
    enum MockUploadOutcome {
        Ok(UploadResult),
        Err(String),
    }

    struct MockUploadService {
        calls: Mutex<Vec<UploadCall>>,
        outcome: MockUploadOutcome,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct UploadCall {
        filename: String,
        content_type: String,
        bytes: Vec<u8>,
        image_params: Option<UploadImageParams>,
    }

    impl MockUploadService {
        fn ok(result: UploadResult) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                outcome: MockUploadOutcome::Ok(result),
            }
        }

        fn err(message: impl Into<String>) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                outcome: MockUploadOutcome::Err(message.into()),
            }
        }

        fn take_calls(&self) -> Vec<UploadCall> {
            self.calls.lock().expect("lock calls").clone()
        }

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

    fn test_csrf_config() -> CsrfConfig {
        CsrfConfig::from_env_with(|_| None)
    }

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
            mut multipart: Multipart,
        ) -> impl IntoResponse {
            if enable_csrf && !crate::web::csrf::validate_csrf(&headers, &jar, &csrf_cfg) {
                return (StatusCode::UNAUTHORIZED, "CSRF token missing or invalid").into_response();
            }

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
                    _ => {}
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

        Router::new()
            .route("/upload", post(test_handler))
            .layer(Extension(upload_service))
            .layer(Extension(enable_csrf))
            .layer(Extension(csrf_cfg))
    }

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

    fn ok_result() -> UploadResult {
        UploadResult {
            key: "files/202603/test.txt".into(),
            abs_path: "/tmp/files/202603/test.txt".into(),
            bytes: 5,
            content_type: "text/plain".into(),
        }
    }

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

        let calls = upload_service.take_calls();
        assert!(calls.is_empty());
    }
}
