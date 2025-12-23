use axum::{http::StatusCode, response::IntoResponse};

/// Default 404 Not Found handler.
///
/// # Overview
///
/// This handler is intended to be used as the final fallback
/// in an Axum router.
///
/// It returns a plain `404 Not Found` response without a body.
///
/// # Design Notes
///
/// - Application-agnostic
/// - Suitable for APIs and SPAs
/// - Can be replaced by application-specific handlers if needed
pub async fn not_found() -> impl IntoResponse {
    StatusCode::NOT_FOUND
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn returns_404_not_found() {
        let response = not_found().await.into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
