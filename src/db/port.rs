//! # Database Port (Synchronous)
//!
//! Defines an abstract database interface (`Db`) and supporting types
//! used by adapters such as the MySQL implementation.
//!
//! - [`Param`]: Represents SQL parameters.
//! - [`Value`] / [`Row`]: Generic owned data representations.
//! - [`Db`]: Defines minimal operations (`fetch_one`, `fetch_all`, `exec`, etc.).
//!
//! # Example
//! ```rust,ignore
//! use wzs_web::db::port::{Db, params};
//!
//! // Repository example (pseudo-code)
//! let params = params![42u64, "Alice", true, None::<&str>]; // last is NULL
//! let id = db.exec_returning_last_insert_id("INSERT INTO users VALUES (?, ?, ?, ?)", &params)?;
//! ```
use std::collections::HashMap;

use anyhow::{bail, Result};
use chrono::NaiveDateTime;
use uuid::Uuid;

/// SQL parameter types passed to a query.
///
/// - `Str(&str)` holds a borrowed string reference.
/// - `Null` represents an SQL NULL.
/// - `DateTime` uses [`NaiveDateTime`] (no time zone).
#[derive(Debug)]
pub enum Param<'a> {
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    Bool(bool),
    Str(&'a str),
    DateTime(NaiveDateTime),
    Bin(&'a [u8]), // BINARY/VARBINARY 用
    Null,
}

/// Generic owned database value used for row mapping.
#[derive(Debug, Clone)]
pub enum Value {
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    Bool(bool),
    Str(String),
    DateTime(NaiveDateTime),
    Bin(Vec<u8>), // 所有データとして保持（ライフタイム不要）
    Null,
}

/// Represents a single database row (column name → value map).
#[derive(Debug, Clone, Default)]
pub struct Row {
    cols: HashMap<String, Value>,
}

// ------------------------------
// Param conversions (From impls)
// ------------------------------

impl<'a> From<i64> for Param<'a> {
    fn from(x: i64) -> Self {
        Param::I64(x)
    }
}

impl<'a> From<u64> for Param<'a> {
    fn from(x: u64) -> Self {
        Param::U64(x)
    }
}

impl<'a> From<f32> for Param<'a> {
    fn from(x: f32) -> Self {
        Param::F32(x)
    }
}

impl<'a> From<f64> for Param<'a> {
    fn from(x: f64) -> Self {
        Param::F64(x)
    }
}

impl<'a> From<bool> for Param<'a> {
    fn from(x: bool) -> Self {
        Param::Bool(x)
    }
}

impl<'a> From<&'a str> for Param<'a> {
    fn from(x: &'a str) -> Self {
        Param::Str(x)
    }
}

impl<'a> From<Option<&'a str>> for Param<'a> {
    fn from(x: Option<&'a str>) -> Self {
        match x {
            Some(s) => Param::Str(s),
            None => Param::Null,
        }
    }
}

impl<'a> From<&'a [u8]> for Param<'a> {
    fn from(x: &'a [u8]) -> Self {
        Param::Bin(x)
    }
}

impl<'a> From<Option<&'a [u8]>> for Param<'a> {
    fn from(x: Option<&'a [u8]>) -> Self {
        match x {
            Some(b) => Param::Bin(b),
            None => Param::Null,
        }
    }
}

impl<'a> From<&'a Uuid> for Param<'a> {
    fn from(u: &'a Uuid) -> Self {
        Param::Bin(u.as_bytes())
    }
}

// ------------------------------------
// params! macro
// ------------------------------------

/// Macro to easily build a `Vec<Param>` for SQL queries.
///
/// # Example
/// ```rust,ignore
/// use wzs_web::db::port::{Param, params};
///
/// let name = "Alice";
/// let age: u64 = 42;
/// let note: Option<&str> = None; // becomes NULL
///
/// let ps = params![age, name, true, note];
/// assert!(matches!(ps[0], Param::U64(42)));
/// assert!(matches!(ps[1], Param::Str("Alice")));
/// assert!(matches!(ps[2], Param::Bool(true)));
/// assert!(matches!(ps[3], Param::Null));
/// ```
#[macro_export]
macro_rules! params {
    ($($x:expr),* $(,)?) => {{
       let mut v = Vec::<Param>::new();
       $( v.push(Param::from($x)); )*
          v
    }};
}

// ------------------------------
// Row helper methods
// ------------------------------

impl Row {
    /// Inserts a new column (used internally by DB adapters).
    pub fn insert(&mut self, key: impl Into<String>, val: Value) {
        self.cols.insert(key.into(), val);
    }

