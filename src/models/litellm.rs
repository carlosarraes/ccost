use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::fs;
use std::path::PathBuf;
use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use reqwest::Client;

const LITELLM_PRICING_URL: &str = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";
const CACHE_TTL_SECONDS: u64 = 3600; // 1 hour
const PERSISTENT_CACHE_TTL_HOURS: i64 = 24; // 24 hours for file cache

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiteLLMModelData {
    #[serde(rename = "input_cost_per_token")]
    pub input_cost_per_token: Option<f64>,
    #[serde(rename = "output_cost_per_token")]
    pub output_cost_per_token: Option<f64>,
    #[serde(rename = "cache_creation_input_token_cost")]
    pub cache_creation_input_token_cost: Option<f64>,
    #[serde(rename = "cache_read_input_token_cost")]
    pub cache_read_input_token_cost: Option<f64>,
    #[serde(rename = "max_tokens")]
    pub max_tokens: Option<u32>,
    #[serde(rename = "max_input_tokens")]
    pub max_input_tokens: Option<u32>,
    #[serde(rename = "max_output_tokens")]
    pub max_output_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiteLLMPricingData {
    pub models: HashMap<String, LiteLLMModelData>,
}

#[derive(Debug, Clone)]
pub struct EnhancedModelPricing {
    pub input_cost_per_mtok: f64,
    pub output_cost_per_mtok: f64,
    pub cache_creation_cost_per_mtok: f64,
    pub cache_read_cost_per_mtok: f64,
    pub source: PricingSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PricingSource {
    LiteLLM,
    StaticFallback,
}

impl EnhancedModelPricing {
    pub fn new(
        input_cost: f64,
        output_cost: f64,
        cache_creation_cost: f64,
        cache_read_cost: f64,
        source: PricingSource,
    ) -> Self {
        Self {
            input_cost_per_mtok: input_cost,
            output_cost_per_mtok: output_cost,
            cache_creation_cost_per_mtok: cache_creation_cost,
            cache_read_cost_per_mtok: cache_read_cost,
            source,
        }
    }

    /// Calculate cost for token usage in USD with granular cache pricing
    pub fn calculate_cost(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        cache_creation_tokens: u64,
        cache_read_tokens: u64,
    ) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * self.input_cost_per_mtok;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * self.output_cost_per_mtok;
        let cache_creation_cost = (cache_creation_tokens as f64 / 1_000_000.0) * self.cache_creation_cost_per_mtok;
        let cache_read_cost = (cache_read_tokens as f64 / 1_000_000.0) * self.cache_read_cost_per_mtok;

        input_cost + output_cost + cache_creation_cost + cache_read_cost
    }
}

#[derive(Debug)]
pub struct CacheEntry {
    data: LiteLLMPricingData,
    timestamp: Instant,
}

impl CacheEntry {
    pub fn new(data: LiteLLMPricingData) -> Self {
        Self {
            data,
            timestamp: Instant::now(),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.timestamp.elapsed() > Duration::from_secs(CACHE_TTL_SECONDS)
    }
}

/// Persistent cache entry for LiteLLM pricing data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentCacheEntry {
    /// LiteLLM pricing data
    pub data: LiteLLMPricingData,
    /// Timestamp when this data was fetched
    pub timestamp: DateTime<Utc>,
}

impl PersistentCacheEntry {
    pub fn new(data: LiteLLMPricingData) -> Self {
        Self {
            data,
            timestamp: Utc::now(),
        }
    }

    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        let age = now.signed_duration_since(self.timestamp);
        age.num_hours() >= PERSISTENT_CACHE_TTL_HOURS
    }
}

#[derive(Debug)]
pub struct LiteLLMClient {
    client: Client,
    cache: Option<CacheEntry>,
}

