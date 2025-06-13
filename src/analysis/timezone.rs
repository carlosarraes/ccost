use anyhow::Result;
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use std::str::FromStr;

/// Timezone-aware date calculation utilities
pub struct TimezoneCalculator {
    timezone: Tz,
    daily_cutoff_hour: u8,
}

impl TimezoneCalculator {
    /// Create a new timezone calculator
    pub fn new(timezone_str: &str, daily_cutoff_hour: u8) -> Result<Self> {
        let timezone = Tz::from_str(timezone_str)
            .map_err(|e| anyhow::anyhow!("Invalid timezone '{}': {}", timezone_str, e))?;

        if daily_cutoff_hour > 23 {
            anyhow::bail!("Daily cutoff hour must be 0-23, got: {}", daily_cutoff_hour);
        }

        Ok(Self {
            timezone,
            daily_cutoff_hour,
        })
    }

    /// Create calculator with UTC timezone (default)
    pub fn utc() -> Self {
        Self {
            timezone: Tz::UTC,
            daily_cutoff_hour: 0,
        }
    }

    /// Get the start of today in the configured timezone
    pub fn today_start(&self) -> DateTime<Utc> {
        let now = Utc::now().with_timezone(&self.timezone);
        let today = now.date_naive();

        // Create start of day with cutoff hour
        let start_time = today
            .and_hms_opt(self.daily_cutoff_hour.into(), 0, 0)
            .expect("Invalid daily cutoff hour");

        self.timezone
            .from_local_datetime(&start_time)
            .single()
            .expect("Unambiguous local time")
            .with_timezone(&Utc)
    }

    /// Get the end of today in the configured timezone
    pub fn today_end(&self) -> DateTime<Utc> {
        let now = Utc::now().with_timezone(&self.timezone);
        let today = now.date_naive();

        // Create end of day (just before midnight + cutoff hour)
        let next_day = today + chrono::Duration::days(1);
        let end_time = next_day
            .and_hms_opt(self.daily_cutoff_hour.into(), 0, 0)
            .expect("Invalid daily cutoff hour")
            - chrono::Duration::seconds(1);

        self.timezone
            .from_local_datetime(&end_time)
            .single()
            .expect("Unambiguous local time")
            .with_timezone(&Utc)
    }

    /// Get the start of yesterday in the configured timezone
    pub fn yesterday_start(&self) -> DateTime<Utc> {
        let now = Utc::now().with_timezone(&self.timezone);
        let yesterday = now.date_naive() - chrono::Duration::days(1);

        let start_time = yesterday
            .and_hms_opt(self.daily_cutoff_hour.into(), 0, 0)
            .expect("Invalid daily cutoff hour");

        self.timezone
            .from_local_datetime(&start_time)
            .single()
            .expect("Unambiguous local time")
            .with_timezone(&Utc)
    }

    /// Get the end of yesterday in the configured timezone
    pub fn yesterday_end(&self) -> DateTime<Utc> {
        let now = Utc::now().with_timezone(&self.timezone);
        let yesterday = now.date_naive() - chrono::Duration::days(1);

        let today = yesterday + chrono::Duration::days(1);
        let end_time = today
            .and_hms_opt(self.daily_cutoff_hour.into(), 0, 0)
            .expect("Invalid daily cutoff hour")
            - chrono::Duration::seconds(1);

        self.timezone
            .from_local_datetime(&end_time)
            .single()
            .expect("Unambiguous local time")
            .with_timezone(&Utc)
    }

    /// Get the start of this week (Monday) in the configured timezone
    pub fn this_week_start(&self) -> DateTime<Utc> {
        let now = Utc::now().with_timezone(&self.timezone);
        let today = now.date_naive();
        let days_since_monday = today.weekday().num_days_from_monday();
        let monday = today - chrono::Duration::days(days_since_monday as i64);

        let start_time = monday
            .and_hms_opt(self.daily_cutoff_hour.into(), 0, 0)
            .expect("Invalid daily cutoff hour");

        self.timezone
            .from_local_datetime(&start_time)
            .single()
            .expect("Unambiguous local time")
            .with_timezone(&Utc)
    }

    /// Get the start of this month in the configured timezone
    pub fn this_month_start(&self) -> DateTime<Utc> {
        let now = Utc::now().with_timezone(&self.timezone);
        let today = now.date_naive();
        let first_of_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
            .expect("Valid first day of month");

        let start_time = first_of_month
            .and_hms_opt(self.daily_cutoff_hour.into(), 0, 0)
            .expect("Invalid daily cutoff hour");

        self.timezone
            .from_local_datetime(&start_time)
            .single()
            .expect("Unambiguous local time")
            .with_timezone(&Utc)
    }

