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
}
