# wzs-web

A reusable **Rust web foundation library** providing shared infrastructure
components for backend services built with **Axum**, **MySQL**, and
environment-based configuration.

`wzs-web` provides configuration management, database abstractions,
GraphQL helpers, JWT authentication support, CSRF/CORS utilities,
SPA helpers, email infrastructure, image processing, file uploads,
and other common web application components.

## Features

* **Unified Application Configuration (`AppConfig`)**
  Captures the process environment once and builds application-wide typed
  configuration from a shared immutable `EnvConfig` snapshot.

* **Application-Defined API Configurations (`ApiConfig`)**
  Applications can construct any number of API configurations using
  arbitrary environment-variable prefixes. Each API can independently
  configure CORS, CSRF, JWT authentication, and GraphQL authentication
  cookies.

* **Web Root Configuration (`WebConfig`)**
  `WEB_ROOT` defines the application's web root without imposing any
  application-specific frontend or SPA directory layout.

* **Optional JWT Authentication**
  JWT authentication is enabled only when a non-empty JWT secret is
  configured. Applications can therefore start without JWT configuration,
  which is useful for frontend development and testing.

* **Environment Snapshot (`EnvConfig`)**
  All environment variables are preserved, including application-specific
  variables not interpreted directly by `wzs-web`. Scoped environment
  views can be created with `EnvConfig::with_prefix()`.

* **Database Layer**
  Reusable MySQL connection helpers and a generic `Db` abstraction with
  `Param`, `Value`, and `Row` types.

* **GraphQL Infrastructure**
  Helpers for Async-GraphQL/Axum integration, authentication context, and
  CSRF validation. GraphQL request handling uses `ApiConfig`.

* **SPA Infrastructure**
  A generic SPA entry handler that injects CSRF tokens and uses `ApiConfig`
  without making assumptions about application-specific SPA names,
  routes, or directory layouts.

* **Notification / Email Infrastructure**
  SMTP-based email delivery using a port/adapter design.

* **Security & Web Utilities**
  CSRF protection, CORS middleware, JWT utilities, and web helpers.
  Authentication, CSRF, and SMTP secrets are redacted from debug output.

* **Image Processing**
  Image resizing and validation using the `image` crate, including input
  safety limits.

* **Upload Infrastructure**
  Configurable upload directories, local storage, and upload handling.

## Configuration Architecture

`AppConfig::from_env()` loads application-wide configuration in the
following order:

1. Read `APP_ENV`.
2. Load a dotenv file when not running in production.
3. Capture the complete process environment into `EnvConfig`.
4. Build application-wide typed configuration from that snapshot.
5. Preserve the complete `EnvConfig` so applications can construct their
   own API-specific and application-specific configuration.

The resulting structure is conceptually:

```text
AppConfig
├── env
├── db
├── http
├── web
│   └── root
├── image
├── upload
├── mail
└── enable_graphiql
```

API configuration is deliberately separate from `AppConfig`:

```text
EnvConfig
   │
   ├── prefix "PUBLIC_" ──> ApiConfig
   │                        ├── cors
   │                        ├── csrf
   │                        ├── auth
   │                        └── graphql_auth
   │
   ├── prefix "ADMIN_" ───> ApiConfig
   │                        ├── cors
   │                        ├── csrf
   │                        ├── auth
   │                        └── graphql_auth
   │
   └── any other prefix ──> ApiConfig
                            ├── cors
                            ├── csrf
                            ├── auth
                            └── graphql_auth
```

This separation keeps `wzs-web` application-agnostic.

The library does not decide how many APIs an application has or what those
APIs are called.

## Environment Snapshot

`EnvConfig` captures the complete environment at application startup.

Typed configuration structures read from this snapshot instead of repeatedly
accessing the process environment.

Application-specific variables that are not known by `wzs-web` remain
available:

```rust
use wzs_web::config::app::AppConfig;

let cfg = AppConfig::from_env();

let bcrypt_cost = cfg.env.get_u32("BCRYPT_COST");
let public_web_base_url = cfg.env.get("PUBLIC_WEB_BASE_URL");
let reservation_max_days = cfg.env.get_u32("RESERVATION_MAX_DAYS");
```

This allows applications to keep their own settings in the same environment
without requiring every variable to be represented by `wzs-web`.

### Scoped environment configuration

`EnvConfig::with_prefix()` creates a new environment snapshot containing
only variables with the specified prefix, with that prefix removed.

For example:

```text
PUBLIC_JWT_SECRET=secret
PUBLIC_CORS_ENABLED=true
OTHER_VALUE=ignored
```

can be scoped with:

```rust
let public_env = cfg.env.with_prefix("PUBLIC_");
```

