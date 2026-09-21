//! # Database Port (Synchronous)
//!
//! Defines an abstract database interface ([`Db`]) and supporting types
//! used by adapters such as the MySQL implementation.
//!
//! This module provides:
//!
//! - [`Param`]: SQL parameter values passed to database adapters.
//! - [`Value`]: Generic owned values returned from database adapters.
//! - [`Row`]: A database row represented as a column-name/value map.
//! - [`Db`]: A minimal synchronous database abstraction.
//!
//! # Example
//!
//! ```rust,ignore
//! use wzs_web::db::port::{Db, params};
//!
//! let params = params![42u64, "Alice", true, None::<&str>];
//!
//! let id = db.exec_returning_last_insert_id(
//!     "INSERT INTO users VALUES (?, ?, ?, ?)",
//!     &params,
//! )?;
//! ```
//!
//! Nullable database columns can be read using the `*_opt` getters.
//!
//! ```rust,ignore
//! let parent_id = row.get_u64_opt("parent_id")?;
//!
//! match parent_id {
//!     Some(id) => println!("parent_id = {id}"),
//!     None => println!("parent_id is NULL"),
//! }
//! ```

use std::collections::HashMap;

use anyhow::{Result, bail};
use chrono::NaiveDateTime;
use uuid::Uuid;

/// SQL parameter types passed to a query.
///
/// Borrowed values are used where appropriate to avoid unnecessary
/// allocations when executing SQL statements.
#[derive(Debug)]
pub enum Param<'a> {
    /// Signed 64-bit integer.
    I64(i64),

    /// Unsigned 64-bit integer.
    U64(u64),

    /// 32-bit floating-point value.
    F32(f32),

    /// 64-bit floating-point value.
    F64(f64),

    /// Boolean value.
    Bool(bool),

    /// Borrowed UTF-8 string.
    Str(&'a str),

    /// Timezone-independent date and time.
    DateTime(NaiveDateTime),

    /// Borrowed binary data.
    Bin(&'a [u8]),

    /// SQL `NULL`.
    Null,
}

/// Generic owned database value used for row mapping.
///
/// Concrete database adapters convert database-specific values into this
/// representation before inserting them into a [`Row`].
#[derive(Debug, Clone)]
pub enum Value {
    /// Signed 64-bit integer.
    I64(i64),

    /// Unsigned 64-bit integer.
    U64(u64),

    /// 32-bit floating-point value.
    F32(f32),

    /// 64-bit floating-point value.
    F64(f64),

    /// Boolean value.
    Bool(bool),

    /// Owned UTF-8 string.
    Str(String),

    /// Timezone-independent date and time.
    DateTime(NaiveDateTime),

    /// Owned binary data.
    Bin(Vec<u8>),

    /// SQL `NULL`.
    Null,
}

/// Represents a single database row.
///
/// Columns are stored as a mapping from column names to generic [`Value`]
/// instances.
#[derive(Debug, Clone, Default)]
pub struct Row {
    cols: HashMap<String, Value>,
}

// -----------------------------------------------------------------------------
// Param conversions
// -----------------------------------------------------------------------------

impl<'a> From<i64> for Param<'a> {
    fn from(value: i64) -> Self {
        Param::I64(value)
    }
}

impl<'a> From<Option<i64>> for Param<'a> {
    fn from(value: Option<i64>) -> Self {
        match value {
            Some(value) => Param::I64(value),
            None => Param::Null,
        }
    }
}

impl<'a> From<u64> for Param<'a> {
    fn from(value: u64) -> Self {
        Param::U64(value)
    }
}

impl<'a> From<Option<u64>> for Param<'a> {
    fn from(value: Option<u64>) -> Self {
        match value {
            Some(value) => Param::U64(value),
            None => Param::Null,
        }
    }
}

impl<'a> From<f32> for Param<'a> {
    fn from(value: f32) -> Self {
        Param::F32(value)
    }
}

impl<'a> From<Option<f32>> for Param<'a> {
    fn from(value: Option<f32>) -> Self {
        match value {
            Some(value) => Param::F32(value),
            None => Param::Null,
        }
    }
}

impl<'a> From<f64> for Param<'a> {
    fn from(value: f64) -> Self {
        Param::F64(value)
    }
}

impl<'a> From<Option<f64>> for Param<'a> {
    fn from(value: Option<f64>) -> Self {
        match value {
            Some(value) => Param::F64(value),
            None => Param::Null,
        }
    }
}

impl<'a> From<bool> for Param<'a> {
    fn from(value: bool) -> Self {
        Param::Bool(value)
    }
}

impl<'a> From<Option<bool>> for Param<'a> {
    fn from(value: Option<bool>) -> Self {
        match value {
            Some(value) => Param::Bool(value),
            None => Param::Null,
        }
    }
}

impl<'a> From<&'a str> for Param<'a> {
    fn from(value: &'a str) -> Self {
        Param::Str(value)
    }
}

