
# wzs-web

A reusable **Rust web foundation library** providing shared infrastructure components
for backend services built with **Axum**, **MySQL**, and **dotenv-based configuration**.

This crate offers configuration management, database connection helpers, and
security-related utilities (CSRF, CORS, etc.) used across multiple Rust web projects.

---

## Features

- **Unified Configuration Loader**
  Load all environment-based settings via `AppConfig` (with `.env` auto-loading).
- **Database Layer**
  Provides reusable MySQL connection helpers (`create_pool`, `get_pool`) using `Arc<Pool>`.
- **Infrastructure Abstraction**
  Defines `Db` trait and `Param` / `Value` / `Row` types for driver-agnostic persistence.
- **Security & Web Utilities**
  Includes CSRF protection, CORS configuration, and HTML template rendering.
- **Environment Utilities**
  Helper functions for reading boolean flags and numeric values safely.

---

## Directory Overview
```
src/
├── config/
│   ├── app.rs # Loads .env and builds top-level AppConfig
│   ├── db.rs # Defines DbConfig and connection pool factory
│   ├── csrf.rs # CSRF secret/cookie configuration
│   ├── env.rs # Environment variable utilities (read_flag, read_u32, etc.)
│   └── web.rs # HttpConfig and CorsConfig
│
├── db/
│   ├── connection.rs # Global shared MySQL pool (OnceLock or external cfg-based)
│   ├── mysql_adapter.rs# MySQL implementation of generic Db trait
│   └── port.rs # Db trait, Param/Value/Row abstraction
│
└── web/
    ├── csrf.rs # Token generation, verification, cookie handling
    ├── cors.rs # CORS layer builder using tower_http
    └── template.rs # Askama template rendering utilities
```

## Usage

Add this crate to your project (e.g. as a workspace dependency):

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
| `HTTP_MAX_BODY_MB`     | Max request size in MB (if above unset)                 | `5`                                      |
| `CSRF_SECRET`          | Secret string for CSRF HMAC (enables CSRF protection)   | *random if missing*                      |
| `CSRF_COOKIE_SECURE`   | Sets Secure flag on CSRF cookie                         | `true`                                   |
| `CSRF_COOKIE_HTTPONLY` | Sets HttpOnly flag on CSRF cookie                       | `true`                                   |
| `CORS_ORIGINS`         | Comma-separated allowed origins                         | `"http://localhost:5173"`                |
| `CORS_CREDENTIALS`     | Allow credentials in CORS                               | `false`                                  |
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
