use chrono::NaiveDate;

use crate::time::clock::Clock;
use crate::time::local::today_in_local;

/// A [`Clock`] implementation backed by the system clock.
///
/// # Overview
/// `SystemClock` provides the current date based on the operating system's
/// current time and a configured IANA timezone.
///
/// Internally, it delegates timezone handling and date conversion to
/// [`today_in_local`].
///
/// # Design Notes
/// - The timezone is fixed at construction time.
/// - Any invalid timezone should be considered a **configuration error**.
/// - Therefore, this implementation is allowed to panic if the timezone
///   is invalid.
///
/// # Responsibility
/// - Selecting the timezone is the responsibility of the **composition root**
///   (e.g. `main.rs`).
/// - Application and domain logic should treat `Clock` as a trusted source.
pub struct SystemClock {
    tz_name: String,
}

impl SystemClock {
    /// Creates a new [`SystemClock`] with the given IANA timezone name.
    ///
    /// # Arguments
    /// - `tz_name`: An IANA timezone name such as `"Asia/Tokyo"`
    ///   or `"Australia/Melbourne"`.
    ///
    /// # Panics
    /// This constructor itself does not panic, but [`Clock::today`] will panic
    /// if the provided timezone name is invalid.
    pub fn new(tz_name: impl Into<String>) -> Self {
        Self {
            tz_name: tz_name.into(),
        }
    }
}

impl Clock for SystemClock {
    /// Returns today's date in the configured timezone.
    ///
    /// # Panics
    /// Panics if the timezone name is invalid.
    /// This is intentional, as an invalid timezone represents a
    /// misconfiguration rather than a recoverable runtime error.
    fn today(&self) -> NaiveDate {
        today_in_local(&self.tz_name).expect("Invalid timezone for SystemClock")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn system_clock_returns_a_date_for_valid_timezone() {
        let clock = SystemClock::new("Asia/Tokyo");

        let today = clock.today();

        // Basic sanity checks:
        // - Year must be reasonable
        // - Month and day must be valid ranges
        assert!(today.year() >= 2000);
        assert!((1..=12).contains(&today.month()));
        assert!((1..=31).contains(&today.day()));
    }

    #[test]
    #[should_panic(expected = "Invalid timezone for SystemClock")]
    fn system_clock_panics_for_invalid_timezone() {
        let clock = SystemClock::new("Invalid/Timezone");

        // This should panic due to invalid timezone configuration
        let _ = clock.today();
    }
}
