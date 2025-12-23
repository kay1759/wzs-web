use async_graphql::Schema;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::http::HeaderMap;
use axum::Extension;
use axum_extra::extract::cookie::CookieJar;

use crate::auth::CurrentUser;
use crate::config::csrf::CsrfConfig;
use crate::graphql::config::GraphqlAuthConfig;
use crate::graphql::context::extract_current_user;
use crate::graphql::guard::validate_csrf_guard;

/// Generic GraphQL endpoint handler.
///
/// # Overview
///
/// This handler provides a reusable, application-agnostic GraphQL endpoint
/// implementation intended for use in multiple projects.
///
/// It is designed to handle **cross-cutting infrastructure concerns only**,
/// such as CSRF protection and authentication, and delegates all
/// authorization and domain logic to GraphQL resolvers.
///
/// # Responsibilities
///
/// - Validate CSRF tokens when enabled
/// - Extract a JWT from cookies and authenticate the request
/// - Inject the authentication result (`Option<CurrentUser>`)
///   into the GraphQL execution context
///
/// # Non-Responsibilities
///
/// - Authorization (role checks, permissions)
/// - Domain-specific interpretation of the authenticated subject
/// - Error shaping for application-specific use cases
///
/// # Authentication Model
///
/// - If authentication succeeds, `Some(CurrentUser)` is injected
/// - If authentication fails or is disabled, `None` is injected
///
/// This allows resolvers to explicitly distinguish between
/// *unauthenticated* and *authenticated* requests using the type system.
///
/// # Type Parameters
///
/// - `Q`: GraphQL query root
/// - `M`: GraphQL mutation root
/// - `S`: GraphQL subscription root
///
/// All type parameters must be `Send + Sync + 'static` to satisfy
/// `async-graphql` execution requirements.
pub async fn graphql_handler<Q, M, S>(
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
    Q: Send + Sync + 'static,
    M: Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    // -----------------------------
    // CSRF validation
    // -----------------------------
    //
    // When CSRF protection is enabled, validate the request headers
    // and cookies. On failure, return a GraphQL-compliant error
    // response (HTTP 200 with `errors` field).
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
