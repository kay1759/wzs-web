# wzs-web

A reusable **Rust web foundation library** providing shared infrastructure
components for backend services built with **Axum**, **MySQL**, and
environment-based configuration.

`wzs-web` provides configuration management, database abstractions,
GraphQL helpers, JWT authentication support, CSRF/CORS utilities,
email infrastructure, image processing, file uploads, and other common
web application components.

## Features

* **Unified Configuration (`AppConfig`)**
  Captures the process environment once and builds typed configuration
  from a shared immutable `EnvConfig` snapshot.

* **Multiple API Configurations (`ApiConfig`)**
  Public and administrative APIs can independently configure CORS,
  CSRF, JWT authentication, and GraphQL authentication cookies.

* **Optional JWT Authentication**
  JWT authentication is enabled only when a non-empty JWT secret is
  configured. Applications can therefore start without JWT configuration,
  which is useful for frontend development and testing.

* **Environment Snapshot (`EnvConfig`)**
  All environment variables are preserved, including application-specific
  variables not interpreted directly by `wzs-web`.

* **Database Layer**
  Reusable MySQL connection helpers and a generic `Db` abstraction with
  `Param`, `Value`, and `Row` types.

* **GraphQL Infrastructure**
  Helpers for Async-GraphQL/Axum integration, authentication context, and
  CSRF validation.

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

`AppConfig::from_env()` loads configuration in the following order:

1. Read `APP_ENV`.
2. Load a dotenv file when not running in production.
3. Capture the complete process environment into `EnvConfig`.
4. Build application-wide configuration from that snapshot.
5. Build independent public and administrative API configurations.

The resulting structure is conceptually:

```text
AppConfig
├── env
├── db
├── http
├── image
├── upload
├── mail
├── public_api
│   ├── cors
│   ├── csrf
│   ├── auth
│   └── graphql_auth
├── admin_api
│   ├── cors
│   ├── csrf
│   ├── auth
│   └── graphql_auth
├── enable_graphiql
└── html_path
```

Application-wide configuration uses ordinary environment variables such as
`DATABASE_URL`, `HTTP_MAX_BODY_MB`, and `SMTP_HOST`.

API-specific configuration uses separate prefixes:

```text
PUBLIC_*
ADMIN_*
```

For example:

```text
PUBLIC_JWT_SECRET
PUBLIC_CSRF_SECRET
PUBLIC_CORS_ENABLED

ADMIN_JWT_SECRET
ADMIN_CSRF_SECRET
ADMIN_CORS_ENABLED
```

Internally, the prefix is removed before constructing each `ApiConfig`.

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

## Public and Admin APIs

`AppConfig` contains two independent API configurations:

```rust
let cfg = AppConfig::from_env();

let public_api = &cfg.public_api;
let admin_api = &cfg.admin_api;
```

This makes it possible for one application to expose API entry points with
different security policies, for example:

```text
/public/graphql
/admin/graphql
```

The two APIs can use different CORS origins, CSRF secrets, JWT secrets, and
authentication cookie names.

## JWT Authentication

JWT authentication is optional.

For the public API:

```text
PUBLIC_JWT_SECRET=public-secret
```

For the administrative API:

```text
ADMIN_JWT_SECRET=admin-secret
```

A missing, empty, or whitespace-only JWT secret disables authentication for
that API.

This means an application can start without JWT configuration when
authentication is not required, such as during frontend development or tests.

Default GraphQL JWT cookie names are:

```text
public API: public_token
admin API:  admin_token
```

They can be overridden with:

```text
PUBLIC_JWT_COOKIE_NAME=my_public_token
ADMIN_JWT_COOKIE_NAME=my_admin_token
```

## CSRF

CSRF protection is configured independently for each API.

For example:

```text
PUBLIC_CSRF_SECRET=public-csrf-secret
PUBLIC_CSRF_COOKIE_SECURE=true
PUBLIC_CSRF_COOKIE_HTTPONLY=true

ADMIN_CSRF_SECRET=admin-csrf-secret
ADMIN_CSRF_COOKIE_SECURE=true
ADMIN_CSRF_COOKIE_HTTPONLY=true
```

CSRF protection for an API is enabled only when its `CSRF_SECRET` contains a
non-empty value.

