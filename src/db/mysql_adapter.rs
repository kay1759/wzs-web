//! # MySQL Database Adapter
//!
//! An implementation of the [`Db`] port using the [`mysql`] driver crate.
//! It provides MySQL-specific conversions and query execution helpers for the
//! application’s infrastructure layer.
//!
//! ## Responsibilities
//! - Convert generic [`Param`] values into [`mysql::Value`]
//! - Convert [`mysql::Row`] into a generic [`Row`]
//! - Implement `fetch_one`, `fetch_all`, `exec`, and
//!   `exec_returning_last_insert_id` using `mysql::Pool`
//!
//! ## Testing Policy
//! - Unit tests focus only on pure conversion functions
//!   (`to_mysql_value` / `to_mysql_params`).
//! - Integration tests should verify database I/O behaviors such as
//!   `row_from_mysql` and query execution.
//!
//! Example integration test setup (to be enabled via `--features mysql_integration`):
//!
//! ```ignore
//! #[test]
//! #[cfg(feature = "mysql_integration")]
//! fn integration_fetches_rows() {
//!     // Use a real MySQL instance and check fetch_one/fetch_all behavior.
//! }
//! ```

use std::sync::{Arc, OnceLock};

use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use mysql::{prelude::*, Error as MyError, Params, Pool, Value as My};

use crate::db::port::{Db, Param, Row as GRow, Value};

static SQL_DEBUG: OnceLock<bool> = OnceLock::new();

#[inline]
fn sql_debug() -> bool {
    *SQL_DEBUG.get_or_init(|| std::env::var_os("SQL_DEBUG").is_some())
}

macro_rules! dbglog {
    ($($arg:tt)*) => {
       if sql_debug() { eprintln!($($arg)*); }
    }
}

#[inline]
fn mysql_err_summary(e: &MyError) -> String {
    match e {
        &MyError::MySqlError(ref me) => format!(
            "code={}, state={}, message={}",
            me.code, me.state, me.message
        ),
        &MyError::DriverError(ref de) => format!("driver={de:?}"),
        &MyError::UrlError(ref ue) => format!("url={ue:?}"),
        &MyError::IoError(ref ioe) => format!("io={ioe}"),
        &MyError::CodecError(ref ce) => format!("codec={ce:?}"),
        &MyError::FromValueError(ref fve) => format!("from_value={fve:?}"),
        &MyError::FromRowError(ref fre) => format!("from_row={fre:?}"),
    }
}

#[inline]
fn log_who_where(conn: &mut mysql::PooledConn) {
    if !sql_debug() {
        return;
    }
    if let Ok(Some(row)) = conn.query_first::<(String, String, String, String), _>(
        "SELECT CURRENT_USER(), USER(), DATABASE(), @@hostname",
    ) {
        eprintln!("who/where = {:?}", row);
    }
}

/// MySQL implementation of the [`Db`] port.
///
/// - Wraps a connection pool (`mysql::Pool`) for query execution.
/// - Propagates errors as [`anyhow::Error`].
#[derive(Clone)]
pub struct MySqlDb {
    pool: Arc<Pool>,
}

impl MySqlDb {
    /// Creates a new adapter instance using the provided connection pool.
    pub fn new(pool: Arc<Pool>) -> Self {
        Self { pool }
    }

    /// Converts a single [`Param`] into a [`mysql::Value`].
    ///
    /// Mapping conventions:
    /// - `Bool(true)` → `Int(1)` / `Bool(false)` → `Int(0)`
    /// - `Str` → `Bytes`
    /// - `DateTime` → `Value::Date` (Y, M, D, H, M, S, μs)
    /// - `Null` → `NULL`
    #[inline]
    fn to_mysql_value(p: &Param) -> My {
        match p {
            Param::I64(x) => My::Int(*x),
            Param::U64(x) => My::UInt(*x),
            Param::Bool(b) => My::Int(if *b { 1 } else { 0 }),
            Param::Str(s) => My::Bytes(s.as_bytes().to_vec()),
            Param::DateTime(dt) => {
                let d = dt.date();
                let t = dt.time();
                My::Date(
                    d.year() as u16,
                    d.month() as u8,
                    d.day() as u8,
                    t.hour() as u8,
                    t.minute() as u8,
                    t.second() as u8,
                    t.nanosecond() / 1_000, // μs
                )
            }
            Param::Bin(b) => My::Bytes(b.to_vec()), // ← これを追加（UUIDなどBINARY(16)に対応）
            Param::Null => My::NULL,
        }
    }

    /// Converts a slice of [`Param`] into a positional [`Params`].
    #[inline]
    fn to_mysql_params(params_in: &[Param]) -> Params {
        let v: Vec<My> = params_in.iter().map(Self::to_mysql_value).collect();
        Params::Positional(v)
    }