    /// Returns a `u64` (accepts non-negative `i64`).
    pub fn get_u64(&self, key: &str) -> Result<u64> {
        match self.cols.get(key) {
            Some(Value::U64(v)) => Ok(*v),
            Some(Value::I64(v)) if *v >= 0 => Ok(*v as u64),
            _ => bail!("column `{key}` is not U64"),
        }
    }

    /// Returns an `i64`.
    pub fn get_i64(&self, key: &str) -> Result<i64> {
        match self.cols.get(key) {
            Some(Value::I64(v)) => Ok(*v),
            _ => bail!("column `{key}` is not I64"),
        }
    }

    /// Returns a `u64` (accepts non-negative `i64`).
    pub fn get_f32(&self, key: &str) -> Result<f32> {
        match self.cols.get(key) {
            Some(Value::F32(v)) => Ok(*v),
            _ => bail!("column `{key}` is not F32"),
        }
    }

    /// Returns an `i64`.
    pub fn get_f64(&self, key: &str) -> Result<f64> {
        match self.cols.get(key) {
            Some(Value::F64(v)) => Ok(*v),
            _ => bail!("column `{key}` is not F64"),
        }
    }

    /// Returns a `bool`.
    ///
    /// Accepts:
    /// - `Bool` directly
    /// - Numeric values (`I64`, `U64`) where non-zero = `true`
    /// - Strings `"0"` or `"1"`
    pub fn get_bool(&self, key: &str) -> Result<bool> {
        match self.cols.get(key) {
            Some(Value::Bool(v)) => Ok(*v),
            Some(Value::I64(v)) => Ok(*v != 0),
            Some(Value::U64(v)) => Ok(*v != 0),
            Some(Value::Str(s)) if s == "0" || s == "1" => Ok(s != "0"),
            _ => bail!("column `{key}` is not Bool"),
        }
    }

    /// Returns a `String` (only for `Value::Str`).
    pub fn get_string(&self, key: &str) -> Result<String> {
        match self.cols.get(key) {
            Some(Value::Str(s)) => Ok(s.clone()),
            _ => bail!("column `{key}` is not String"),
        }
    }

    /// Returns a [`NaiveDateTime`].
    pub fn get_datetime(&self, key: &str) -> Result<NaiveDateTime> {
        match self.cols.get(key) {
            Some(Value::DateTime(dt)) => Ok(*dt),
            _ => bail!("column `{key}` is not DateTime"),
        }
    }

    /// Returns a binary `Vec<u8>` (clone of internal data).
    pub fn get_bin(&self, key: &str) -> Result<Vec<u8>> {
        match self.cols.get(key) {
            Some(Value::Bin(b)) => Ok(b.clone()),
            _ => bail!("column `{key}` is not Bin"),
        }
    }

    /// Returns a [`Uuid`] from a BINARY(16) column.
    pub fn get_uuid(&self, key: &str) -> Result<Uuid> {
        let b = self.get_bin(key)?;
        Uuid::from_slice(&b).map_err(|_| anyhow::anyhow!("column `{key}` is not valid UUID bytes"))
    }

    /// Returns an optional `String` (`NULL` → `None`).
    pub fn get_string_opt(&self, key: &str) -> Result<Option<String>> {
        match self.cols.get(key) {
            Some(Value::Str(s)) => Ok(Some(s.clone())),
            Some(Value::Null) => Ok(None),
            Some(_) => bail!("column `{key}` is not String/NULL"),
            None => bail!("column `{key}` not found"),
        }
    }

    /// Returns an optional [`NaiveDateTime`] (`NULL` → `None`).
    pub fn get_datetime_opt(&self, key: &str) -> Result<Option<NaiveDateTime>> {
        match self.cols.get(key) {
            Some(Value::DateTime(dt)) => Ok(Some(*dt)),
            Some(Value::Null) => Ok(None),
            Some(_) => bail!("column `{key}` is not DateTime/NULL"),
            None => bail!("column `{key}` not found"),
        }
    }
}

/// Helper to build `Vec<Param>` without using the [`params!`] macro.
pub fn params<'a>(xs: impl Into<Vec<Param<'a>>>) -> Vec<Param<'a>> {
    xs.into()
}

