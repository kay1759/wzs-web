
# wzs-web


A reusable **Rust web foundation library** providing shared infrastructure components
for backend services built with **Axum**, **MySQL**, and **dotenv-based configuration**.

This crate offers unified configuration management, database connection helpers,
and security utilities (CSRF, CORS), along with image and upload configuration modules.

---

## Features


- **Unified Configuration Loader (`AppConfig`)**
  Centralized environment-based configuration with `.env` auto-loading.

- **Database Layer**
  Reusable MySQL connection helpers (`create_pool`, `get_pool`) with shared `Arc<Pool>`.

- **Infrastructure Abstraction**
  Generic `Db` trait and `Param` / `Value` / `Row` types for driver-agnostic persistence.

- **Security & Web Utilities**
  Includes CSRF protection, CORS middleware, and HTML template rendering via Askama.

- **Image & Upload Configuration**
  Structured control for image resizing limits and upload directory paths.

- **Environment Utilities**
  Safe parsers for boolean and numeric environment variables (`read_flag`, `read_u32`, etc.).

---

## Directory Overview
```
src/
├── config/
│    ├── app.rs # Loads .env and builds top-level AppConfig
│    ├── db.rs # Database configuration and connection URL
│    ├── csrf.rs # CSRF secret and cookie configuration
│    ├── env.rs # Environment variable utilities
│    ├── image.rs # Max width/height configuration for image processing
│    ├── upload.rs # Upload directory configuration (root, image_dir, file_dir)
│    └── web.rs # HttpConfig and CorsConfig
│
├── db/
│    ├── connection.rs # Shared MySQL pool (OnceLock or external injection)
│    ├── mysql_adapter.rs # MySQL implementation of generic Db trait
│    └── port.rs # Db trait and Row/Value abstractions
│
├── image/
│    ├── image_rs_processor.rs # Image resizing implementation using image-rs
│    └── processor.rs # Generic image processing traits and utilities
│
└── web/
     ├── csrf.rs # Token generation, verification, cookie handling
     ├── cors.rs # CORS layer builder using tower_http
     ├── template.rs # Askama template rendering utilities
     └── upload/
          ├── storage.rs # Storage trait abstraction (local/S3-compatible)
          ├── local_storage.rs # Local filesystem adapter for file uploads
          ├── uploader.rs # High-level upload service
          └── upload_handler.rs# Axum multipart upload handler
```

## Usage

Add this crate to your project:

```toml
[dependencies]
wzs-web = { git = "https://github.com/kay1759/wzs-web.git" }
```
```
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
        let uuid: String = row.get_string("uuid").unwrap();
        let title: String = row.get_string("title").unwrap();

        println!("uuid: {}, title: {}", uuid, title);
    }
}
```

## Environment Variables

| Variable               | Description                                             | Default / Example                        |
| ---------------------- | ------------------------------------------------------- | ---------------------------------------- |
| `APP_ENV`              | Current environment (`development`, `production`, etc.) | `development`                            |
| `DOTENV_FILE`          | Custom `.env` path override                             | `.env.local`                             |
| `DATABASE_URL`         | MySQL connection string                                 | `mysql://user:pass@localhost:3306/appdb` |
| `DATABASE_MAX_CONN`    | Max DB connections (optional)                           | `20`                                     |
| `HTTP_MAX_BODY_BYTES`  | Max request size in bytes                               | Derived from MB                          |
| `HTTP_MAX_BODY_MB`     | Max request size in MB (if bytes not set)               | `5`                                      |
| `CSRF_SECRET`          | Secret string for CSRF HMAC (enables CSRF protection)   | *random if missing*                      |
| `CSRF_COOKIE_SECURE`   | Sets `Secure` flag on CSRF cookie                       | `true`                                   |
| `CSRF_COOKIE_HTTPONLY` | Sets `HttpOnly` flag on CSRF cookie                     | `true`                                   |
| `CORS_ORIGINS`         | Comma-separated allowed origins                         | `"http://localhost:5173"`                |
| `CORS_CREDENTIALS`     | Allow credentials (cookies, headers) in CORS            | `false`                                  |
| `UPLOAD_ROOT`          | Root directory for uploaded files                       | `"./var/uploads"`                        |
| `UPLOAD_IMAGE_DIR`     | Subdirectory for image uploads                          | `"images"`                               |
| `UPLOAD_FILE_DIR`      | Subdirectory for general file uploads                   | `"files"`                                |
| `IMAGE_MAX_WIDTH`      | Max allowed image width (px)                            | `1280`                                   |
| `IMAGE_MAX_HEIGHT`     | Max allowed image height (px)                           | `1280`                                   |
| `GRAPHIQL`             | Enable GraphiQL IDE (for dev only)                      | `false`                                  |


## Testing

All core modules are unit-tested and can be run locally:

    cargo test


You can also run documentation tests:

    cargo test --doc


## Documentation
Generate and open the full crate documentation:

    cargo doc --open


This will include all module-level documentation (config, db, etc.) with usage examples.


## Licence:

[MIT]

## Author

[Katsuyoshi Yabe](https://github.com/kay1759)