    /// Converts a [`mysql::Row`] into a generic [`Row`].
    ///
    /// Unsupported types (e.g., decimals, time) are temporarily stringified.
    /// Extend [`Value`] as needed for stricter type support.
    fn row_from_mysql(mut r: mysql::Row) -> GRow {
        // 列名を先にコピー（borrow 競合回避）
        let names: Vec<String> = r
            .columns_ref()
            .iter()
            .map(|c| c.name_str().to_string())
            .collect();

        let mut out = GRow::default();
        for (idx, name) in names.into_iter().enumerate() {
            let v = r
                .take_opt::<My, _>(idx)
                .unwrap_or(Ok(My::NULL))
                .unwrap_or(My::NULL);

            let vv = match v {
                My::NULL => Value::Null,
                My::Int(i) => Value::I64(i),
                My::UInt(u) => Value::U64(u),

                // 先行して文字列化：必要なら Value に F64/Decimal を追加して厳密化
                My::Float(f) => Value::Str(f.to_string()),
                My::Double(f) => Value::Str(f.to_string()),

                // BLOB/TEXT 等
                My::Bytes(b) => match String::from_utf8(b) {
                    Ok(s) => Value::Str(s),
                    Err(e) => Value::Str(String::from_utf8_lossy(e.as_bytes()).into_owned()),
                },

                // DATE/DATETIME → NaiveDateTime へ
                My::Date(y, m, d, hh, mm, ss, _micro) => {
                    let date = NaiveDate::from_ymd_opt(y as i32, m as u32, d as u32)
                        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
                    let time = NaiveTime::from_hms_opt(hh as u32, mm as u32, ss as u32)
                        .unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap());
                    Value::DateTime(NaiveDateTime::new(date, time))
                }

                // TIME は（符号付き 日/時/分/秒.μ）→ とりあえず String 化
                My::Time(neg, days, hh, mm, ss, micro) => {
                    // 例: "-001 12:34:56.789012"
                    let sign = if neg { "-" } else { "" };
                    let s = if micro > 0 {
                        format!("{sign}{days:03} {hh:02}:{mm:02}:{ss:02}.{micro:06}")
                    } else {
                        format!("{sign}{days:03} {hh:02}:{mm:02}:{ss:02}")
                    };
                    Value::Str(s)
                }
            };

            out.insert(name, vv);
        }
        out
    }
}

impl Db for MySqlDb {
    fn fetch_one(&self, sql: &str, params_in: &[Param]) -> Result<Option<GRow>> {
        let params = Self::to_mysql_params(params_in);
        let mut conn = self.pool.get_conn().context("get_conn failed")?;

        dbglog!("-- exec_first about to run\nSQL: {sql}");
        for (i, p) in params_in.iter().enumerate() {
            dbglog!("param[{i}] = {:?}", p);
        }

        let res: std::result::Result<Option<mysql::Row>, MyError> = conn.exec_first(sql, params);
        if let Err(ref e) = res {
            eprintln!("exec_first failed: {}", mysql_err_summary(e));
            dbglog!("exec_first failed (debug): {e:?}");
            log_who_where(&mut conn);
        }
        let row_opt = res.context("exec_first failed")?;
        dbglog!("fetch_one: row_present={}", row_opt.is_some());

        Ok(row_opt.map(Self::row_from_mysql))
    }

    fn fetch_all(&self, sql: &str, params_in: &[Param]) -> Result<Vec<GRow>> {
        let params = Self::to_mysql_params(params_in);
        let mut conn = self.pool.get_conn().context("get_conn failed")?;

        dbglog!("-- exec(fetch_all) about to run\nSQL: {sql}");
        for (i, p) in params_in.iter().enumerate() {
            dbglog!("param[{i}] = {:?}", p);
        }

        let res: std::result::Result<Vec<mysql::Row>, MyError> = conn.exec(sql, params);
        if let Err(ref e) = res {
            eprintln!("exec (fetch_all) failed: {}", mysql_err_summary(e));
            dbglog!("exec (fetch_all) failed (debug): {e:?}");
            log_who_where(&mut conn);
        }
        let rows = res.context("exec (fetch_all) failed")?;
        dbglog!("fetch_all: rows={}", rows.len());

        Ok(rows.into_iter().map(Self::row_from_mysql).collect())
    }

    fn exec(&self, sql: &str, params_in: &[Param]) -> Result<u64> {
        let params = Self::to_mysql_params(params_in);
        let mut conn = self.pool.get_conn().context("get_conn failed")?;

        dbglog!("-- exec_drop about to run\nSQL: {sql}");
        for (i, p) in params_in.iter().enumerate() {
            dbglog!("param[{i}] = {:?}", p);
        }

        let res: std::result::Result<(), MyError> = conn.exec_drop(sql, params);
        if let Err(ref e) = res {
            eprintln!("exec_drop failed: {}", mysql_err_summary(e));
            dbglog!("exec_drop failed (debug): {e:?}");
            log_who_where(&mut conn);
        }
        res.context("exec_drop failed")?;

        let n = conn.affected_rows();
        dbglog!("affected_rows = {n}");
        Ok(n)
    }

