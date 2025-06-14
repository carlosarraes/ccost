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


    /// Format a DateTime with time for table output
    pub fn format_for_table_with_time(&self, datetime: &DateTime<Utc>) -> String {
        self.table_format.format_datetime_with_time(datetime)
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





}
