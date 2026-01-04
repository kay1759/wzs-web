//! # wzs_web
//!
//! Common foundation library for internal and future public projects.
//!
//! This crate provides reusable infrastructure utilities such as:
//! - MySQL connection management (`infrastructure::db`)
//! - Common error handling (`anyhow`)
//! - Re-exports of frequently used utility crates (`chrono`, `uuid`, `mysql`, etc.)
//!
//! ## Example usage (in another crate)
//!
//! ```rust
//! use wzs_web::anyhow::{Result, Context};
//! use wzs_web::db::connection::get_pool;
// ===============================
// Re-exports of external crates
// ===============================

pub use anyhow;
pub use askama;
pub use axum;
pub use axum_extra;
pub use base64;
pub use chrono;
pub use chrono_tz;
pub use dotenvy;
pub use hmac;
pub use mysql;
pub use rand;
pub use serde;
pub use serde_json;
pub use sha2;
pub use subtle;
pub use tokio;
pub use tower;
pub use tower_http;
pub use uuid;

// ===============================
// Public modules
// ===============================
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod graphql;
pub mod image;
pub mod time;
pub mod web;
