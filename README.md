# wzs-web

A reusable **Rust web foundation library** providing common components for backend services.

This crate offers configuration management, database connection helpers, and common infrastructure utilities
used across multiple web projects (e.g. REST, GraphQL, and file-based services).

---

## Features

- **Configuration Loader** — Environment-based application config (`AppConfig`, `.env` loading).
- **Database Layer** — MySQL connection pool management with `Arc<Pool>` reuse.
- **Infrastructure Abstraction** — `Db` trait, parameter/value mapping, and row conversion helpers.
- **Environment Helpers** — Convenient utilities for parsing flags and numeric values.

---

## Directory Structure
src
├── config
│ ├── app.rs # Loads .env and builds top-level AppConfig
│ ├── db.rs # Defines DbConfig and connection pool factory
│ └── env.rs # Environment variable utilities (read_flag, read_u32, etc.)
│
├── db
│ ├── connection.rs # Global shared MySQL pool (OnceLock)
│ ├── mysql_adapter.rs# MySQL implementation of the generic Db trait
│ └── port.rs # Defines Db trait, Param/Value/Row abstraction
│
├── config.rs # Module re-exports for config/*
├── db.rs # Module re-exports for db/*
└── lib.rs # Crate root and public API surface


---

## Usage

Add this crate to your project (e.g. as a workspace dependency):

```toml
[dependencies]
    wzs-web = { git = "https://github.com/kay1759/wzs-web.git" }

    use wzs_web::config::app::AppConfig;
    use wzs_web::db::connection::get_pool;

    fn main() {
        let cfg = AppConfig::from_env();
        let pool = get_pool();

        println!("Database URL: {:?}", cfg.db.url);
        // Application logic here...
    }

## Environment Variables

| Variable            | Description                                             | Example                                  |
| ------------------- | ------------------------------------------------------- | ---------------------------------------- |
| `APP_ENV`           | Current environment (`development`, `production`, etc.) | `development`                            |
| `DOTENV_FILE`       | Optional path to custom dotenv file                     | `.env.local`                             |
| `DATABASE_URL`      | MySQL connection URL                                    | `mysql://user:pass@127.0.0.1:3306/appdb` |
| `DATABASE_MAX_CONN` | Optional max connections                                | `20`                                     |
| `SQL_DEBUG`         | Enable SQL logging (`1`, `true`, `yes`)                 | `true`                                   |

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
