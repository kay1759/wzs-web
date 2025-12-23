use async_graphql::http::GraphiQLSource;
use axum::response::Html;

/// GraphiQL UI handler.
///
/// # Overview
///
/// Serves the embedded GraphiQL UI for interactive
/// GraphQL exploration.
///
/// # Intended Usage
///
/// - Development and debugging only
/// - Should typically be enabled conditionally (e.g. by environment)
///
/// # Security Note
///
/// This endpoint should **not** be exposed in production
/// unless explicitly intended.
///
/// # Parameters
///
/// - `endpoint` — GraphQL HTTP endpoint path (e.g. `"/graphql"`)
///
/// # Example
///
/// ```no_run
/// use wzs_web::graphql::graphiql::graphiql_handler;
///
/// # async fn example() {
/// let html = graphiql_handler("/graphql").await;
/// # }
/// ```
pub async fn graphiql_handler(endpoint: &str) -> Html<String> {
    Html(GraphiQLSource::build().endpoint(endpoint).finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn graphiql_handler_embeds_endpoint() {
        let endpoint = "/graphql";

        let Html(body) = graphiql_handler(endpoint).await;

        // 基本的な HTML が生成されていること
        assert!(body.contains("<!DOCTYPE html>"));

        // endpoint が埋め込まれていること
        assert!(
            body.contains(endpoint),
            "GraphiQL HTML does not contain endpoint: {endpoint}"
        );
    }

    #[tokio::test]
    async fn graphiql_handler_accepts_custom_endpoint() {
        let endpoint = "/api/graphql";

        let Html(body) = graphiql_handler(endpoint).await;

        assert!(body.contains(endpoint));
    }
}