impl<'a> From<Option<&'a str>> for Param<'a> {
    fn from(value: Option<&'a str>) -> Self {
        match value {
            Some(value) => Param::Str(value),
            None => Param::Null,
        }
    }
}

impl<'a> From<NaiveDateTime> for Param<'a> {
    fn from(value: NaiveDateTime) -> Self {
        Param::DateTime(value)
    }
}

impl<'a> From<Option<NaiveDateTime>> for Param<'a> {
    fn from(value: Option<NaiveDateTime>) -> Self {
        match value {
            Some(value) => Param::DateTime(value),
            None => Param::Null,
        }
    }
}

impl<'a> From<&'a [u8]> for Param<'a> {
    fn from(value: &'a [u8]) -> Self {
        Param::Bin(value)
    }
}

impl<'a> From<Option<&'a [u8]>> for Param<'a> {
    fn from(value: Option<&'a [u8]>) -> Self {
        match value {
            Some(value) => Param::Bin(value),
            None => Param::Null,
        }
    }
}

impl<'a> From<&'a Uuid> for Param<'a> {
    fn from(value: &'a Uuid) -> Self {
        Param::Bin(value.as_bytes())
    }
}

impl<'a> From<Option<&'a Uuid>> for Param<'a> {
    fn from(value: Option<&'a Uuid>) -> Self {
        match value {
            Some(value) => Param::Bin(value.as_bytes()),
            None => Param::Null,
        }
    }
}

// -----------------------------------------------------------------------------
// params! macro
// -----------------------------------------------------------------------------

/// Builds a `Vec<Param>` for use with SQL queries.
///
/// Values are converted using [`Param::from`].
///
/// # Example
///
/// ```rust,ignore
/// use wzs_web::db::port::{Param, params};
///
/// let name = "Alice";
/// let age: u64 = 42;
/// let note: Option<&str> = None;
///
/// let ps = params![age, name, true, note];
///
/// assert!(matches!(ps[0], Param::U64(42)));
/// assert!(matches!(ps[1], Param::Str("Alice")));
/// assert!(matches!(ps[2], Param::Bool(true)));
/// assert!(matches!(ps[3], Param::Null));
/// ```
#[macro_export]
macro_rules! params {
    ($($x:expr),* $(,)?) => {
        vec![$(Param::from($x)),*]
    };
}

// -----------------------------------------------------------------------------
// Row helper methods
// -----------------------------------------------------------------------------

impl Row {
    /// Inserts or replaces a column value.
    ///
    /// This method is primarily intended for database adapters while
    /// converting native database rows into the generic [`Row`] type.
    pub fn insert(&mut self, key: impl Into<String>, val: Value) {
        self.cols.insert(key.into(), val);
    }

    /// Returns a `u64`.
    ///
    /// Accepts:
    ///
    /// - [`Value::U64`]
    /// - non-negative [`Value::I64`]
    ///
    /// # Errors
    ///
    /// Returns an error when the column is missing, contains an incompatible
    /// type, or contains a negative `i64`.
    pub fn get_u64(&self, key: &str) -> Result<u64> {
        match self.cols.get(key) {
            Some(Value::U64(v)) => Ok(*v),
            Some(Value::I64(v)) if *v >= 0 => Ok(*v as u64),
            _ => bail!("column `{key}` is not U64"),
        }
    }

