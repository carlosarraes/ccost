use anyhow::{Context, Result};
use reqwest;
use std::time::Duration;

/// Stateless currency conversion manager with ECB API
pub struct CurrencyConverter {
    client: reqwest::Client,
}

impl CurrencyConverter {
    /// Create a new stateless currency converter
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("ccost/0.1.0")
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Convert amount from USD to target currency
    pub async fn convert_from_usd(&self, amount: f64, target_currency: &str) -> Result<f64> {
        if target_currency == "USD" {
            return Ok(amount);
        }

        let rate = self.get_exchange_rate("USD", target_currency).await?;
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
        let response = self
            .client
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
        let re = regex::Regex::new(&pattern).context("Failed to create regex")?;

        if let Some(captures) = re.captures(&xml_text) {
            let rate_str = captures.get(1).unwrap().as_str();
            let rate: f64 = rate_str.parse().context("Failed to parse exchange rate")?;
            Ok(rate)
        } else {
            anyhow::bail!("Currency {} not found in ECB data", currency)
        }
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
