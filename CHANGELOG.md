# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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
```