/// Database abstraction (synchronous).
///
/// For async support, define an equivalent trait with `async_trait`.
#[cfg_attr(test, mockall::automock)]
pub trait Db: Send + Sync + 'static {
    fn fetch_one(&self, sql: &str, params: &[Param]) -> Result<Option<Row>>;

    fn fetch_all(&self, sql: &str, params: &[Param]) -> Result<Vec<Row>>;

    /// Execute a write operation (`INSERT`, `UPDATE`, `DELETE`).
    ///
    /// Returns affected row count.
    fn exec(&self, sql: &str, params: &[Param]) -> Result<u64>;

    /// Execute and return `LAST_INSERT_ID()` (for inserts).
    fn exec_returning_last_insert_id(&self, sql: &str, params: &[Param]) -> Result<u64>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn params_macro_and_from_impls_work() {
        let note: Option<&str> = None;
        let v = params![123u64, -5i64, "abc", true, note];

        assert!(matches!(v[0], Param::U64(123)));
        assert!(matches!(v[1], Param::I64(-5)));
        assert!(matches!(v[2], Param::Str("abc")));
        assert!(matches!(v[3], Param::Bool(true)));
        assert!(matches!(v[4], Param::Null));
    }

    #[test]
    fn row_getters_happy_paths() {
        let mut r = Row::default();
        let dt = NaiveDate::from_ymd_opt(2024, 7, 9)
            .unwrap()
            .and_hms_opt(12, 34, 56)
            .unwrap();

        r.insert("u64", Value::U64(7));
        r.insert("i64", Value::I64(-3));
        r.insert("bool_t", Value::Bool(true));
        r.insert("bool_i", Value::I64(1));
        r.insert("bool_u", Value::U64(0));
        r.insert("bool_s1", Value::Str("1".into()));
        r.insert("bool_s0", Value::Str("0".into()));
        r.insert("str", Value::Str("hello".into()));
        r.insert("dt", Value::DateTime(dt));
        r.insert("opt_str", Value::Null);
        r.insert("opt_dt", Value::Null);

        assert_eq!(r.get_u64("u64").unwrap(), 7);
        assert_eq!(r.get_i64("i64").unwrap(), -3);
        assert_eq!(r.get_bool("bool_t").unwrap(), true);
        assert_eq!(r.get_bool("bool_i").unwrap(), true);
        assert_eq!(r.get_bool("bool_u").unwrap(), false);
        assert_eq!(r.get_bool("bool_s1").unwrap(), true);
        assert_eq!(r.get_bool("bool_s0").unwrap(), false);
        assert_eq!(r.get_string("str").unwrap(), "hello");
        assert_eq!(r.get_datetime("dt").unwrap(), dt);
        assert_eq!(r.get_string_opt("opt_str").unwrap(), None);
        assert_eq!(r.get_datetime_opt("opt_dt").unwrap(), None);
    }

    #[test]
    fn row_getters_type_mismatch_errors() {
        let mut r = Row::default();
        r.insert("x", Value::Str("abc".into()));

        let e = r.get_u64("x").unwrap_err().to_string();
        assert!(e.contains("is not U64"));

        let e = r.get_string("missing").unwrap_err().to_string();
        assert!(e.contains("not String") || e.contains("not found"));
    }

    #[test]
    fn row_get_u64_accepts_non_negative_i64() {
        let mut r = Row::default();
        r.insert("pos_i64", Value::I64(10));
        r.insert("neg_i64", Value::I64(-1));

        assert_eq!(r.get_u64("pos_i64").unwrap(), 10);
        assert!(r.get_u64("neg_i64").is_err());
    }

    #[test]
    fn params_macro_accepts_f32_f64() {
        let x_f32: f32 = 1.5;
        let x_f64: f64 = 3.14159;

        let v = params![x_f32, x_f64];

        assert!(matches!(v[0], Param::F32(f) if (f - 1.5).abs() < 1e-6));
        assert!(matches!(v[1], Param::F64(f) if (f - 3.14159).abs() < 1e-12));
    }

    #[test]
    fn row_get_f32_and_f64() {
        let mut r = Row::default();

        r.insert("f32col", Value::F32(1.5));
        r.insert("f64col", Value::F64(3.14159));

        // get_f32
        let v32 = r.get_f32("f32col").unwrap();
        assert!((v32 - 1.5).abs() < 1e-6);

        // get_f64
        let v64 = r.get_f64("f64col").unwrap();
        assert!((v64 - 3.14159).abs() < 1e-12);
    }

    #[test]
    fn row_get_f32_and_f64_errors_on_wrong_type() {
        let mut r = Row::default();

        r.insert("not_f32", Value::Str("abc".into()));
        r.insert("not_f64", Value::Bool(true));

        let e1 = r.get_f32("not_f32").unwrap_err().to_string();
        assert!(e1.contains("is not F32"));

        let e2 = r.get_f64("not_f64").unwrap_err().to_string();
        assert!(e2.contains("is not F64"));
    }
}
