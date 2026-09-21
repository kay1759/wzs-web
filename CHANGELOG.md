# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.3.0] - 2026-09-22

### Added

* Added `WebConfig` for application-wide web root configuration through
  `WEB_ROOT`.
* Added `ApiConfig::from_prefixed_env()` for constructing API configuration
  from arbitrary application-defined environment-variable prefixes.
* Added support for overriding the GraphQL JWT cookie name through the scoped
  `JWT_COOKIE_NAME` environment variable.
* Added generic SPA entry handling based on `ApiConfig`.

### Changed

* Changed `AppConfig` so it no longer defines a fixed set of API entry points.
  Applications now construct the `ApiConfig` instances they require from the
  shared `EnvConfig` snapshot.
* Changed API configuration from predefined public and administrative APIs to
  application-defined API prefixes.
* Changed web configuration from a single HTML file path to a generic
  `WEB_ROOT`.
* Changed `spa_entry_handler` to receive `ApiConfig` instead of receiving
  `CsrfConfig` directly.
* Changed SPA CSRF handling to obtain CSRF configuration through
  `ApiConfig`.
* Unified GraphQL and SPA configuration injection around `ApiConfig`.
* Kept SPA naming, URL namespaces, filesystem layout, and API-to-SPA mapping
  as application-level concerns rather than library-level configuration.

### Removed

* Removed `HTML_PATH` configuration.
* Removed `html_path` from `AppConfig`.
* Removed predefined `public_api` and `admin_api` fields from `AppConfig`.
* Removed the requirement to inject `CsrfConfig` separately into the generic
  SPA entry handler.

### Breaking Changes

* `AppConfig` no longer exposes `public_api` or `admin_api`.
* Applications must explicitly construct API configurations using
  `ApiConfig::from_prefixed_env()` or another `ApiConfig` constructor.
* `HTML_PATH` is no longer supported. Applications should configure
  `WEB_ROOT` instead.
* `spa_entry_handler` now requires an Axum `Extension<ApiConfig>` instead of
  `Extension<CsrfConfig>`.

### Migration Notes

Applications that previously accessed API configuration through:

```rust
let cfg = AppConfig::from_env();

let public_api = &cfg.public_api;
let admin_api = &cfg.admin_api;
```

should explicitly construct the required configurations:

```rust
use wzs_web::config::api::ApiConfig;
use wzs_web::config::app::AppConfig;

let cfg = AppConfig::from_env();

let public_api =
    ApiConfig::from_prefixed_env(
        &cfg.env,
        "PUBLIC_",
        "public_token",
    );

let admin_api =
    ApiConfig::from_prefixed_env(
        &cfg.env,
        "ADMIN_",
        "admin_token",
    );
```

Applications are not limited to these prefixes. For example:

```rust
let pickup_api =
    ApiConfig::from_prefixed_env(
        &cfg.env,
        "PICKUP_",
        "pickup_token",
    );
```

Configuration that previously used:

```text
HTML_PATH=/path/to/frontend/index.html
```

should be changed to the application web root:

```text
WEB_ROOT=/path/to/frontend
```

The application is responsible for selecting files and directories below
`WEB_ROOT`.

For example, an application may use:

```text
WEB_ROOT/
├── members/
│   └── index.html
├── admin/
│   └── index.html
└── pickup/
    └── index.html
```

but this layout is not imposed by `wzs-web`.

SPA routers that previously injected CSRF configuration separately:

```rust
Router::new()
    .route("/", get(spa_entry_handler))
    .layer(Extension(html))
    .layer(Extension(csrf_config))
```

should now inject the API configuration:

```rust
Router::new()
    .route("/", get(spa_entry_handler))
    .layer(Extension(html))
    .layer(Extension(api_config))
```

`spa_entry_handler` obtains its CSRF configuration from `ApiConfig`.

---

## [0.2.0] - 2026-09-21

### Added

* Added `EnvConfig` as an immutable snapshot of the process environment.
* Added typed environment accessors for strings, booleans, and numeric values.
* Added `EnvConfig::with_prefix()` for creating scoped environment views.
* Added `AuthConfig` with optional JWT authentication.
* Added `ApiConfig` for independently configuring API entry points.
* Added separate public and administrative API configuration using `PUBLIC_*`
  and `ADMIN_*` environment variables.
* Added configurable JWT cookie names with `public_token` and `admin_token`
  as defaults.
* Added default decode limits for image processing to restrict input size,
  dimensions, and total pixel count.

### Changed

* Changed `AppConfig` to capture the environment once and construct typed
  configuration from the same `EnvConfig` snapshot.
* Changed `AppConfig` to expose `public_api` and `admin_api` instead of
  application-wide authentication, CSRF, and CORS configuration.
* Changed database, HTTP, CORS, CSRF, image, upload, mail, and authentication
  configuration to support construction from `EnvConfig`.
* Changed GraphQL request handling to use `ApiConfig`, allowing each GraphQL
  endpoint to use its own authentication and CSRF configuration.
* Changed JWT authentication so that a missing, empty, or whitespace-only
  `JWT_SECRET` disables authentication instead of preventing applications from
  starting.
* Changed CSRF enablement so that an API enables CSRF protection only when a
  non-empty `CSRF_SECRET` is configured.
* Updated dependencies, including Askama, Async-GraphQL, Base64, HMAC,
  JSON Web Token, MySQL, Rand, SHA-2, and Tower HTTP.
* Updated code for compatibility with the newer HMAC, Rand, MySQL, and related
  dependency APIs.
* Updated code and tests to pass Clippy across all targets and features.

### Removed

* Removed the previous top-level authentication, CSRF, and CORS configuration
  from `AppConfig`.
* Removed the obsolete `AppConfig::is_csrf_enabled()` helper.
* Removed the unused GraphQL JWT guard.
* Removed the `temp-env` development dependency.

### Security

* JWT secrets are redacted from `Debug` output.
* CSRF signing secrets are redacted from `Debug` output.
* SMTP passwords are redacted from `Debug` output.
* Added image decode limits to reduce the risk of excessive resource usage
  from unusually large image inputs.

### Migration Notes

Applications upgrading from 0.1.x should update API-specific environment
variables to use the appropriate prefix.

For example:

```text
JWT_SECRET=...
CSRF_SECRET=...
CORS_ENABLED=true
CORS_ORIGINS=...
```

becomes either:

```text
PUBLIC_JWT_SECRET=...
PUBLIC_CSRF_SECRET=...
PUBLIC_CORS_ENABLED=true
PUBLIC_CORS_ORIGINS=...
```

or:

```text
ADMIN_JWT_SECRET=...
ADMIN_CSRF_SECRET=...
ADMIN_CORS_ENABLED=true
ADMIN_CORS_ORIGINS=...
```

Code that previously accessed top-level API security configuration through
`AppConfig` should instead use:

```rust
cfg.public_api
cfg.admin_api
```

Application-specific environment variables remain available through:

```rust
cfg.env