`CSRF_COOKIE_SECURE` and `CSRF_COOKIE_HTTPONLY` both default to `true`.

`CsrfConfig` itself always contains a 32-byte signing secret. When no
`CSRF_SECRET` is supplied, a random secret is generated, but `ApiConfig`
still considers CSRF protection disabled.

## CORS

CORS is also configured independently for the public and administrative APIs.

Example:

```text
PUBLIC_CORS_ENABLED=true
PUBLIC_CORS_ORIGINS=http://localhost:5173
PUBLIC_CORS_CREDENTIALS=true

ADMIN_CORS_ENABLED=true
ADMIN_CORS_ORIGINS=https://admin.example.com
ADMIN_CORS_CREDENTIALS=true
```

CORS is disabled by default.

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
| `UPLOAD_ROOT`         | Upload root used by environment configuration | `./var/uploads` |
| `UPLOAD_IMAGE_DIR`    | Image upload subdirectory                     | `images`        |
| `UPLOAD_FILE_DIR`     | General file upload subdirectory              | `files`         |
| `IMAGE_MAX_WIDTH`     | Maximum configured image width                | `1280`          |
| `IMAGE_MAX_HEIGHT`    | Maximum configured image height               | `1280`          |
| `GRAPHIQL`            | Enable GraphiQL                               | `false`         |
| `HTML_PATH`           | HTML template path                            | empty           |

`HTTP_MAX_BODY_BYTES` takes precedence over `HTTP_MAX_BODY_MB`.
If neither contains a valid value, the limit is 5 MiB.

Note that `UploadConfig::default()` retains `./uploads` for backward
compatibility, while environment-based configuration through
`AppConfig` / `UploadConfig::from_env_config()` defaults to
`./var/uploads`.

### API-specific

Each variable below can use either the `PUBLIC_` or `ADMIN_` prefix.

| Variable suffix        | Description                                        | Default                        |
| ---------------------- | -------------------------------------------------- | ------------------------------ |
| `CORS_ENABLED`         | Enable CORS                                        | `false`                        |
| `CORS_ORIGINS`         | Allowed origins                                    | empty                          |
| `CORS_CREDENTIALS`     | Allow credentialed CORS requests                   | `false`                        |
| `CSRF_SECRET`          | CSRF signing secret; non-empty value enables CSRF  | unset / disabled               |
| `CSRF_COOKIE_SECURE`   | Set `Secure` on the CSRF cookie                    | `true`                         |
| `CSRF_COOKIE_HTTPONLY` | Set `HttpOnly` on the CSRF cookie                  | `true`                         |
| `JWT_SECRET`           | JWT secret; non-empty value enables authentication | unset / disabled               |
| `JWT_COOKIE_NAME`      | GraphQL JWT cookie name                            | `public_token` / `admin_token` |

For example:

```text
PUBLIC_CORS_ENABLED=true
PUBLIC_CORS_ORIGINS=http://localhost:5173
PUBLIC_CORS_CREDENTIALS=true
PUBLIC_CSRF_SECRET=development-csrf-secret
PUBLIC_CSRF_COOKIE_SECURE=false
PUBLIC_JWT_SECRET=development-jwt-secret

ADMIN_CORS_ENABLED=true
ADMIN_CORS_ORIGINS=https://admin.example.com
ADMIN_CORS_CREDENTIALS=true
ADMIN_CSRF_SECRET=admin-csrf-secret
ADMIN_JWT_SECRET=admin-jwt-secret
```

## Mail / SMTP

Mail configuration is application-wide.

| Variable          | Description                             | Default                          |
| ----------------- | --------------------------------------- | -------------------------------- |
| `SMTP_HOST`       | SMTP server hostname                    | unset                            |
| `SMTP_PORT`       | SMTP server port                        | required when mail is configured |
| `SMTP_USERNAME`   | SMTP username                           | required when mail is configured |
| `SMTP_PASSWORD`   | SMTP password                           | required when mail is configured |
| `SMTP_FROM_EMAIL` | Sender email address                    | required when mail is configured |
| `SMTP_FROM_NAME`  | Sender display name                     | `Notifier`                       |
| `NOTIFY_TO_EMAIL` | Comma-separated notification recipients | empty                            |

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