    fn exec_returning_last_insert_id(&self, sql: &str, params_in: &[Param]) -> Result<u64> {
        let params = Self::to_mysql_params(params_in);
        let mut conn = self.pool.get_conn().context("get_conn failed")?;

        dbglog!("-- exec_drop about to run");
        dbglog!("SQL  : {sql}");
        for (i, p) in params_in.iter().enumerate() {
            dbglog!("param[{i}] = {:?}", p);
        }

        let res: std::result::Result<(), MyError> = conn.exec_drop(sql, params);
        if let Err(ref e) = res {
            eprintln!("exec_drop failed: {}", mysql_err_summary(e));
            dbglog!("exec_drop failed (debug): {e:?}");
            log_who_where(&mut conn);
        }
        res.context("exec_drop failed")?;

        let id: Option<u64> = conn
            .query_first("SELECT LAST_INSERT_ID()")
            .context("query_first(LAST_INSERT_ID()) failed")?;
        let id = id.ok_or_else(|| anyhow::anyhow!("LAST_INSERT_ID() returned NULL"))?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    /// Verifies primitive `Param` → `mysql::Value` conversions.
    #[test]
    fn to_mysql_value_maps_primitive_params() {
        // I64
        match MySqlDb::to_mysql_value(&Param::I64(-7)) {
            My::Int(v) => assert_eq!(v, -7),
            other => panic!("expected Int, got {other:?}"),
        }

        // U64
        match MySqlDb::to_mysql_value(&Param::U64(9)) {
            My::UInt(v) => assert_eq!(v, 9),
            other => panic!("expected UInt, got {other:?}"),
        }

        // Bool -> Int(0/1)
        match MySqlDb::to_mysql_value(&Param::Bool(true)) {
            My::Int(v) => assert_eq!(v, 1),
            other => panic!("expected Int(1), got {other:?}"),
        }
        match MySqlDb::to_mysql_value(&Param::Bool(false)) {
            My::Int(v) => assert_eq!(v, 0),
            other => panic!("expected Int(0), got {other:?}"),
        }

        // Str -> Bytes
        match MySqlDb::to_mysql_value(&Param::Str("abc")) {
            My::Bytes(b) => assert_eq!(b, b"abc"),
            other => panic!("expected Bytes(\"abc\"), got {other:?}"),
        }

        // Null -> NULL
        match MySqlDb::to_mysql_value(&Param::Null) {
            My::NULL => {}
            other => panic!("expected NULL, got {other:?}"),
        }
    }

    /// Checks DateTime → `My::Date` conversion.
    #[test]
    fn to_mysql_value_maps_datetime() {
        let dt = NaiveDate::from_ymd_opt(2025, 8, 28)
            .unwrap()
            .and_hms_micro_opt(15, 12, 34, 987_654)
            .unwrap();
        match MySqlDb::to_mysql_value(&Param::DateTime(dt)) {
            My::Date(y, m, d, hh, mm, ss, micro) => {
                assert_eq!(y, 2025);
                assert_eq!(m, 8);
                assert_eq!(d, 28);
                assert_eq!(hh, 15);
                assert_eq!(mm, 12);
                assert_eq!(ss, 34);
                assert_eq!(micro, 987_654);
            }
            other => panic!("expected Date, got {other:?}"),
        }
    }

    /// Ensures `to_mysql_params` preserves order and uses positional parameters.
    #[test]
    fn to_mysql_params_is_positional_and_ordered() {
        let dt = NaiveDate::from_ymd_opt(1970, 1, 2)
            .unwrap()
            .and_hms_opt(3, 4, 5)
            .unwrap();
        let ps = [
            Param::U64(1),
            Param::Str("x"),
            Param::Bool(true),
            Param::I64(-2),
            Param::DateTime(dt),
            Param::Null,
        ];

        let params = MySqlDb::to_mysql_params(&ps);
        match params {
            Params::Positional(v) => {
                assert_eq!(v.len(), 6);

                // index 0: U64
                matches!(v[0], My::UInt(1));

                // index 1: Str
                matches!(v[1], My::Bytes(_));

                // index 2: Bool -> Int(1)
                matches!(v[2], My::Int(1));

                // index 3: I64(-2)
                matches!(v[3], My::Int(-2));

                // index 4: DateTime
                if let My::Date(y, m, d, hh, mm, ss, micro) = v[4].clone() {
                    assert_eq!((y, m, d, hh, mm, ss, micro), (1970, 1, 2, 3, 4, 5, 0));
                } else {
                    panic!("index 4 must be My::Date");
                }

                // index 5: NULL
                matches!(v[5], My::NULL);
            }
            _ => panic!("expected Params::Positional"),
        }
    }
}