impl LiteLLMClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            cache: None,
        }
    }

    /// Get path to persistent LiteLLM cache file
    fn get_persistent_cache_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to determine home directory")?;
        Ok(home.join(".config").join("ccost").join("litellm_cache.json"))
    }

    /// Load persistent cache from file
    fn load_persistent_cache() -> Option<PersistentCacheEntry> {
        match Self::get_persistent_cache_path() {
            Ok(cache_path) => {
                if cache_path.exists() {
                    match fs::read_to_string(&cache_path) {
                        Ok(contents) => {
                            match serde_json::from_str::<PersistentCacheEntry>(&contents) {
                                Ok(cache) => {
                                    if !cache.is_expired() {
                                        Some(cache)
                                    } else {
                                        None // Expired cache
                                    }
                                }
                                Err(_) => None, // Invalid cache
                            }
                        }
                        Err(_) => None, // Can't read file
                    }
                } else {
                    None // No cache file
                }
            }
            Err(_) => None, // Can't determine path
        }
    }

    /// Save persistent cache to file
    fn save_persistent_cache(data: &LiteLLMPricingData) -> Result<()> {
        let cache_path = Self::get_persistent_cache_path()?;
        
        // Ensure parent directory exists
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create cache directory: {}", parent.display())
            })?;
        }

        let cache_entry = PersistentCacheEntry::new(data.clone());
        let contents = serde_json::to_string_pretty(&cache_entry)
            .context("Failed to serialize LiteLLM cache")?;

        fs::write(&cache_path, contents)
            .with_context(|| format!("Failed to write cache file: {}", cache_path.display()))?;

        Ok(())
    }

    /// Fetch pricing data from LiteLLM repository with persistent caching
    pub async fn fetch_pricing_data(&mut self) -> Result<LiteLLMPricingData> {
        // Check in-memory cache first (fastest)
        if let Some(ref cache) = self.cache {
            if !cache.is_expired() {
                return Ok(cache.data.clone());
            }
        }

        // Check persistent cache (fast, avoids network)
        if let Some(persistent_cache) = Self::load_persistent_cache() {
            // Load into in-memory cache for subsequent calls
            self.cache = Some(CacheEntry::new(persistent_cache.data.clone()));
            return Ok(persistent_cache.data);
        }

        // Fetch fresh data
        let response = self
            .client
            .get(LITELLM_PRICING_URL)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .context("Failed to fetch LiteLLM pricing data")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "HTTP error fetching pricing data: {}",
                response.status()
            ));
        }

        // Get response text first for better error handling
        let response_text = response
            .text()
            .await
            .context("Failed to read LiteLLM response text")?;

        // Parse the raw JSON and filter out non-model entries
        let raw_json: serde_json::Value = serde_json::from_str(&response_text)
            .with_context(|| {
                let preview = if response_text.len() > 200 {
                    &response_text[..200]
                } else {
                    &response_text
                };
                format!("Failed to parse LiteLLM raw JSON. Preview: {}", preview)
            })?;

        // Filter out special fields and only keep actual model entries
        let mut models = HashMap::new();
        if let serde_json::Value::Object(obj) = raw_json {
            for (key, value) in obj {
                // Skip special fields that aren't model data
                if key == "sample_spec" || key.starts_with("_") {
                    continue;
                }
                
                // Try to parse as model data
                if let Ok(model_data) = serde_json::from_value::<LiteLLMModelData>(value) {
                    models.insert(key, model_data);
                }
                // Skip entries that don't match our model data structure
            }
        }

        let pricing_data = LiteLLMPricingData { models };

        // Cache the data in memory
        self.cache = Some(CacheEntry::new(pricing_data.clone()));

        // Save to persistent cache (ignore errors to not fail the fetch)
        let _ = Self::save_persistent_cache(&pricing_data);

        Ok(pricing_data)
    }

    /// Get pricing for a specific model from LiteLLM data
    pub async fn get_model_pricing(&mut self, model_name: &str) -> Result<Option<EnhancedModelPricing>> {
        let pricing_data = self.fetch_pricing_data().await?;

        if let Some(model_data) = pricing_data.models.get(model_name) {
            // Convert per-token costs to per-million-token costs
            let input_cost = model_data.input_cost_per_token.unwrap_or(0.0) * 1_000_000.0;
            let output_cost = model_data.output_cost_per_token.unwrap_or(0.0) * 1_000_000.0;
            
            // Handle cache pricing with fallback logic
            let cache_creation_cost = model_data
                .cache_creation_input_token_cost
                .map(|cost| cost * 1_000_000.0)
                .unwrap_or_else(|| {
                    // Fallback: 25% of input cost for cache creation
                    input_cost * 0.25
                });

            let cache_read_cost = model_data
                .cache_read_input_token_cost
                .map(|cost| cost * 1_000_000.0)
                .unwrap_or_else(|| {
                    // Fallback: 10% of input cost for cache read
                    input_cost * 0.10
                });

            Ok(Some(EnhancedModelPricing::new(
                input_cost,
                output_cost,
                cache_creation_cost,
                cache_read_cost,
                PricingSource::LiteLLM,
            )))
        } else {
            Ok(None)
        }
    }

    /// Get enhanced pricing with static fallback
    pub async fn get_pricing_with_fallback(&mut self, model_name: &str) -> EnhancedModelPricing {
        match self.get_model_pricing(model_name).await {
            Ok(Some(pricing)) => pricing,
            _ => {
                // Static fallback pricing (Claude 3.5 Sonnet rates)
                EnhancedModelPricing::new(
                    3.0,   // input_cost_per_mtok
                    15.0,  // output_cost_per_mtok
                    0.75,  // cache_creation_cost_per_mtok (25% of input)
                    0.30,  // cache_read_cost_per_mtok (10% of input)
                    PricingSource::StaticFallback,
                )
            }
        }
    }

    /// Check if cache is available and fresh
    pub fn has_fresh_cache(&self) -> bool {
        self.cache.as_ref().map_or(false, |cache| !cache.is_expired())
    }

    /// Get cache age in seconds
    pub fn cache_age_seconds(&self) -> Option<u64> {
        self.cache.as_ref().map(|cache| cache.timestamp.elapsed().as_secs())
    }
}

