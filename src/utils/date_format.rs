use anyhow::{Result, anyhow};
use chrono::{DateTime, NaiveDate, Utc};

/// Supported date format options for table output
#[derive(Debug, Clone, PartialEq)]
pub enum DateFormat {
    YearMonthDay, // yyyy-mm-dd (ISO)
    DayMonthYear, // dd-mm-yyyy (European)
    MonthDayYear, // mm-dd-yyyy (American)
}

impl DateFormat {
    /// Parse a date format string from config
    pub fn from_config_str(format_str: &str) -> Result<Self> {
        match format_str.to_lowercase().as_str() {
            "yyyy-mm-dd" => Ok(DateFormat::YearMonthDay),
            "dd-mm-yyyy" => Ok(DateFormat::DayMonthYear),
            "mm-dd-yyyy" => Ok(DateFormat::MonthDayYear),
            _ => Err(anyhow!(
                "Invalid date format '{}'. Supported formats: yyyy-mm-dd, dd-mm-yyyy, mm-dd-yyyy",
                format_str
            )),
        }
    }

    /// Get the chrono format string for this date format
    pub fn to_chrono_format(&self) -> &'static str {
        match self {
            DateFormat::YearMonthDay => "%Y-%m-%d",
            DateFormat::DayMonthYear => "%d-%m-%Y",
            DateFormat::MonthDayYear => "%m-%d-%Y",
        }
    }

    /// Get a user-friendly name for this format
    pub fn name(&self) -> &'static str {
        match self {
            DateFormat::YearMonthDay => "yyyy-mm-dd",
            DateFormat::DayMonthYear => "dd-mm-yyyy",
            DateFormat::MonthDayYear => "mm-dd-yyyy",
        }
    }

    /// Get an example of this format
    pub fn example(&self) -> &'static str {
        match self {
            DateFormat::YearMonthDay => "2024-03-15",
            DateFormat::DayMonthYear => "15-03-2024",
            DateFormat::MonthDayYear => "03-15-2024",
        }
    }

    /// Format a DateTime for table display
    pub fn format_datetime(&self, datetime: &DateTime<Utc>) -> String {
        datetime.format(self.to_chrono_format()).to_string()
    }

    /// Format a NaiveDate for table display
    pub fn format_naive_date(&self, date: &NaiveDate) -> String {
        date.format(self.to_chrono_format()).to_string()
    }

    /// Format a date with time for table display (date + time)
    pub fn format_datetime_with_time(&self, datetime: &DateTime<Utc>) -> String {
        match self {
            DateFormat::YearMonthDay => datetime.format("%Y-%m-%d %H:%M").to_string(),
            DateFormat::DayMonthYear => datetime.format("%d-%m-%Y %H:%M").to_string(),
            DateFormat::MonthDayYear => datetime.format("%m-%d-%Y %H:%M").to_string(),
        }
    }
}

/// Utility struct for formatting dates according to configuration
#[derive(Debug, Clone)]
pub struct DateFormatter {
    table_format: DateFormat,
}

impl DateFormatter {
    /// Create a new DateFormatter from config string
    pub fn new(config_format: &str) -> Result<Self> {
        let table_format = DateFormat::from_config_str(config_format)?;
        Ok(Self { table_format })
    }

    /// Create formatter with ISO format (for JSON output)
    pub fn iso() -> Self {
        Self {
            table_format: DateFormat::YearMonthDay,
        }
    }

    /// Format a DateTime for table output
    pub fn format_for_table(&self, datetime: &DateTime<Utc>) -> String {
        self.table_format.format_datetime(datetime)
    }

    /// Format a DateTime for JSON output (always ISO)
    pub fn format_for_json(&self, datetime: &DateTime<Utc>) -> String {
        // JSON output always uses ISO format regardless of config
        datetime.format("%Y-%m-%d").to_string()
    }

    /// Format a DateTime with time for table output
    pub fn format_for_table_with_time(&self, datetime: &DateTime<Utc>) -> String {
        self.table_format.format_datetime_with_time(datetime)
    }

    /// Format a DateTime with time for JSON output (always ISO)
    pub fn format_for_json_with_time(&self, datetime: &DateTime<Utc>) -> String {
        // JSON output always uses ISO format regardless of config
        datetime.format("%Y-%m-%d %H:%M").to_string()
    }

    /// Format a NaiveDate for table output
    pub fn format_naive_date_for_table(&self, date: &NaiveDate) -> String {
        self.table_format.format_naive_date(date)
    }

    /// Format a NaiveDate for JSON output (always ISO)
    pub fn format_naive_date_for_json(&self, date: &NaiveDate) -> String {
        // JSON output always uses ISO format regardless of config
        date.format("%Y-%m-%d").to_string()
    }

    /// Get the current table format
    pub fn table_format(&self) -> &DateFormat {
        &self.table_format
    }

    /// Validate a date format string
    pub fn validate_format(format_str: &str) -> Result<()> {
        DateFormat::from_config_str(format_str).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone};