    /// Convert a UTC datetime to the configured timezone for display
    pub fn to_local(&self, utc_time: DateTime<Utc>) -> DateTime<Tz> {
        utc_time.with_timezone(&self.timezone)
    }

    /// Get the timezone name
    pub fn timezone_name(&self) -> &str {
        self.timezone.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveTime, Timelike, Weekday};

    #[test]
    fn test_utc_timezone_calculator() {
        let calc = TimezoneCalculator::utc();
        assert_eq!(calc.timezone_name(), "UTC");
        assert_eq!(calc.daily_cutoff_hour, 0);
    }

    #[test]
    fn test_timezone_creation() {
        // Test valid timezones
        assert!(TimezoneCalculator::new("UTC", 0).is_ok());
        assert!(TimezoneCalculator::new("America/New_York", 6).is_ok());
        assert!(TimezoneCalculator::new("Europe/London", 12).is_ok());

        // Test invalid timezone
        assert!(TimezoneCalculator::new("Invalid/Timezone", 0).is_err());

        // Test invalid cutoff hour
        assert!(TimezoneCalculator::new("UTC", 24).is_err());
    }

    #[test]
    fn test_today_calculations_utc() {
        let calc = TimezoneCalculator::utc();
        let today_start = calc.today_start();
        let today_end = calc.today_end();

        // Today start should be earlier than today end
        assert!(today_start < today_end);

        // Both should be within today (UTC)
        let now = Utc::now();
        let today_date = now.date_naive();
        assert_eq!(today_start.date_naive(), today_date);

        // Start should be at midnight (00:00:00)
        let start_time = today_start.time();
        assert_eq!(start_time.hour(), 0);
        assert_eq!(start_time.minute(), 0);
        assert_eq!(start_time.second(), 0);
    }

    #[test]
    fn test_yesterday_calculations_utc() {
        let calc = TimezoneCalculator::utc();
        let yesterday_start = calc.yesterday_start();
        let yesterday_end = calc.yesterday_end();

        // Yesterday start should be earlier than yesterday end
        assert!(yesterday_start < yesterday_end);

        // Yesterday should be before today
        let today_start = calc.today_start();
        assert!(yesterday_end < today_start);

        // Yesterday should be exactly one day before today
        let expected_yesterday = Utc::now().date_naive() - chrono::Duration::days(1);
        assert_eq!(yesterday_start.date_naive(), expected_yesterday);
    }

    #[test]
    fn test_week_calculations_utc() {
        let calc = TimezoneCalculator::utc();
        let week_start = calc.this_week_start();

        // Week start should be a Monday
        assert_eq!(week_start.weekday(), Weekday::Mon);

        // Week start should be at midnight
        let start_time = week_start.time();
        assert_eq!(start_time.hour(), 0);
        assert_eq!(start_time.minute(), 0);
        assert_eq!(start_time.second(), 0);
    }

    #[test]
    fn test_month_calculations_utc() {
        let calc = TimezoneCalculator::utc();
        let month_start = calc.this_month_start();

        // Month start should be the 1st day of the current month
        assert_eq!(month_start.day(), 1);

        // Month start should be at midnight
        let start_time = month_start.time();
        assert_eq!(start_time.hour(), 0);
        assert_eq!(start_time.minute(), 0);
        assert_eq!(start_time.second(), 0);

        // Should be current month and year
        let now = Utc::now();
        assert_eq!(month_start.month(), now.month());
        assert_eq!(month_start.year(), now.year());
    }

    #[test]
    fn test_daily_cutoff_hour() {
        let calc = TimezoneCalculator::new("UTC", 6).unwrap();
        let today_start = calc.today_start();

        // Start should be at 6 AM
        assert_eq!(today_start.time().hour(), 6);
        assert_eq!(today_start.time().minute(), 0);
        assert_eq!(today_start.time().second(), 0);
    }

    #[test]
    fn test_timezone_conversion() {
        let calc = TimezoneCalculator::new("America/New_York", 0).unwrap();
        let utc_time = Utc::now();
        let local_time = calc.to_local(utc_time);

        // Converted time should be the same instant
        assert_eq!(utc_time.timestamp(), local_time.timestamp());

        // But should be in different timezone
        assert_eq!(local_time.timezone().name(), "America/New_York");
    }
}
