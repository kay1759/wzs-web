//! Local time utilities based on `chrono` and `chrono-tz`.
//!
//! This module provides generic helper functions for converting
//! the current UTC time into local time using IANA timezone names.
//!
//! # Provided Functions
//! - [`today_in_local`]: Returns the current date (`NaiveDate`) in the given timezone.
//! - [`now_in_local`]: Returns the current local time (`DateTime<Tz>`).
//!
//! # Timezone Format
//! - Timezone names must follow the **IANA format**, e.g. `"Asia/Tokyo"` or `"Australia/Melbourne"`.
//! - If an invalid name is given, the functions will return an error.

use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use std::str::FromStr;

/// # today_in_local
///
/// Returns **today's date** in the specified IANA timezone.
///
/// ## Arguments
/// - `tz_name`: A string such as `"Australia/Melbourne"` or `"Asia/Tokyo"`.
///
/// ## Returns
/// - `Ok(NaiveDate)` — Local date in the specified timezone.
/// - `Err` — If the timezone name is invalid.
///
/// ## Example
/// ```
/// use wzs_web::time::local::today_in_local;
/// let date = today_in_local("Asia/Tokyo").unwrap();
/// println!("Tokyo today = {}", date);
/// ```
pub fn today_in_local(tz_name: &str) -> Result<NaiveDate> {
    let tz: Tz =
        Tz::from_str(tz_name).map_err(|_| anyhow!("Invalid timezone name: {}", tz_name))?;

    let local_dt = Utc::now().with_timezone(&tz);
    Ok(local_dt.date_naive())
}

/// # now_in_local
///
/// Returns the **current local time** in the specified timezone.
///
/// ## Example
/// ```
/// use wzs_web::time::local::now_in_local;
/// let now_tokyo = now_in_local("Asia/Tokyo").unwrap();
/// println!("Tokyo now = {}", now_tokyo);
/// ```
pub fn now_in_local(tz_name: &str) -> Result<DateTime<Tz>> {
    let tz: Tz =
        Tz::from_str(tz_name).map_err(|_| anyhow!("Invalid timezone name: {}", tz_name))?;

    Ok(Utc::now().with_timezone(&tz))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone, Timelike, Utc};

    /// Ensures that timezone parsing works and date conversion is valid.
    #[test]
    fn test_today_in_local_valid_timezone() {
        let res = today_in_local("Asia/Tokyo");
        assert!(res.is_ok());
    }

    /// Tests conversion correctness using a fixed UTC time.
    /// We cannot mock Utc::now(), so instead we ensure the timezone conversion behavior is correct.
    #[test]
    fn test_timezone_conversion_logic() {
        let fixed = Utc.with_ymd_and_hms(2025, 2, 1, 0, 0, 0).unwrap();
        let tz: Tz = "Asia/Tokyo".parse().unwrap();

        let converted = fixed.with_timezone(&tz);

        assert_eq!(converted.hour(), 9); // JST is UTC+9
        assert_eq!(converted.year(), 2025);
    }

    /// Invalid timezone string should return an error.
    #[test]
    fn test_invalid_timezone_returns_error() {
        let result = today_in_local("Invalid/Timezone");
        assert!(result.is_err());
    }
}
