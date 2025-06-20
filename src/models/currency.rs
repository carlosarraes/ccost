use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Cache entry for a single currency conversion rate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyCacheEntry {
    /// Exchange rate from USD to this currency
    pub rate_from_usd: f64,
    /// Timestamp when this rate was fetched
    pub timestamp: DateTime<Utc>,
}

/// Currency conversion cache stored as JSON file
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CurrencyCache {
    /// Map of currency code to cached rate data
    pub rates: HashMap<String, CurrencyCacheEntry>,
}

/// Currency conversion manager with ECB API and 24-hour file-based caching
pub struct CurrencyConverter {
    client: reqwest::Client,
}

impl CurrencyConverter {
    /// Create a new stateless currency converter
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("ccost/0.1.1")
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Get path to currency cache file
    fn get_cache_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to determine home directory")?;
        Ok(home.join(".config").join("ccost").join("currency_cache.json"))
    }

    /// Load currency cache from file
    fn load_cache() -> CurrencyCache {
        match Self::get_cache_path() {
            Ok(cache_path) => {
                if cache_path.exists() {
                    match fs::read_to_string(&cache_path) {
                        Ok(contents) => {
                            match serde_json::from_str::<CurrencyCache>(&contents) {
                                Ok(cache) => cache,
                                Err(_) => CurrencyCache::default(), // Invalid cache, start fresh
                            }
                        }
                        Err(_) => CurrencyCache::default(), // Can't read file, start fresh
                    }
                } else {
                    CurrencyCache::default() // No cache file, start fresh
                }
            }
            Err(_) => CurrencyCache::default(), // Can't determine path, start fresh
        }
    }

    /// Save currency cache to file
    fn save_cache(cache: &CurrencyCache) -> Result<()> {
        let cache_path = Self::get_cache_path()?;
        
        // Ensure parent directory exists
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create cache directory: {}", parent.display())
            })?;
        }

        let contents = serde_json::to_string_pretty(cache)
            .context("Failed to serialize currency cache")?;

        fs::write(&cache_path, contents)
            .with_context(|| format!("Failed to write cache file: {}", cache_path.display()))?;

        Ok(())
    }

    /// Check if cache entry is still valid (less than 24 hours old)
    fn is_cache_valid(entry: &CurrencyCacheEntry) -> bool {
        let now = Utc::now();
        let age = now.signed_duration_since(entry.timestamp);
        age.num_hours() < 24
    }

    /// Convert amount from USD to target currency
    pub async fn convert_from_usd(&self, amount: f64, target_currency: &str) -> Result<f64> {
        if target_currency == "USD" {
            return Ok(amount);
        }

        let rate = self.get_exchange_rate("USD", target_currency).await?;
        Ok(amount * rate)
    }

    /// Get exchange rate between two currencies with caching
    async fn get_exchange_rate(&self, from_currency: &str, to_currency: &str) -> Result<f64> {
        // Only cache USD to other currency conversions for simplicity
        if from_currency != "USD" {
            return self.fetch_ecb_rate(from_currency, to_currency).await;
        }

        // Load cache
        let mut cache = Self::load_cache();

        // Check if we have a valid cached rate
        if let Some(entry) = cache.rates.get(to_currency) {
            if Self::is_cache_valid(entry) {
                return Ok(entry.rate_from_usd);
            }
        }

        // Cache miss or expired - fetch fresh rate
        let rate = self.fetch_ecb_rate(from_currency, to_currency).await?;

        // Update cache
        cache.rates.insert(to_currency.to_string(), CurrencyCacheEntry {
            rate_from_usd: rate,
            timestamp: Utc::now(),
        });

        // Save cache (ignore errors to not fail the conversion)
        let _ = Self::save_cache(&cache);

        Ok(rate)
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
        let pattern = format!(r#"currency='{currency}' rate='([0-9.]+)'"#);
        let re = regex::Regex::new(&pattern).context("Failed to create regex")?;

        if let Some(captures) = re.captures(&xml_text) {
            let rate_str = captures.get(1).unwrap().as_str();
            let rate: f64 = rate_str.parse().context("Failed to parse exchange rate")?;
            Ok(rate)
        } else {
            anyhow::bail!("Currency {currency} not found in ECB data")
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
        format!("{amount:.0}")
    } else {
        format!("{:.width$}", amount, width = decimal_places as usize)
    };

    match currency {
        "USD" | "GBP" => format!("{symbol}{formatted_amount}"),
        "EUR" | "JPY" | "CNY" => format!("{formatted_amount} {symbol}"),
        _ => format!("{formatted_amount} {currency}"),
    }
}
