use chrono::NaiveDate;

/// A port that provides the **current date** for the application.
///
/// # Purpose
/// This trait abstracts access to "today" so that:
///
/// - Application and domain logic do **not** depend on system time
/// - Implementations can be swapped (system clock, fixed clock, mock, etc.)
/// - Tests can be deterministic and time-independent
///
/// # Design Notes
/// - The timezone concept is intentionally delegated to the implementation.
/// - This trait represents an **external capability**, similar to a Repository or Mailer.
///
/// # Typical Implementations
/// - `SystemClock`: Uses the OS / runtime clock with a configured timezone
/// - `FixedClock`: Returns a constant date (for testing)
pub trait Clock: Send + Sync {
    /// Returns today's date as a [`NaiveDate`].
    ///
    /// Implementations decide how "today" is determined
    /// (e.g. system time, fixed value, mocked time source).
    fn today(&self) -> NaiveDate;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    /// Test implementation of `Clock` that always returns a fixed date.
    struct FixedClock {
        date: NaiveDate,
    }

    impl FixedClock {
        fn new(date: NaiveDate) -> Self {
            Self { date }
        }
    }

    impl Clock for FixedClock {
        fn today(&self) -> NaiveDate {
            self.date
        }
    }

    #[test]
    fn fixed_clock_returns_given_date() {
        let date = NaiveDate::from_ymd_opt(2025, 10, 2).unwrap();
        let clock = FixedClock::new(date);

        assert_eq!(clock.today(), date);
    }

    #[test]
    fn clock_trait_object_works() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let clock: Box<dyn Clock> = Box::new(FixedClock::new(date));

        assert_eq!(clock.today(), date);
    }
}