    /// Returns an optional `u64`.
    ///
    /// Accepts:
    ///
    /// - [`Value::U64`] → `Some(value)`
    /// - non-negative [`Value::I64`] → `Some(value)`
    /// - [`Value::Null`] → `None`
    pub fn get_u64_opt(&self, key: &str) -> Result<Option<u64>> {
        match self.cols.get(key) {
            Some(Value::U64(v)) => Ok(Some(*v)),
            Some(Value::I64(v)) if *v >= 0 => Ok(Some(*v as u64)),
            Some(Value::Null) => Ok(None),
            Some(_) => bail!("column `{key}` is not U64/NULL"),
            None => bail!("column `{key}` not found"),
        }
    }

    /// Returns an `i64`.
    pub fn get_i64(&self, key: &str) -> Result<i64> {
        match self.cols.get(key) {
            Some(Value::I64(v)) => Ok(*v),
            _ => bail!("column `{key}` is not I64"),
        }
    }

    /// Returns an optional `i64`.
    ///
    /// - [`Value::I64`] → `Some(value)`
    /// - [`Value::Null`] → `None`
    pub fn get_i64_opt(&self, key: &str) -> Result<Option<i64>> {
        match self.cols.get(key) {
            Some(Value::I64(v)) => Ok(Some(*v)),
            Some(Value::Null) => Ok(None),
            Some(_) => bail!("column `{key}` is not I64/NULL"),
            None => bail!("column `{key}` not found"),
        }
    }

    /// Returns an `f32`.
    pub fn get_f32(&self, key: &str) -> Result<f32> {
        match self.cols.get(key) {
            Some(Value::F32(v)) => Ok(*v),
            _ => bail!("column `{key}` is not F32"),
        }
    }

    /// Returns an optional `f32`.
    ///
    /// - [`Value::F32`] → `Some(value)`
    /// - [`Value::Null`] → `None`
    pub fn get_f32_opt(&self, key: &str) -> Result<Option<f32>> {
        match self.cols.get(key) {
            Some(Value::F32(v)) => Ok(Some(*v)),
            Some(Value::Null) => Ok(None),
            Some(_) => bail!("column `{key}` is not F32/NULL"),
            None => bail!("column `{key}` not found"),
        }
    }

    /// Returns an `f64`.
    pub fn get_f64(&self, key: &str) -> Result<f64> {
        match self.cols.get(key) {
            Some(Value::F64(v)) => Ok(*v),
            _ => bail!("column `{key}` is not F64"),
        }
    }

    /// Returns an optional `f64`.
    ///
    /// - [`Value::F64`] → `Some(value)`
    /// - [`Value::Null`] → `None`
    pub fn get_f64_opt(&self, key: &str) -> Result<Option<f64>> {
        match self.cols.get(key) {
            Some(Value::F64(v)) => Ok(Some(*v)),
            Some(Value::Null) => Ok(None),
            Some(_) => bail!("column `{key}` is not F64/NULL"),
            None => bail!("column `{key}` not found"),
        }
    }

    /// Returns a `bool`.
    ///
    /// Accepted representations:
    ///
    /// - [`Value::Bool`]
    /// - [`Value::I64`], where zero is `false` and non-zero is `true`
    /// - [`Value::U64`], where zero is `false` and non-zero is `true`
    /// - [`Value::Str`] containing `"0"` or `"1"`
    pub fn get_bool(&self, key: &str) -> Result<bool> {
        match self.cols.get(key) {
            Some(Value::Bool(v)) => Ok(*v),
            Some(Value::I64(v)) => Ok(*v != 0),
            Some(Value::U64(v)) => Ok(*v != 0),
            Some(Value::Str(s)) if s == "0" || s == "1" => Ok(s != "0"),
            _ => bail!("column `{key}` is not Bool"),
        }
    }