The resulting configuration contains:

```text
JWT_SECRET=secret
CORS_ENABLED=true
```

The original `EnvConfig` is unchanged.

Normally applications do not need to call `with_prefix()` directly when
constructing an `ApiConfig`, because `ApiConfig::from_prefixed_env()` performs
this operation internally.

## Dotenv Loading

When `APP_ENV` is not `production`, `AppConfig::from_env()` attempts to load
a dotenv file in this order:

1. `DOTENV_FILE`, when explicitly configured
2. `.env.{APP_ENV}`
3. `.env`

For example:

```text
APP_ENV=development
```

causes `.env.development` to be tried before `.env`.

When:

```text
APP_ENV=production
```

dotenv loading is skipped.

## Usage

### Dependency

```toml
[dependencies]
wzs-web = { git = "https://github.com/kay1759/wzs-web.git" }
```

### Basic setup

```rust
use wzs_web::config::app::AppConfig;
use wzs_web::db::connection::get_pool;
use wzs_web::db::mysql_adapter::MySqlDb;
use wzs_web::db::port::Db;

fn main() {
    let cfg = AppConfig::from_env();

    let pool = get_pool(&cfg.db);
    let db = MySqlDb::new(pool.clone());

    let rows = db
        .fetch_all("SELECT uuid, title FROM contents", &[])
        .unwrap();

    for row in rows {
        let uuid = row.get_string("uuid").unwrap();
        let title = row.get_string("title").unwrap();

        println!("uuid: {uuid}, title: {title}");
    }
}
```

## API Configuration

`wzs-web` does not define fixed public or administrative APIs.

Instead, applications construct the API configurations they require from
the shared environment snapshot.

For example:

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

let pickup_api =
    ApiConfig::from_prefixed_env(
        &cfg.env,
        "PICKUP_",
        "pickup_token",
    );
```

The prefixes and default cookie names are application decisions.

An application could equally use:

```rust
let customer_api =
    ApiConfig::from_prefixed_env(
        &cfg.env,
        "CUSTOMER_",
        "customer_token",
    );

let staff_api =
    ApiConfig::from_prefixed_env(
        &cfg.env,
        "STAFF_",
        "staff_token",
    );
```

`wzs-web` does not automatically discover prefixes. Applications explicitly
construct the API configurations they need.

This makes it possible for one application to expose multiple API entry
points with different security policies, for example:

```text
/api/public/graphql
/api/admin/graphql
/api/pickup/graphql
```

Each API can use different CORS origins, CSRF secrets, JWT secrets, and
authentication cookie names.

## Web Root and SPA Layout

Application-wide web configuration uses:

```text
WEB_ROOT=/path/to/web/root
```

It is available through:

```rust
let cfg = AppConfig::from_env();

let web_root = &cfg.web.root;
```

`wzs-web` deliberately does not impose an SPA directory structure.

For example, an application may choose:

```text
WEB_ROOT/
├── members/
│   ├── index.html
│   └── assets/
├── admin/
│   ├── index.html
│   └── assets/
└── pickup/
    ├── index.html
    └── assets/
```

Another application may use an entirely different structure.

The application is responsible for deciding:

* SPA names
* URL namespaces
* frontend build directories
* static asset directories
* which `ApiConfig` belongs to each SPA

`wzs-web` provides the reusable handler and configuration primitives rather
than defining application composition.

## SPA Entry Handler

`spa_entry_handler` provides generic SPA entry-page handling with CSRF token
generation.

It requires two Axum extensions:

```text
ApiConfig
Arc<String>
```

The `Arc<String>` contains the preloaded SPA entry HTML.

The HTML may contain:

```text
{{ csrf_token }}
```

The handler:

1. obtains CSRF configuration from `ApiConfig`,
2. generates a CSRF token,
3. stores the token in the CSRF cookie, and
4. replaces `{{ csrf_token }}` in the HTML.

A simplified router can be constructed as:

```rust
use std::sync::Arc;

use axum::{
    routing::get,
    Extension,
    Router,
};

use wzs_web::config::api::ApiConfig;
use wzs_web::web::spa::entry::spa_entry_handler;

fn build_spa_router(
    html: Arc<String>,
    api_config: ApiConfig,
) -> Router {
    Router::new()
        .route("/", get(spa_entry_handler))
        .fallback(spa_entry_handler)
        .layer(Extension(html))
        .layer(Extension(api_config))
}
```

The handler does not know whether the SPA represents members,
administrators, pickup terminals, or any other application concept.

## JWT Authentication

JWT authentication is optional for each `ApiConfig`.

For an API constructed with:

```rust
ApiConfig::from_prefixed_env(
    &cfg.env,
    "PUBLIC_",
    "public_token",
)
```

JWT configuration can be supplied as:

```text
PUBLIC_JWT_SECRET=public-secret
```

A missing, empty, or whitespace-only JWT secret disables authentication for
that API.

This allows an application to start without JWT configuration when
authentication is not required, such as during frontend development or tests.

The default JWT cookie name is supplied by the application when constructing
the `ApiConfig`:

```rust
let public_api =
    ApiConfig::from_prefixed_env(
        &cfg.env,
        "PUBLIC_",
        "public_token",
    );
