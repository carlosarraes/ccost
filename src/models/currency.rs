use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::storage::sqlite::Database;

/// Exchange rate data from ECB API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRate {
    pub base_currency: String,
    pub target_currency: String,
    pub rate: f64,
    pub fetched_at: DateTime<Utc>,
}

/// Currency conversion manager with ECB API and SQLite caching
pub struct CurrencyConverter {
    db: Database,
    cache_ttl_hours: u32,
    client: reqwest::Client,
}

/// ECB API response structure
#[derive(Debug, Deserialize)]
struct EcbResponse {
    #[serde(rename = "Cube")]
    cube: EcbCube,
}

#[derive(Debug, Deserialize)]
struct EcbCube {
    #[serde(rename = "Cube")]
    cube: Vec<EcbDateCube>,
}

#[derive(Debug, Deserialize)]
struct EcbDateCube {
    time: String,
    #[serde(rename = "Cube")]
    cube: Vec<EcbRateCube>,
}

#[derive(Debug, Deserialize)]
struct EcbRateCube {
    currency: String,
    rate: String,
}

impl CurrencyConverter {
    /// Create a new currency converter
    pub fn new(db: Database, cache_ttl_hours: u32) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("ccost/0.1.0")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            db,
            cache_ttl_hours,
            client,
        }
    }

    /// Convert amount from USD to target currency
    pub async fn convert_from_usd(&self, amount: f64, target_currency: &str) -> Result<f64> {
        if target_currency == "USD" {
            return Ok(amount);
        }

        let rate = self.get_exchange_rate("USD", target_currency).await?;
        Ok(amount * rate)
    }

    /// Convert amount between any two currencies
    pub async fn convert(&self, amount: f64, from_currency: &str, to_currency: &str) -> Result<f64> {
        if from_currency == to_currency {
            return Ok(amount);
        }

        let rate = self.get_exchange_rate(from_currency, to_currency).await?;
        Ok(amount * rate)
    }

    /// Get exchange rate between two currencies
    async fn get_exchange_rate(&self, from_currency: &str, to_currency: &str) -> Result<f64> {
        // Try to get from cache first
        if let Ok(cached_rate) = self.get_cached_rate(from_currency, to_currency).await {
            return Ok(cached_rate);
        }

        // Fetch from ECB API
        let rate = self.fetch_ecb_rate(from_currency, to_currency).await?;

        // Cache the result
        let exchange_rate = ExchangeRate {
            base_currency: from_currency.to_string(),
            target_currency: to_currency.to_string(),
            rate,
            fetched_at: Utc::now(),
        };

        if let Err(e) = self.cache_exchange_rate(&exchange_rate).await {
            eprintln!("Warning: Failed to cache exchange rate: {}", e);
        }

        Ok(rate)
    }

    /// Get cached exchange rate if still valid
    async fn get_cached_rate(&self, from_currency: &str, to_currency: &str) -> Result<f64> {
        if let Some((rate, fetched_at_str)) = self.db.get_exchange_rate(from_currency, to_currency)? {
            let fetched_at = DateTime::parse_from_rfc3339(&fetched_at_str)
                .context("Invalid datetime in cache")?
                .with_timezone(&Utc);

            let age_hours = Utc::now()
                .signed_duration_since(fetched_at)
                .num_hours() as u32;

            if age_hours < self.cache_ttl_hours {
                return Ok(rate);
            }
        }

        anyhow::bail!("No valid cached rate found")
    }

    /// Cache exchange rate in database
    pub async fn cache_exchange_rate(&self, rate: &ExchangeRate) -> Result<()> {
        self.db.save_exchange_rate(
            &rate.base_currency,
            &rate.target_currency,
            rate.rate,
            &rate.fetched_at.to_rfc3339(),
        )?;

        Ok(())
    }

    /// Fetch exchange rate from ECB API
    async fn fetch_ecb_rate(&self, from_currency: &str, to_currency: &str) -> Result<f64> {
        // ECB provides rates with EUR as base currency
        // For non-EUR conversions, we need to calculate through EUR
        let eur_to_target = if to_currency == "EUR" {
            1.0
        } else {
            self.fetch_eur_rate(to_currency).await?
        };

        let eur_to_base = if from_currency == "EUR" {
            1.0
        } else {
            self.fetch_eur_rate(from_currency).await?
        };

        // Convert from base to target via EUR
        // Rate = (1 / EUR_to_base) * EUR_to_target
        let rate = eur_to_target / eur_to_base;
        Ok(rate)
    }

    /// Fetch EUR to currency rate from ECB API
    async fn fetch_eur_rate(&self, currency: &str) -> Result<f64> {
        if currency == "EUR" {
            return Ok(1.0);
        }

        let url = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";
        let response = self.client
            .get(url)
            .send()
            .await
            .context("Failed to fetch ECB exchange rates")?;

        if !response.status().is_success() {
            anyhow::bail!("ECB API returned error: {}", response.status());
        }

        let xml_text = response.text().await?;
        
        // Parse the simple XML structure manually (ECB XML is predictable)
        // Look for currency='XXX' rate='Y.YY' pattern (ECB uses single quotes)
        let pattern = format!(r#"currency='{}' rate='([0-9.]+)'"#, currency);
        let re = regex::Regex::new(&pattern)
            .context("Failed to create regex")?;

        if let Some(captures) = re.captures(&xml_text) {
            let rate_str = captures.get(1).unwrap().as_str();
            let rate: f64 = rate_str.parse()
                .context("Failed to parse exchange rate")?;
            Ok(rate)
        } else {
            anyhow::bail!("Currency {} not found in ECB data", currency)
        }
    }

    /// Get list of supported currencies from cache
    pub async fn get_supported_currencies(&self) -> Result<Vec<String>> {
        let mut currencies = self.db.get_supported_currencies()?;
        
        // Always include EUR and USD as they're guaranteed to be supported
        if !currencies.contains(&"EUR".to_string()) {
            currencies.push("EUR".to_string());
        }
        if !currencies.contains(&"USD".to_string()) {
            currencies.push("USD".to_string());
        }
        
        currencies.sort();
        Ok(currencies)
    }

    /// Clear expired cache entries
    pub async fn cleanup_cache(&self) -> Result<usize> {
        let cutoff = Utc::now() 
            - chrono::Duration::hours(self.cache_ttl_hours as i64);
        
        let rows_affected = self.db.cleanup_exchange_rates(&cutoff.to_rfc3339())?;
        Ok(rows_affected)
    }
}

