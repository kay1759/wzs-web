use async_graphql::{ObjectType, Schema, SubscriptionType};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::http::HeaderMap;
use axum::Extension;
use axum_extra::extract::cookie::CookieJar;

use crate::auth::CurrentUser;
use crate::config::csrf::CsrfConfig;
use crate::graphql::config::GraphqlAuthConfig;
use crate::graphql::context::extract_current_user;
use crate::graphql::guard::validate_csrf_guard;

/// GraphQL POST endpoint handler.
///
/// # Overview
///
/// This handler provides a **reusable, application-agnostic**
/// implementation of a GraphQL POST endpoint for Axum.
///
/// It is responsible **only for infrastructure-level concerns**
/// that are common across applications:
///
/// - CSRF validation
/// - Authentication (JWT extraction)
/// - Injecting authentication context
///
/// All domain logic, authorization rules, and error semantics
/// must be handled by GraphQL resolvers.
///
/// # Responsibilities
///
/// - Validate CSRF tokens when CSRF protection is enabled
/// - Extract a JWT from cookies
/// - Authenticate the request and build `CurrentUser`
/// - Inject `Option<CurrentUser>` into the GraphQL context
///
/// # Non-Responsibilities
///
/// - Authorization (roles, permissions, access control)
/// - Interpreting the meaning of the authenticated subject
/// - Shaping application-specific error responses
///
/// # Authentication Model
///
/// - `Some(CurrentUser)` is injected when authentication succeeds
/// - `None` is injected when authentication is disabled or fails
///
/// This allows resolvers to explicitly distinguish between
/// *authenticated* and *unauthenticated* requests using the type system.
///
/// # Type Parameters
///
/// - `Q`: GraphQL query root
/// - `M`: GraphQL mutation root
/// - `S`: GraphQL subscription root
///
/// All type parameters must satisfy `Send + Sync + 'static`
/// to meet `async-graphql` execution requirements.
pub async fn graphql_post_handler<Q, M, S>(
    Extension(schema): Extension<Schema<Q, M, S>>,
    Extension(enable_csrf): Extension<bool>,
    Extension(csrf_cfg): Extension<CsrfConfig>,
    Extension(jwt_secret): Extension<Option<String>>,
    Extension(auth_cfg): Extension<GraphqlAuthConfig>,
    jar: CookieJar,
    headers: HeaderMap,
    req: GraphQLRequest,
) -> GraphQLResponse
where
    Q: ObjectType + Send + Sync + 'static,
    M: ObjectType + Send + Sync + 'static,
    S: SubscriptionType + Send + Sync + 'static,
{
    // -----------------------------
    // CSRF validation
    // -----------------------------
    //
    // When CSRF protection is enabled, validate the request
    // headers and cookies. On failure, return a GraphQL-
    // compliant error response (HTTP 200 with `errors`).
    if let Err(resp) = validate_csrf_guard(enable_csrf, &headers, &jar, &csrf_cfg) {
        return resp.into();
    }

    // -----------------------------
    // Authentication (JWT → CurrentUser)
    // -----------------------------
    //
    // Extract an authenticated principal from the JWT cookie.
    // This step is intentionally application-agnostic: only the
    // JWT subject is extracted and wrapped in `CurrentUser`.
    let current_user: Option<CurrentUser> = extract_current_user(
        &jar,
        &headers,
        jwt_secret.as_deref(),
        &auth_cfg.jwt_cookie_name,
    );

    // -----------------------------
    // Execute GraphQL with injected context
    // -----------------------------
    //
    // The authentication result is injected into the GraphQL
    // execution context, allowing resolvers to decide how to
    // handle authenticated vs unauthenticated requests.
    schema
        .execute(req.into_inner().data(current_user))
        .await
        .into()
}

#[tokio::test]
async fn graphql_handler_executes_query() {
    use async_graphql::{EmptyMutation, EmptySubscription, Object, Schema};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::{routing::post, Extension, Router};
    use tower::ServiceExt; // oneshot

    struct Query;

    #[Object]
    impl Query {
        async fn dummy(&self) -> &str {
            "ok"
        }
    }

    let schema = Schema::build(Query, EmptyMutation, EmptySubscription).finish();

    let app = Router::new()
        .route(
            "/graphql",
            post(graphql_post_handler::<Query, EmptyMutation, EmptySubscription>),
        )
        .layer(Extension(schema))
        .layer(Extension(false)) // CSRF disabled
        .layer(Extension(CsrfConfig::from_env_with(|_| None)))
        .layer(Extension(None::<String>))
        .layer(Extension(GraphqlAuthConfig::new("auth")));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/graphql")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"query":"{ dummy }"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
