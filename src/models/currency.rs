use anyhow::{Context, Result};
use reqwest;
use serde::Deserialize;
use std::time::Duration;

/// Stateless currency conversion manager with ECB API
pub struct CurrencyConverter {
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
    /// Create a new stateless currency converter
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("ccost/0.1.0")
            .build()
            .expect("Failed to create HTTP client");

        Self {
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
        // Fetch from ECB API (no caching)
        self.fetch_ecb_rate(from_currency, to_currency).await
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

    /// Get list of supported currencies (hardcoded list)
    pub async fn get_supported_currencies(&self) -> Result<Vec<String>> {
        let currencies = vec![
            "USD".to_string(),
            "EUR".to_string(),
            "GBP".to_string(),
            "JPY".to_string(),
            "CNY".to_string(),
            "AUD".to_string(),
            "CAD".to_string(),
            "CHF".to_string(),
            "SEK".to_string(),
            "NOK".to_string(),
            "DKK".to_string(),
            "PLN".to_string(),
            "CZK".to_string(),
            "HUF".to_string(),
            "RON".to_string(),
            "BGN".to_string(),
            "HRK".to_string(),
            "RUB".to_string(),
            "TRY".to_string(),
            "BRL".to_string(),
            "MXN".to_string(),
            "ZAR".to_string(),
            "INR".to_string(),
            "KRW".to_string(),
            "SGD".to_string(),
            "HKD".to_string(),
            "NZD".to_string(),
            "MYR".to_string(),
            "THB".to_string(),
            "PHP".to_string(),
        ];
        
        Ok(currencies)
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

    fn create_test_converter() -> CurrencyConverter {
        CurrencyConverter::new()
    }

    #[tokio::test]
    async fn test_usd_to_usd_conversion() {
        let converter = create_test_converter();
        let result = converter.convert_from_usd(100.0, "USD").await.unwrap();
        assert_eq!(result, 100.0);
    }

    #[tokio::test]
    async fn test_same_currency_conversion() {
        let converter = create_test_converter();
        let result = converter.convert(100.0, "EUR", "EUR").await.unwrap();
        assert_eq!(result, 100.0);
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
        let converter = create_test_converter();
        let currencies = converter.get_supported_currencies().await.unwrap();
        assert!(currencies.contains(&"EUR".to_string()));
        assert!(currencies.contains(&"USD".to_string()));
    }
}