impl Default for LiteLLMClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_model_pricing_creation() {
        let pricing = EnhancedModelPricing::new(
            3.0, 15.0, 0.75, 0.30, PricingSource::LiteLLM
        );
        
        assert_eq!(pricing.input_cost_per_mtok, 3.0);
        assert_eq!(pricing.output_cost_per_mtok, 15.0);
        assert_eq!(pricing.cache_creation_cost_per_mtok, 0.75);
        assert_eq!(pricing.cache_read_cost_per_mtok, 0.30);
        assert_eq!(pricing.source, PricingSource::LiteLLM);
    }

    #[test]
    fn test_enhanced_pricing_cost_calculation() {
        let pricing = EnhancedModelPricing::new(
            3.0, 15.0, 0.75, 0.30, PricingSource::LiteLLM
        );

        // Test with 1M tokens each
        let cost = pricing.calculate_cost(1_000_000, 1_000_000, 1_000_000, 1_000_000);
        let expected = 3.0 + 15.0 + 0.75 + 0.30; // 19.05
        assert!(
            (cost - expected).abs() < 0.001,
            "Expected {}, got {}",
            expected,
            cost
        );
    }

    #[test]
    fn test_cache_entry_expiration() {
        let data = LiteLLMPricingData {
            models: HashMap::new(),
        };
        let cache = CacheEntry::new(data);
        
        // Fresh cache should not be expired
        assert!(!cache.is_expired());
    }

    #[test]
    fn test_litellm_client_creation() {
        let client = LiteLLMClient::new();
        assert!(client.cache.is_none());
        assert!(!client.has_fresh_cache());
        assert!(client.cache_age_seconds().is_none());
    }

    #[test]
    fn test_pricing_source_equality() {
        assert_eq!(PricingSource::LiteLLM, PricingSource::LiteLLM);
        assert_eq!(PricingSource::StaticFallback, PricingSource::StaticFallback);
        assert_ne!(PricingSource::LiteLLM, PricingSource::StaticFallback);
    }
}