    /// Returns an optional `bool`.
    ///
    /// Accepted representations are the same as [`Row::get_bool`].
    ///
    /// - boolean-compatible value → `Some(value)`
    /// - [`Value::Null`] → `None`
    pub fn get_bool_opt(&self, key: &str) -> Result<Option<bool>> {
        match self.cols.get(key) {
            Some(Value::Bool(v)) => Ok(Some(*v)),
            Some(Value::I64(v)) => Ok(Some(*v != 0)),
            Some(Value::U64(v)) => Ok(Some(*v != 0)),
            Some(Value::Str(s)) if s == "0" || s == "1" => Ok(Some(s != "0")),
            Some(Value::Null) => Ok(None),
            Some(_) => bail!("column `{key}` is not Bool/NULL"),
            None => bail!("column `{key}` not found"),
        }
    }

    /// Returns a cloned `String`.
    pub fn get_string(&self, key: &str) -> Result<String> {
        match self.cols.get(key) {
            Some(Value::Str(s)) => Ok(s.clone()),
            _ => bail!("column `{key}` is not String"),
        }
    }

    /// Returns an optional `String`.
    ///
    /// - [`Value::Str`] → `Some(String)`
    /// - [`Value::Null`] → `None`
    pub fn get_string_opt(&self, key: &str) -> Result<Option<String>> {
        match self.cols.get(key) {
            Some(Value::Str(s)) => Ok(Some(s.clone())),
            Some(Value::Null) => Ok(None),
            Some(_) => bail!("column `{key}` is not String/NULL"),
            None => bail!("column `{key}` not found"),
        }
    }

    /// Returns a [`NaiveDateTime`].
    pub fn get_datetime(&self, key: &str) -> Result<NaiveDateTime> {
        match self.cols.get(key) {
            Some(Value::DateTime(dt)) => Ok(*dt),
            _ => bail!("column `{key}` is not DateTime"),
        }
    }

    /// Returns an optional [`NaiveDateTime`].
    ///
    /// - [`Value::DateTime`] → `Some(value)`
    /// - [`Value::Null`] → `None`
    pub fn get_datetime_opt(&self, key: &str) -> Result<Option<NaiveDateTime>> {
        match self.cols.get(key) {
            Some(Value::DateTime(dt)) => Ok(Some(*dt)),
            Some(Value::Null) => Ok(None),
            Some(_) => bail!("column `{key}` is not DateTime/NULL"),
            None => bail!("column `{key}` not found"),
        }
    }

    /// Returns binary data as a cloned `Vec<u8>`.
    pub fn get_bin(&self, key: &str) -> Result<Vec<u8>> {
        match self.cols.get(key) {
            Some(Value::Bin(b)) => Ok(b.clone()),
            _ => bail!("column `{key}` is not Bin"),
        }
    }

    /// Returns optional binary data as a cloned `Vec<u8>`.
    ///
    /// - [`Value::Bin`] → `Some(Vec<u8>)`
    /// - [`Value::Null`] → `None`
    pub fn get_bin_opt(&self, key: &str) -> Result<Option<Vec<u8>>> {
        match self.cols.get(key) {
            Some(Value::Bin(b)) => Ok(Some(b.clone())),
            Some(Value::Null) => Ok(None),
            Some(_) => bail!("column `{key}` is not Bin/NULL"),
            None => bail!("column `{key}` not found"),
        }
    }

    /// Returns a [`Uuid`] decoded from binary data.
    ///
    /// The value is expected to contain a UUID-compatible binary
    /// representation, normally from a `BINARY(16)` column.
    pub fn get_uuid(&self, key: &str) -> Result<Uuid> {
        let b = self.get_bin(key)?;

        Uuid::from_slice(&b).map_err(|_| anyhow::anyhow!("column `{key}` is not valid UUID bytes"))
    }

    /// Returns an optional [`Uuid`] decoded from binary data.
    ///
    /// - valid [`Value::Bin`] → `Some(Uuid)`
    /// - [`Value::Null`] → `None`
    ///
    /// # Errors
    ///
    /// Returns an error when the column is missing, contains an incompatible
    /// type, or contains invalid UUID bytes.
    pub fn get_uuid_opt(&self, key: &str) -> Result<Option<Uuid>> {
        match self.cols.get(key) {
            Some(Value::Bin(b)) => {
                let uuid = Uuid::from_slice(b)
                    .map_err(|_| anyhow::anyhow!("column `{key}` is not valid UUID bytes"))?;

                Ok(Some(uuid))
            }
            Some(Value::Null) => Ok(None),
            Some(_) => bail!("column `{key}` is not UUID/NULL"),
            None => bail!("column `{key}` not found"),
        }
    }
}