    #[test]
    fn test_date_format_from_config() {
        assert_eq!(
            DateFormat::from_config_str("yyyy-mm-dd").unwrap(),
            DateFormat::YearMonthDay
        );
        assert_eq!(
            DateFormat::from_config_str("dd-mm-yyyy").unwrap(),
            DateFormat::DayMonthYear
        );
        assert_eq!(
            DateFormat::from_config_str("mm-dd-yyyy").unwrap(),
            DateFormat::MonthDayYear
        );

        // Case insensitive
        assert_eq!(
            DateFormat::from_config_str("YYYY-MM-DD").unwrap(),
            DateFormat::YearMonthDay
        );

        // Invalid format
        assert!(DateFormat::from_config_str("invalid").is_err());
    }

    #[test]
    fn test_date_format_chrono_patterns() {
        assert_eq!(DateFormat::YearMonthDay.to_chrono_format(), "%Y-%m-%d");
        assert_eq!(DateFormat::DayMonthYear.to_chrono_format(), "%d-%m-%Y");
        assert_eq!(DateFormat::MonthDayYear.to_chrono_format(), "%m-%d-%Y");
    }

    #[test]
    fn test_date_format_examples() {
        assert_eq!(DateFormat::YearMonthDay.example(), "2024-03-15");
        assert_eq!(DateFormat::DayMonthYear.example(), "15-03-2024");
        assert_eq!(DateFormat::MonthDayYear.example(), "03-15-2024");
    }

    #[test]
    fn test_datetime_formatting() {
        let dt = Utc.with_ymd_and_hms(2024, 3, 15, 14, 30, 0).unwrap();

        assert_eq!(DateFormat::YearMonthDay.format_datetime(&dt), "2024-03-15");
        assert_eq!(DateFormat::DayMonthYear.format_datetime(&dt), "15-03-2024");
        assert_eq!(DateFormat::MonthDayYear.format_datetime(&dt), "03-15-2024");
    }

    #[test]
    fn test_datetime_with_time_formatting() {
        let dt = Utc.with_ymd_and_hms(2024, 3, 15, 14, 30, 0).unwrap();

        assert_eq!(
            DateFormat::YearMonthDay.format_datetime_with_time(&dt),
            "2024-03-15 14:30"
        );
        assert_eq!(
            DateFormat::DayMonthYear.format_datetime_with_time(&dt),
            "15-03-2024 14:30"
        );
        assert_eq!(
            DateFormat::MonthDayYear.format_datetime_with_time(&dt),
            "03-15-2024 14:30"
        );
    }

    #[test]
    fn test_naive_date_formatting() {
        let date = NaiveDate::from_ymd_opt(2024, 3, 15).unwrap();

        assert_eq!(
            DateFormat::YearMonthDay.format_naive_date(&date),
            "2024-03-15"
        );
        assert_eq!(
            DateFormat::DayMonthYear.format_naive_date(&date),
            "15-03-2024"
        );
        assert_eq!(
            DateFormat::MonthDayYear.format_naive_date(&date),
            "03-15-2024"
        );
    }

    #[test]
    fn test_date_formatter_creation() {
        let formatter = DateFormatter::new("yyyy-mm-dd").unwrap();
        assert_eq!(formatter.table_format(), &DateFormat::YearMonthDay);

        let formatter = DateFormatter::new("dd-mm-yyyy").unwrap();
        assert_eq!(formatter.table_format(), &DateFormat::DayMonthYear);

        assert!(DateFormatter::new("invalid").is_err());
    }

    #[test]
    fn test_date_formatter_table_vs_json() {
        let dt = Utc.with_ymd_and_hms(2024, 3, 15, 14, 30, 0).unwrap();

        // European format for table
        let formatter = DateFormatter::new("dd-mm-yyyy").unwrap();
        assert_eq!(formatter.format_for_table(&dt), "15-03-2024");
        assert_eq!(formatter.format_for_json(&dt), "2024-03-15"); // Always ISO for JSON

        // American format for table
        let formatter = DateFormatter::new("mm-dd-yyyy").unwrap();
        assert_eq!(formatter.format_for_table(&dt), "03-15-2024");
        assert_eq!(formatter.format_for_json(&dt), "2024-03-15"); // Always ISO for JSON
    }

    #[test]
    fn test_date_formatter_with_time() {
        let dt = Utc.with_ymd_and_hms(2024, 3, 15, 14, 30, 0).unwrap();

        let formatter = DateFormatter::new("dd-mm-yyyy").unwrap();
        assert_eq!(
            formatter.format_for_table_with_time(&dt),
            "15-03-2024 14:30"
        );
        assert_eq!(formatter.format_for_json_with_time(&dt), "2024-03-15 14:30"); // Always ISO for JSON
    }

    #[test]
    fn test_date_formatter_iso() {
        let dt = Utc.with_ymd_and_hms(2024, 3, 15, 14, 30, 0).unwrap();
        let formatter = DateFormatter::iso();

        assert_eq!(formatter.format_for_table(&dt), "2024-03-15");
        assert_eq!(formatter.format_for_json(&dt), "2024-03-15");
    }

    #[test]
    fn test_validate_format() {
        assert!(DateFormatter::validate_format("yyyy-mm-dd").is_ok());
        assert!(DateFormatter::validate_format("dd-mm-yyyy").is_ok());
        assert!(DateFormatter::validate_format("mm-dd-yyyy").is_ok());
        assert!(DateFormatter::validate_format("invalid").is_err());
    }
}