/// Format currency amount with appropriate symbol and decimals
pub fn format_currency(amount: f64, currency: &str, decimal_places: u8) -> String {
    let symbol = match currency {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "JPY" => "¥",
        "CNY" => "¥",
        _ => currency,
    };

    // Format with thousands separators
    let formatted_amount = if decimal_places == 0 {
        format!("{:.0}", amount)
    } else {
        format!("{:.width$}", amount, width = decimal_places as usize)
    };

    match currency {
        "USD" | "GBP" => format!("{}{}", symbol, formatted_amount),
        "EUR" | "JPY" | "CNY" => format!("{} {}", formatted_amount, symbol),
        _ => format!("{} {}", formatted_amount, currency),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::storage::sqlite::Database;

    async fn create_test_converter() -> (CurrencyConverter, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).unwrap();
        let converter = CurrencyConverter::new(db, 24);
        (converter, temp_dir)
    }

    #[tokio::test]
    async fn test_usd_to_usd_conversion() {
        let (converter, _temp_dir) = create_test_converter().await;
        let result = converter.convert_from_usd(100.0, "USD").await.unwrap();
        assert_eq!(result, 100.0);
    }

    #[tokio::test]
    async fn test_same_currency_conversion() {
        let (converter, _temp_dir) = create_test_converter().await;
        let result = converter.convert(100.0, "EUR", "EUR").await.unwrap();
        assert_eq!(result, 100.0);
    }

    #[tokio::test]
    async fn test_cache_storage_and_retrieval() {
        let (converter, _temp_dir) = create_test_converter().await;
        
        let rate = ExchangeRate {
            base_currency: "USD".to_string(),
            target_currency: "EUR".to_string(),
            rate: 0.85,
            fetched_at: Utc::now(),
        };

        converter.cache_exchange_rate(&rate).await.unwrap();
        
        let cached_rate = converter.get_cached_rate("USD", "EUR").await.unwrap();
        assert!((cached_rate - 0.85).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_expired_cache_rejection() {
        let (converter, _temp_dir) = create_test_converter().await;
        
        let old_rate = ExchangeRate {
            base_currency: "USD".to_string(),
            target_currency: "EUR".to_string(),
            rate: 0.85,
            fetched_at: Utc::now() - chrono::Duration::hours(25), // Older than 24h TTL
        };

        converter.cache_exchange_rate(&old_rate).await.unwrap();
        
        let result = converter.get_cached_rate("USD", "EUR").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cache_cleanup() {
        let (converter, _temp_dir) = create_test_converter().await;
        
        // Add old entry
        let old_rate = ExchangeRate {
            base_currency: "USD".to_string(),
            target_currency: "EUR".to_string(),
            rate: 0.85,
            fetched_at: Utc::now() - chrono::Duration::hours(25),
        };
        converter.cache_exchange_rate(&old_rate).await.unwrap();
        
        // Add fresh entry
        let fresh_rate = ExchangeRate {
            base_currency: "USD".to_string(),
            target_currency: "GBP".to_string(),
            rate: 0.80,
            fetched_at: Utc::now(),
        };
        converter.cache_exchange_rate(&fresh_rate).await.unwrap();
        
        let cleaned = converter.cleanup_cache().await.unwrap();
        assert_eq!(cleaned, 1); // Should remove only the old entry
    }

    #[test]
    fn test_currency_formatting_usd() {
        assert_eq!(format_currency(1234.56, "USD", 2), "$1234.56");
        assert_eq!(format_currency(1000.0, "USD", 0), "$1000");
    }

    #[test]
    fn test_currency_formatting_eur() {
        assert_eq!(format_currency(1234.56, "EUR", 2), "1234.56 €");
    }

    #[test]
    fn test_currency_formatting_gbp() {
        assert_eq!(format_currency(1234.56, "GBP", 2), "£1234.56");
    }

    #[test]
    fn test_currency_formatting_jpy() {
        assert_eq!(format_currency(1234.0, "JPY", 0), "1234 ¥");
    }

    #[test]
    fn test_currency_formatting_unknown() {
        assert_eq!(format_currency(1234.56, "XYZ", 2), "1234.56 XYZ");
    }

    // Mock test for API integration (would need actual API in integration tests)
    #[tokio::test]
    async fn test_supported_currencies_includes_defaults() {
        let (converter, _temp_dir) = create_test_converter().await;
        let currencies = converter.get_supported_currencies().await.unwrap();
        assert!(currencies.contains(&"EUR".to_string()));
        assert!(currencies.contains(&"USD".to_string()));
    }

    #[tokio::test]
    async fn test_ecb_xml_parsing_with_single_quotes() {
        let (converter, _temp_dir) = create_test_converter().await;
        
        // Mock XML data that matches ECB format with single quotes
        let xml_data = r#"
            <Cube currency='USD' rate='1.1429'/>
            <Cube currency='GBP' rate='0.8567'/>
            <Cube currency='JPY' rate='144.52'/>
        "#;
        
        // Test USD parsing
        let pattern = format!(r#"currency='{}' rate='([0-9.]+)'"#, "USD");
        let re = regex::Regex::new(&pattern).unwrap();
        
        let captures = re.captures(xml_data).unwrap();
        let rate_str = captures.get(1).unwrap().as_str();
        let rate: f64 = rate_str.parse().unwrap();
        
        assert!((rate - 1.1429).abs() < 0.0001);
    }
    
    #[tokio::test]
    async fn test_regex_fails_with_double_quotes() {
        // This test ensures we don't regress to double quotes
        let xml_data = r#"<Cube currency='USD' rate='1.1429'/>"#;
        
        // Test that old pattern (double quotes) fails
        let old_pattern = format!(r#"currency="{}" rate="([0-9.]+)""#, "USD");
        let old_re = regex::Regex::new(&old_pattern).unwrap();
        assert!(old_re.captures(xml_data).is_none());
        
        // Test that new pattern (single quotes) works
        let new_pattern = format!(r#"currency='{}' rate='([0-9.]+)'"#, "USD");
        let new_re = regex::Regex::new(&new_pattern).unwrap();
        assert!(new_re.captures(xml_data).is_some());
    }

    #[tokio::test]
    async fn test_usd_to_eur_conversion_realistic() {
        let (converter, _temp_dir) = create_test_converter().await;
        
        // Cache a realistic USD to EUR rate (EUR base: 1 EUR = 1.1429 USD, so USD to EUR = 1/1.1429 ≈ 0.875)
        let rate = ExchangeRate {
            base_currency: "USD".to_string(),
            target_currency: "EUR".to_string(),
            rate: 0.875,
            fetched_at: Utc::now(),
        };
        
        converter.cache_exchange_rate(&rate).await.unwrap();
        
        // Test conversion: $100 USD should be approximately €87.50
        let result = converter.convert_from_usd(100.0, "EUR").await.unwrap();
        assert!((result - 87.5).abs() < 0.1);
    }
}