```

It can be overridden by the scoped environment variable:

```text
PUBLIC_JWT_COOKIE_NAME=my_public_token
```

For another prefix:

```rust
let admin_api =
    ApiConfig::from_prefixed_env(
        &cfg.env,
        "ADMIN_",
        "admin_token",
    );
```

the corresponding override is:

```text
ADMIN_JWT_COOKIE_NAME=my_admin_token
```

## CSRF

CSRF protection is configured independently for each `ApiConfig`.

For example:

```text
PUBLIC_CSRF_SECRET=public-csrf-secret
PUBLIC_CSRF_COOKIE_SECURE=true
PUBLIC_CSRF_COOKIE_HTTPONLY=true
```

or:

```text
ADMIN_CSRF_SECRET=admin-csrf-secret
ADMIN_CSRF_COOKIE_SECURE=true
ADMIN_CSRF_COOKIE_HTTPONLY=true
```

CSRF protection for an API is enabled only when its scoped `CSRF_SECRET`
contains a non-empty value.

`CSRF_COOKIE_SECURE` and `CSRF_COOKIE_HTTPONLY` both default to `true`.

`CsrfConfig` itself always contains a 32-byte signing secret. When no
`CSRF_SECRET` is supplied, a random secret is generated, but `ApiConfig`
still considers CSRF protection disabled.

The same mechanism works with arbitrary application-defined prefixes:

```text
PICKUP_CSRF_SECRET=pickup-csrf-secret
PICKUP_CSRF_COOKIE_SECURE=true
PICKUP_CSRF_COOKIE_HTTPONLY=true
```

## CORS

CORS is configured independently for each `ApiConfig`.

For example:

```text
PUBLIC_CORS_ENABLED=true
PUBLIC_CORS_ORIGINS=http://localhost:5173
PUBLIC_CORS_CREDENTIALS=true

