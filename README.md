
# wzs-web


A reusable **Rust web foundation library** providing shared infrastructure components
for backend services built with **Axum**, **MySQL**, and **dotenv-based configuration**.

This crate offers unified configuration management, database connection helpers,
security utilities (CSRF, CORS), and **a notification/email abstraction layer**,
along with image and upload configuration modules.


---

## Features


- **Unified Configuration Loader (`AppConfig`)**
  Centralized environment-based configuration with `.env` auto-loading.

- **Database Layer**
  Reusable MySQL connection helpers (`create_pool`, `get_pool`) with shared `Arc<Pool>`.

- **Infrastructure Abstraction**
  Generic `Db` trait and `Param` / `Value` / `Row` types for driver-agnostic persistence.

- **Notification / Email Infrastructure**
  SMTP-based email sender with a clean port/adapter design.

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
│    ├── app.rs        # Loads .env and builds top-level AppConfig
│    ├── db.rs         # Database configuration
│    ├── csrf.rs       # CSRF configuration
│    ├── env.rs        # Environment variable utilities
│    ├── image.rs      # Image processing limits
│    ├── mail.rs       # SMTP / mail configuration
│    ├── upload.rs     # Upload directory configuration
│    └── web.rs        # HTTP & CORS configuration
│
├── db/
│    ├── connection.rs # Shared MySQL pool
│    ├── mysql_adapter.rs # MySQL implementation of Db trait
│    └── port.rs       # Db trait and Row/Value abstractions
│
├── notification/
│    ├── email.rs      # Email Value Objects (Email, EmailBody, Attachment)
│    ├── email_sender.rs # EmailSender port (trait)
│    ├── smtp/
│    │    └── smtp_email_sender.rs # SMTP adapter (lettre-based)
│    └── smtp.rs       # Module exports
│
├── image/
│    ├── image_rs_processor.rs # image-rs based processor
│    └── processor.rs # Generic image processing traits
│
└── web/
     ├── csrf.rs       # CSRF token handling
     ├── cors.rs       # CORS layer builder
     ├── template.rs   # Askama helpers
     └── upload/
          ├── storage.rs
          ├── local_storage.rs
          ├── uploader.rs
          └── upload_handler.rs
```

## Usage

### Basic setup

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

### Sending email (example)
```
use wzs_web::notification::{
    email::{Email, EmailBody},
    smtp::SmtpEmailSender,
};

let sender = SmtpEmailSender::new(
    "smtp.example.com",
    587,
    "user",
    "pass",
    "from@example.com",
    "Notifier",
    vec!["to@example.com".parse()?],
)?;

let email = Email {
    subject: "Hello".into(),
    body: EmailBody::Text("Hello world".into()),
    to: vec![],
    cc: vec![],
    bcc: vec![],
};

sender.send(email).await?;
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

### Mail / SMTP

| Variable               | Description                                             | Default / Example                        |
| ---------------------- | ------------------------------------------------------- | ---------------------------------------- |
| `SMTP_HOST`            | SMTP server hostname                                    | `none`                                   |
| `SMTP_PORT`            | SMTP server port                                        | `none`                                   |
| `SMTP_USERNAME`        | SMTP username                                           | `none`                                   |
| `SMTP_PASSWORD`        | SMTP password                                           | `none`                                   |
| `SMTP_FROM_EMAIL`      | Sender email address                                    | `none`                                   |
| `SMTP_FROM_NAME`       | Sender display name                                     | `"Notifier"`                             |
| `NOTIFY_TO_EMAIL`      | Notification recipients (comma-separated)               | `empty`                                  |

#### Note

- Mail support is enabled only when SMTP_HOST is set.

- Missing or invalid SMTP variables disable mail configuration.

- NOTIFY_TO_EMAIL supports multiple addresses:

```
NOTIFY_TO_EMAIL=a@example.com,b@example.com
```


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