/// Builds a `Vec<Param>` without using the [`params!`] macro.
pub fn params<'a>(xs: impl Into<Vec<Param<'a>>>) -> Vec<Param<'a>> {
    xs.into()
}

/// Synchronous database abstraction.
///
/// Repository code can depend on this trait rather than directly depending
/// on a specific database library.
pub trait Db: Send + Sync + 'static {
    /// Executes a query expected to return zero or one row.
    fn fetch_one(&self, sql: &str, params: &[Param]) -> Result<Option<Row>>;

    /// Executes a query and returns all rows.
    fn fetch_all(&self, sql: &str, params: &[Param]) -> Result<Vec<Row>>;

    /// Executes a write operation such as `INSERT`, `UPDATE`, or `DELETE`.
    ///
    /// Returns the number of affected rows.
    fn exec(&self, sql: &str, params: &[Param]) -> Result<u64>;

    /// Executes an insert and returns the database-generated last insert ID.
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
    fn params_macro_accepts_f32_and_f64() {
        let x_f32: f32 = 1.5;
        let x_f64: f64 = 12.34567;

        let v = params![x_f32, x_f64];

        assert!(matches!(
           v[0],
           Param::F32(f) if (f - 1.5).abs() < 1e-6
        ));

        assert!(matches!(
           v[1],
           Param::F64(f) if (f - 12.34567).abs() < 1e-12
        ));
    }

    #[test]
    fn row_getters_happy_paths() {
        let mut r = Row::default();

        let dt = NaiveDate::from_ymd_opt(2026, 9, 13)
            .unwrap()
            .and_hms_opt(12, 34, 56)
            .unwrap();

        let uuid = Uuid::new_v4();

        r.insert("u64", Value::U64(7));
        r.insert("i64", Value::I64(-3));
        r.insert("f32", Value::F32(1.5));
        r.insert("f64", Value::F64(12.34567));
        r.insert("bool", Value::Bool(true));
        r.insert("str", Value::Str("hello".into()));
        r.insert("dt", Value::DateTime(dt));
        r.insert("bin", Value::Bin(vec![1, 2, 3]));
        r.insert("uuid", Value::Bin(uuid.as_bytes().to_vec()));

        assert_eq!(r.get_u64("u64").unwrap(), 7);
        assert_eq!(r.get_i64("i64").unwrap(), -3);
        assert!((r.get_f32("f32").unwrap() - 1.5).abs() < 1e-6);
        assert!((r.get_f64("f64").unwrap() - 12.34567).abs() < 1e-12);
        assert!(r.get_bool("bool").unwrap());
        assert_eq!(r.get_string("str").unwrap(), "hello");
        assert_eq!(r.get_datetime("dt").unwrap(), dt);
        assert_eq!(r.get_bin("bin").unwrap(), vec![1, 2, 3]);
        assert_eq!(r.get_uuid("uuid").unwrap(), uuid);
    }

    #[test]
    fn row_get_u64_accepts_non_negative_i64() {
        let mut r = Row::default();

        r.insert("positive", Value::I64(10));
        r.insert("negative", Value::I64(-1));

        assert_eq!(r.get_u64("positive").unwrap(), 10);
        assert!(r.get_u64("negative").is_err());
    }

    #[test]
    fn row_get_bool_accepts_supported_representations() {
        let mut r = Row::default();

        r.insert("bool_true", Value::Bool(true));
        r.insert("bool_false", Value::Bool(false));
        r.insert("i64_true", Value::I64(1));
        r.insert("i64_false", Value::I64(0));
        r.insert("u64_true", Value::U64(42));
        r.insert("u64_false", Value::U64(0));
        r.insert("str_true", Value::Str("1".into()));
        r.insert("str_false", Value::Str("0".into()));

        assert!(r.get_bool("bool_true").unwrap());
        assert!(!r.get_bool("bool_false").unwrap());
        assert!(r.get_bool("i64_true").unwrap());
        assert!(!r.get_bool("i64_false").unwrap());
        assert!(r.get_bool("u64_true").unwrap());
        assert!(!r.get_bool("u64_false").unwrap());
        assert!(r.get_bool("str_true").unwrap());
        assert!(!r.get_bool("str_false").unwrap());
    }

    #[test]
    fn param_optional_conversions_work() {
        let dt = NaiveDate::from_ymd_opt(2026, 9, 13)
            .unwrap()
            .and_hms_opt(12, 34, 56)
            .unwrap();

        let uuid = Uuid::new_v4();
        let bytes: &[u8] = &[1, 2, 3];

        let v = params![
            Some(-10i64),
            None::<i64>,
            Some(10u64),
            None::<u64>,
            Some(1.5f32),
            None::<f32>,
            Some(3.25f64),
            None::<f64>,
            Some(true),
            None::<bool>,
            Some("hello"),
            None::<&str>,
            Some(dt),
            None::<NaiveDateTime>,
            Some(bytes),
            None::<&[u8]>,
            Some(&uuid),
            None::<&Uuid>,
        ];

        assert!(matches!(v[0], Param::I64(-10)));
        assert!(matches!(v[1], Param::Null));

        assert!(matches!(v[2], Param::U64(10)));
        assert!(matches!(v[3], Param::Null));

        assert!(matches!(
            v[4],
            Param::F32(value) if (value - 1.5).abs() < f32::EPSILON
        ));
        assert!(matches!(v[5], Param::Null));

        assert!(matches!(
            v[6],
            Param::F64(value) if (value - 3.25).abs() < f64::EPSILON
        ));
        assert!(matches!(v[7], Param::Null));

        assert!(matches!(v[8], Param::Bool(true)));
        assert!(matches!(v[9], Param::Null));

        assert!(matches!(v[10], Param::Str("hello")));
        assert!(matches!(v[11], Param::Null));

        assert!(matches!(
            v[12],
            Param::DateTime(value) if value == dt
        ));
        assert!(matches!(v[13], Param::Null));

        assert!(matches!(
            &v[14],
            Param::Bin(value) if *value == bytes
        ));
        assert!(matches!(v[15], Param::Null));

        assert!(matches!(
            &v[16],
            Param::Bin(value) if *value == uuid.as_bytes()
        ));
        assert!(matches!(v[17], Param::Null));
    }

    #[test]
    fn row_optional_getters_return_some() {
        let mut r = Row::default();

        let dt = NaiveDate::from_ymd_opt(2026, 9, 13)
            .unwrap()
            .and_hms_opt(12, 34, 56)
            .unwrap();

        let uuid = Uuid::new_v4();

        r.insert("u64", Value::U64(1));
        r.insert("i64", Value::I64(-2));
        r.insert("f32", Value::F32(1.5));
        r.insert("f64", Value::F64(3.5));
        r.insert("bool", Value::Bool(true));
        r.insert("str", Value::Str("hello".into()));
        r.insert("dt", Value::DateTime(dt));
        r.insert("bin", Value::Bin(vec![1, 2, 3]));
        r.insert("uuid", Value::Bin(uuid.as_bytes().to_vec()));

        assert_eq!(r.get_u64_opt("u64").unwrap(), Some(1));
        assert_eq!(r.get_i64_opt("i64").unwrap(), Some(-2));

        assert!(matches!(
           r.get_f32_opt("f32").unwrap(),
           Some(v) if (v - 1.5).abs() < 1e-6
        ));

        assert!(matches!(
           r.get_f64_opt("f64").unwrap(),
           Some(v) if (v - 3.5).abs() < 1e-12
        ));

        assert_eq!(r.get_bool_opt("bool").unwrap(), Some(true));
        assert_eq!(r.get_string_opt("str").unwrap(), Some("hello".to_string()));
        assert_eq!(r.get_datetime_opt("dt").unwrap(), Some(dt));
        assert_eq!(r.get_bin_opt("bin").unwrap(), Some(vec![1, 2, 3]));
        assert_eq!(r.get_uuid_opt("uuid").unwrap(), Some(uuid));
    }

    #[test]
    fn row_optional_getters_return_none_for_null() {
        let mut r = Row::default();

        r.insert("u64", Value::Null);
        r.insert("i64", Value::Null);
        r.insert("f32", Value::Null);
        r.insert("f64", Value::Null);
        r.insert("bool", Value::Null);
        r.insert("str", Value::Null);
        r.insert("dt", Value::Null);
        r.insert("bin", Value::Null);
        r.insert("uuid", Value::Null);

        assert_eq!(r.get_u64_opt("u64").unwrap(), None);
        assert_eq!(r.get_i64_opt("i64").unwrap(), None);
        assert_eq!(r.get_f32_opt("f32").unwrap(), None);
        assert_eq!(r.get_f64_opt("f64").unwrap(), None);
        assert_eq!(r.get_bool_opt("bool").unwrap(), None);
        assert_eq!(r.get_string_opt("str").unwrap(), None);
        assert_eq!(r.get_datetime_opt("dt").unwrap(), None);
        assert_eq!(r.get_bin_opt("bin").unwrap(), None);
        assert_eq!(r.get_uuid_opt("uuid").unwrap(), None);
    }

    #[test]
    fn row_get_u64_opt_accepts_non_negative_i64() {
        let mut r = Row::default();

        r.insert("positive", Value::I64(10));
        r.insert("zero", Value::I64(0));

        assert_eq!(r.get_u64_opt("positive").unwrap(), Some(10));
        assert_eq!(r.get_u64_opt("zero").unwrap(), Some(0));
    }

    #[test]
    fn row_get_u64_opt_rejects_negative_i64() {
        let mut r = Row::default();

        r.insert("value", Value::I64(-1));

        let err = r.get_u64_opt("value").unwrap_err().to_string();

        assert!(err.contains("is not U64/NULL"));
    }

    #[test]
    fn row_get_bool_opt_accepts_supported_representations() {
        let mut r = Row::default();

        r.insert("bool", Value::Bool(true));
        r.insert("i64_true", Value::I64(-1));
        r.insert("i64_false", Value::I64(0));
        r.insert("u64_true", Value::U64(1));
        r.insert("u64_false", Value::U64(0));
        r.insert("str_true", Value::Str("1".into()));
        r.insert("str_false", Value::Str("0".into()));

        assert_eq!(r.get_bool_opt("bool").unwrap(), Some(true));
        assert_eq!(r.get_bool_opt("i64_true").unwrap(), Some(true));
        assert_eq!(r.get_bool_opt("i64_false").unwrap(), Some(false));
        assert_eq!(r.get_bool_opt("u64_true").unwrap(), Some(true));
        assert_eq!(r.get_bool_opt("u64_false").unwrap(), Some(false));
        assert_eq!(r.get_bool_opt("str_true").unwrap(), Some(true));
        assert_eq!(r.get_bool_opt("str_false").unwrap(), Some(false));
    }

    #[test]
    fn row_optional_getters_reject_wrong_types() {
        let mut r = Row::default();

        r.insert("u64", Value::Str("1".into()));
        r.insert("i64", Value::Bool(true));
        r.insert("f32", Value::I64(1));
        r.insert("f64", Value::U64(1));
        r.insert("bool", Value::Str("yes".into()));
        r.insert("str", Value::U64(1));
        r.insert("dt", Value::Str("2026-09-13".into()));
        r.insert("bin", Value::Bool(true));
        r.insert("uuid", Value::Str("not-a-uuid".into()));

        assert!(
            r.get_u64_opt("u64")
                .unwrap_err()
                .to_string()
                .contains("is not U64/NULL")
        );

        assert!(
            r.get_i64_opt("i64")
                .unwrap_err()
                .to_string()
                .contains("is not I64/NULL")
        );

        assert!(
            r.get_f32_opt("f32")
                .unwrap_err()
                .to_string()
                .contains("is not F32/NULL")
        );

        assert!(
            r.get_f64_opt("f64")
                .unwrap_err()
                .to_string()
                .contains("is not F64/NULL")
        );

        assert!(
            r.get_bool_opt("bool")
                .unwrap_err()
                .to_string()
                .contains("is not Bool/NULL")
        );

        assert!(
            r.get_string_opt("str")
                .unwrap_err()
                .to_string()
                .contains("is not String/NULL")
        );

        assert!(
            r.get_datetime_opt("dt")
                .unwrap_err()
                .to_string()
                .contains("is not DateTime/NULL")
        );

        assert!(
            r.get_bin_opt("bin")
                .unwrap_err()
                .to_string()
                .contains("is not Bin/NULL")
        );

        assert!(
            r.get_uuid_opt("uuid")
                .unwrap_err()
                .to_string()
                .contains("is not UUID/NULL")
        );
    }

    #[test]
    fn row_optional_getters_error_when_column_is_missing() {
        let r = Row::default();

        assert!(
            r.get_u64_opt("missing")
                .unwrap_err()
                .to_string()
                .contains("column `missing` not found")
        );

        assert!(
            r.get_i64_opt("missing")
                .unwrap_err()
                .to_string()
                .contains("column `missing` not found")
        );

        assert!(
            r.get_f32_opt("missing")
                .unwrap_err()
                .to_string()
                .contains("column `missing` not found")
        );

        assert!(
            r.get_f64_opt("missing")
                .unwrap_err()
                .to_string()
                .contains("column `missing` not found")
        );

        assert!(
            r.get_bool_opt("missing")
                .unwrap_err()
                .to_string()
                .contains("column `missing` not found")
        );

        assert!(
            r.get_string_opt("missing")
                .unwrap_err()
                .to_string()
                .contains("column `missing` not found")
        );

        assert!(
            r.get_datetime_opt("missing")
                .unwrap_err()
                .to_string()
                .contains("column `missing` not found")
        );

        assert!(
            r.get_bin_opt("missing")
                .unwrap_err()
                .to_string()
                .contains("column `missing` not found")
        );

        assert!(
            r.get_uuid_opt("missing")
                .unwrap_err()
                .to_string()
                .contains("column `missing` not found")
        );
    }

    #[test]
    fn row_get_uuid_errors_on_invalid_uuid_bytes() {
        let mut r = Row::default();

        r.insert("uuid", Value::Bin(vec![1, 2, 3]));

        let err = r.get_uuid("uuid").unwrap_err().to_string();

        assert!(err.contains("not valid UUID bytes"));
    }

    #[test]
    fn row_get_uuid_opt_errors_on_invalid_uuid_bytes() {
        let mut r = Row::default();

        r.insert("uuid", Value::Bin(vec![1, 2, 3]));

        let err = r.get_uuid_opt("uuid").unwrap_err().to_string();

        assert!(err.contains("not valid UUID bytes"));
    }

    #[test]
    fn row_non_optional_getters_reject_wrong_types() {
        let mut r = Row::default();

        r.insert("u64", Value::Str("1".into()));
        r.insert("i64", Value::Str("1".into()));
        r.insert("f32", Value::Str("1.0".into()));
        r.insert("f64", Value::Str("1.0".into()));
        r.insert("bool", Value::Str("true".into()));
        r.insert("str", Value::Bool(true));
        r.insert("dt", Value::Str("2026-09-13".into()));
        r.insert("bin", Value::Bool(true));

        assert!(
            r.get_u64("u64")
                .unwrap_err()
                .to_string()
                .contains("is not U64")
        );

        assert!(
            r.get_i64("i64")
                .unwrap_err()
                .to_string()
                .contains("is not I64")
        );

        assert!(
            r.get_f32("f32")
                .unwrap_err()
                .to_string()
                .contains("is not F32")
        );

        assert!(
            r.get_f64("f64")
                .unwrap_err()
                .to_string()
                .contains("is not F64")
        );

        assert!(
            r.get_bool("bool")
                .unwrap_err()
                .to_string()
                .contains("is not Bool")
        );

        assert!(
            r.get_string("str")
                .unwrap_err()
                .to_string()
                .contains("is not String")
        );

        assert!(
            r.get_datetime("dt")
                .unwrap_err()
                .to_string()
                .contains("is not DateTime")
        );

        assert!(
            r.get_bin("bin")
                .unwrap_err()
                .to_string()
                .contains("is not Bin")
        );
    }
}