ADMIN_CORS_ENABLED=true
ADMIN_CORS_ORIGINS=https://admin.example.com
ADMIN_CORS_CREDENTIALS=true
```

CORS is disabled by default.

The same suffixes can be used with any application-defined API prefix.

## Environment Variables

### Application-wide

| Variable              | Description                                   | Default         |
| --------------------- | --------------------------------------------- | --------------- |
| `APP_ENV`             | Application environment                       | `development`   |
| `DOTENV_FILE`         | Explicit dotenv file                          | automatic       |
| `DATABASE_URL`        | MySQL connection URL                          | unset           |
| `DATABASE_MAX_CONN`   | Optional maximum DB connections               | unset           |
| `HTTP_MAX_BODY_BYTES` | Maximum HTTP body size in bytes               | see below       |
| `HTTP_MAX_BODY_MB`    | Maximum HTTP body size in MiB                 | `5`             |
| `WEB_ROOT`            | Application web root                          | `.`             |
| `UPLOAD_ROOT`         | Upload root used by environment configuration | `./var/uploads` |
| `UPLOAD_IMAGE_DIR`    | Image upload subdirectory                     | `images`        |
| `UPLOAD_FILE_DIR`     | General file upload subdirectory              | `files`         |
| `IMAGE_MAX_WIDTH`     | Maximum configured image width                | `1280`          |
| `IMAGE_MAX_HEIGHT`    | Maximum configured image height               | `1280`          |
| `GRAPHIQL`            | Enable GraphiQL                                | `false`         |

`HTTP_MAX_BODY_BYTES` takes precedence over `HTTP_MAX_BODY_MB`.
If neither contains a valid value, the limit is 5 MiB.

`WEB_ROOT` specifies only the generic application web root. The library does
not define frontend names or subdirectories below that root.

Note that `UploadConfig::default()` retains `./uploads` for backward
compatibility, while environment-based configuration through
`AppConfig` / `UploadConfig::from_env_config()` defaults to
`./var/uploads`.

### API-specific

API-specific variables use an application-selected prefix followed by one of
the suffixes below.

For example, with the prefix `PUBLIC_`:

```text
PUBLIC_CORS_ENABLED
PUBLIC_CSRF_SECRET
PUBLIC_JWT_SECRET
```

With `PICKUP_`:

```text
PICKUP_CORS_ENABLED
PICKUP_CSRF_SECRET
PICKUP_JWT_SECRET
```

| Variable suffix        | Description                                        | Default                         |
| ---------------------- | -------------------------------------------------- | ------------------------------- |
| `CORS_ENABLED`         | Enable CORS                                        | `false`                         |
| `CORS_ORIGINS`         | Allowed origins                                    | empty                           |
| `CORS_CREDENTIALS`     | Allow credentialed CORS requests                   | `false`                         |
| `CSRF_SECRET`          | CSRF signing secret; non-empty value enables CSRF  | unset / disabled                |
| `CSRF_COOKIE_SECURE`   | Set `Secure` on the CSRF cookie                    | `true`                          |
| `CSRF_COOKIE_HTTPONLY` | Set `HttpOnly` on the CSRF cookie                  | `true`                          |
| `JWT_SECRET`           | JWT secret; non-empty value enables authentication | unset / disabled                |
| `JWT_COOKIE_NAME`      | GraphQL JWT cookie name                            | application-provided default    |

For example:

```text
PUBLIC_CORS_ENABLED=true
PUBLIC_CORS_ORIGINS=http://localhost:5173
PUBLIC_CORS_CREDENTIALS=true
PUBLIC_CSRF_SECRET=development-csrf-secret
PUBLIC_CSRF_COOKIE_SECURE=false
PUBLIC_JWT_SECRET=development-jwt-secret
PUBLIC_JWT_COOKIE_NAME=public_token
```

Applications may construct additional configurations using any prefix they
choose.

## Mail / SMTP

Mail configuration is application-wide.

| Variable          | Description                             | Default                           |
| ----------------- | --------------------------------------- | --------------------------------- |
| `SMTP_HOST`       | SMTP server hostname                    | unset                             |
| `SMTP_PORT`       | SMTP server port                        | required when mail is configured |
| `SMTP_USERNAME`   | SMTP username                           | required when mail is configured |
| `SMTP_PASSWORD`   | SMTP password                           | required when mail is configured |
| `SMTP_FROM_EMAIL` | Sender email address                    | required when mail is configured |
| `SMTP_FROM_NAME`  | Sender display name                     | `Notifier`                        |
| `NOTIFY_TO_EMAIL` | Comma-separated notification recipients | empty                             |

When `AppConfig` is used, mail configuration is absent when `SMTP_HOST` is not
configured.

When `SMTP_HOST` is present, `AppConfig` attempts to construct `MailConfig`.
If the remaining required SMTP configuration is missing or invalid,
`AppConfig::mail` becomes `None`.

Calling `MailConfig::from_env_config()` directly instead returns an error for
missing required variables or an invalid `SMTP_PORT`.

Multiple notification recipients can be specified as:

```text
NOTIFY_TO_EMAIL=a@example.com,b@example.com
```

Whitespace around addresses is ignored.

## Image Processing

`ImageConfig` controls application-level target image dimensions:

```text
IMAGE_MAX_WIDTH=1280
IMAGE_MAX_HEIGHT=1280
```

The image processor also applies decode/input safety limits to reject
unreasonably large input before processing.

The default decode limits are:

```text
compressed input: 20 MiB
width:            12,000 px
height:           12,000 px
pixel count:      40 megapixels
```

## Directory Overview

```text
src/
├── auth/
│   └── jwt.rs
├── config/
│   ├── api.rs
│   ├── app.rs
│   ├── auth.rs
│   ├── csrf.rs
│   ├── db.rs
│   ├── env.rs
│   ├── image.rs
│   ├── mail.rs
│   ├── upload.rs
│   └── web.rs
├── db/
│   ├── connection.rs
│   ├── mysql_adapter.rs
│   └── port.rs
├── graphql/
│   ├── config.rs
│   ├── context.rs
│   ├── guard.rs
│   └── handler.rs
├── image/
│   ├── image_rs_processor.rs
│   └── processor.rs
├── notification/
│   ├── email.rs
│   ├── email_sender.rs
│   └── smtp/
├── time/
└── web/
    ├── cors.rs
    ├── csrf.rs
    ├── spa/
    ├── template.rs
    └── upload/
```

## Testing

Run the full test suite:

```bash
cargo test
```

Run documentation tests:

```bash
cargo test --doc
```

Run Clippy across all targets and features:

```bash
cargo clippy --all-targets --all-features
```

Check formatting:

```bash
cargo fmt --check
```

Before release, all four commands should complete successfully.

## Documentation

Generate and open the crate documentation:

```bash
cargo doc --open
```

## License

MIT

## Author

[Katsuyoshi Yabe](https://github.com/kay1759)
