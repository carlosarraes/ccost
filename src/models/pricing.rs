use crate::models::litellm::{EnhancedModelPricing, LiteLLMClient, PricingSource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub input_cost_per_mtok: f64,  // Cost per million tokens for input
    pub output_cost_per_mtok: f64, // Cost per million tokens for output
    pub cache_cost_per_mtok: f64,  // Cost per million tokens for cache operations
}

impl ModelPricing {
    pub fn new(input_cost: f64, output_cost: f64, cache_cost: f64) -> Self {
        Self {
            input_cost_per_mtok: input_cost,
            output_cost_per_mtok: output_cost,
            cache_cost_per_mtok: cache_cost,
        }
    }

    /// Calculate cost for token usage in USD
    pub fn calculate_cost(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        cache_creation_tokens: u64,
        cache_read_tokens: u64,
    ) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * self.input_cost_per_mtok;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * self.output_cost_per_mtok;
        let cache_creation_cost =
            (cache_creation_tokens as f64 / 1_000_000.0) * self.cache_cost_per_mtok;
        let cache_read_cost = (cache_read_tokens as f64 / 1_000_000.0) * self.cache_cost_per_mtok;

        input_cost + output_cost + cache_creation_cost + cache_read_cost
    }
}

#[derive(Debug)]
pub struct PricingManager {
    pricing_data: HashMap<String, ModelPricing>,
    litellm_client: Option<LiteLLMClient>,
    enable_live_pricing: bool,
}

impl PricingManager {
    /// Create new pricing manager with default pricing data
    pub fn new() -> Self {
        let mut pricing_data = HashMap::new();

        // Load default pricing from embedded data
        pricing_data.insert(
            "claude-sonnet-4-20250514".to_string(),
            ModelPricing::new(3.0, 15.0, 0.3),
        );
        pricing_data.insert(
            "claude-opus-4-20250514".to_string(),
            ModelPricing::new(15.0, 75.0, 1.5),
        );
        pricing_data.insert(
            "claude-haiku-3-5-20241022".to_string(),
            ModelPricing::new(1.0, 5.0, 0.1),
        );

        Self {
            pricing_data,
            litellm_client: None,
            enable_live_pricing: false,
        }
    }

    /// Create new pricing manager with LiteLLM integration enabled
    pub fn with_live_pricing() -> Self {
        let mut manager = Self::new();
        manager.litellm_client = Some(LiteLLMClient::new());
        manager.enable_live_pricing = true;
        manager
    }

    /// Pre-fetch and cache pricing data to avoid delays during calculations
    /// Only fetches if explicitly using live pricing mode
    pub async fn initialize_live_pricing(&mut self) -> Result<(), anyhow::Error> {
        // Only fetch if live pricing is explicitly enabled
        // Default behavior should be fast offline mode
        if self.enable_live_pricing {
            if let Some(ref mut client) = self.litellm_client {
                // Try to fetch with short timeout to avoid delays
                match tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    client.fetch_pricing_data(),
                )
                .await
                {
                    Ok(Ok(_)) => {
                        // Successfully fetched live data
                    }
                    Ok(Err(e)) => {
                        eprintln!("Warning: Failed to fetch live pricing, using static: {}", e);
                        self.enable_live_pricing = false; // Fall back to static
                    }
                    Err(_) => {
                        eprintln!("Warning: Live pricing fetch timed out, using static pricing");
                        self.enable_live_pricing = false; // Fall back to static
                    }
                }
            }
        }
        Ok(())
    }

    /// Enable or disable live pricing
    pub fn set_live_pricing(&mut self, enabled: bool) {
        self.enable_live_pricing = enabled;
        if enabled && self.litellm_client.is_none() {
            self.litellm_client = Some(LiteLLMClient::new());
        }
    }

    /// Get pricing for a specific model
    pub fn get_pricing(&self, model_name: &str) -> Option<ModelPricing> {
        self.pricing_data.get(model_name).cloned()
    }

    /// Get pricing with fallback to default if model not found
    pub fn get_pricing_with_fallback(&self, model_name: &str) -> ModelPricing {
        self.get_pricing(model_name).unwrap_or_else(|| {
            // Fallback pricing for unknown models (Claude 3.5 Sonnet pricing)
            ModelPricing::new(3.0, 15.0, 0.3)
        })
    }

    /// Calculate cost for usage data
    pub fn calculate_cost_for_model(
        &self,
        model_name: &str,
        input_tokens: u64,
        output_tokens: u64,
        cache_creation_tokens: u64,
        cache_read_tokens: u64,
    ) -> f64 {
        let pricing = self.get_pricing_with_fallback(model_name);
        pricing.calculate_cost(
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
        )
    }

    /// Get enhanced pricing with live LiteLLM data if available
    pub async fn get_enhanced_pricing(&mut self, model_name: &str) -> EnhancedModelPricing {
        if self.enable_live_pricing {
            if let Some(ref mut client) = self.litellm_client {
                return client.get_pricing_with_fallback(model_name).await;
            }
        }

        // Fallback to static pricing
        let static_pricing = self.get_pricing_with_fallback(model_name);
        EnhancedModelPricing::new(
            static_pricing.input_cost_per_mtok,
            static_pricing.output_cost_per_mtok,
            static_pricing.cache_cost_per_mtok, // Use same rate for both creation and read
            static_pricing.cache_cost_per_mtok,
            PricingSource::StaticFallback,
        )
    }

    /// Calculate cost using enhanced pricing model
    pub async fn calculate_enhanced_cost(
        &mut self,
        model_name: &str,
        input_tokens: u64,
        output_tokens: u64,
        cache_creation_tokens: u64,
        cache_read_tokens: u64,
    ) -> (f64, PricingSource) {
        let pricing = self.get_enhanced_pricing(model_name).await;
        let cost = pricing.calculate_cost(
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
        );
        (cost, pricing.source)
    }

    /// Check if live pricing is enabled and available
    pub fn is_live_pricing_enabled(&self) -> bool {
        self.enable_live_pricing && self.litellm_client.is_some()
    }

    /// Get pricing source information
    pub fn get_pricing_source_info(&self) -> String {
        if self.is_live_pricing_enabled() {
            if let Some(ref client) = self.litellm_client {
                if client.has_fresh_cache() {
                    format!(
                        "Live (cached {}s ago)",
                        client.cache_age_seconds().unwrap_or(0)
                    )
                } else {
                    "Live (will fetch fresh data)".to_string()
                }
            } else {
                "Static fallback".to_string()
            }
        } else {
            "Static".to_string()
        }
    }
}

impl Default for PricingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_pricing_cost_calculation() {
        let pricing = ModelPricing::new(3.0, 15.0, 0.3);

        // Test with 1M tokens each
        let cost = pricing.calculate_cost(1_000_000, 1_000_000, 1_000_000, 1_000_000);
        let expected = 3.0 + 15.0 + 0.3 + 0.3; // 18.6
        assert!(
            (cost - expected).abs() < 0.001,
            "Expected {}, got {}",
            expected,
            cost
        );
    }

    #[test]
    fn test_model_pricing_small_amounts() {
        let pricing = ModelPricing::new(3.0, 15.0, 0.3);

        // Test with 1000 tokens each (0.001 MTok)
        let cost = pricing.calculate_cost(1000, 1000, 1000, 1000);
        let expected = 0.003 + 0.015 + 0.0003 + 0.0003; // 0.0186
        assert!(
            (cost - expected).abs() < 0.0001,
            "Expected {}, got {}",
            expected,
            cost
        );
    }

    #[test]
    fn test_pricing_manager_default_models() {
        let manager = PricingManager::new();

        // Test that all default models are available
        assert!(manager.get_pricing("claude-sonnet-4-20250514").is_some());
        assert!(manager.get_pricing("claude-opus-4-20250514").is_some());
        assert!(manager.get_pricing("claude-haiku-3-5-20241022").is_some());
    }

    #[test]
    fn test_pricing_manager_fallback() {
        let manager = PricingManager::new();

        // Test fallback for unknown model
        let pricing = manager.get_pricing_with_fallback("unknown-model");
        assert!((pricing.input_cost_per_mtok - 3.0).abs() < 0.001);
        assert!((pricing.output_cost_per_mtok - 15.0).abs() < 0.001);
        assert!((pricing.cache_cost_per_mtok - 0.3).abs() < 0.001);
    }

    // TODO: Implement from_bundled_data method before enabling this test
    // #[test]
    // fn test_pricing_manager_from_bundled_data() {
    //     let manager = PricingManager::from_bundled_data().expect("Should load bundled data");

    //     // Test that bundled data loads correctly
    //     let sonnet_pricing = manager
    //         .get_pricing("claude-sonnet-4-20250514")
    //         .expect("Should have Sonnet pricing");
    //     assert!((sonnet_pricing.input_cost_per_mtok - 3.0).abs() < 0.001);
    //     assert!((sonnet_pricing.output_cost_per_mtok - 15.0).abs() < 0.001);
    // }

    #[test]
    fn test_pricing_manager_calculate_cost_for_model() {
        let manager = PricingManager::new();

        // Test cost calculation for known model
        let cost = manager.calculate_cost_for_model(
            "claude-sonnet-4-20250514",
            1_000_000,
            1_000_000,
            0,
            0,
        );
        let expected = 3.0 + 15.0; // 18.0
        assert!(
            (cost - expected).abs() < 0.001,
            "Expected {}, got {}",
            expected,
            cost
        );
    }

    // TODO: Implement list_models method before enabling this test
    // #[test]
    // fn test_pricing_manager_list_models() {
    //     let manager = PricingManager::new();
    //     let models = manager.list_models().expect("Should list models");

    //     assert!(models.len() >= 3);
    //     assert!(models.contains(&"claude-sonnet-4-20250514".to_string()));
    //     assert!(models.contains(&"claude-opus-4-20250514".to_string()));
    //     assert!(models.contains(&"claude-haiku-3-5-20241022".to_string()));
    // }

    // TODO: Implement set_pricing and delete_pricing methods before enabling these tests
    // #[test]
    // fn test_pricing_manager_set_pricing() {
    //     let mut manager = PricingManager::new();
    //     let custom_pricing = ModelPricing::new(5.0, 25.0, 0.5);

    //     manager
    //         .set_pricing("custom-model".to_string(), custom_pricing.clone())
    //         .expect("Should set pricing");

    //     let retrieved = manager
    //         .get_pricing("custom-model")
    //         .expect("Should have custom pricing");
    //     assert!((retrieved.input_cost_per_mtok - 5.0).abs() < 0.001);
    //     assert!((retrieved.output_cost_per_mtok - 25.0).abs() < 0.001);
    //     assert!((retrieved.cache_cost_per_mtok - 0.5).abs() < 0.001);
    // }

    #[test]
    fn test_unknown_model_fallback_calculation() {
        let manager = PricingManager::new();

        // Test that unknown models get fallback pricing
        let cost =
            manager.calculate_cost_for_model("some-unknown-model", 1_000_000, 1_000_000, 0, 0);
        let expected = 3.0 + 15.0; // Should use fallback pricing (Sonnet rates)
        assert!(
            (cost - expected).abs() < 0.001,
            "Expected {}, got {}",
            expected,
            cost
        );
    }

    #[test]
    fn test_pricing_manager_with_live_pricing() {
        let manager = PricingManager::with_live_pricing();
        assert!(manager.is_live_pricing_enabled());
        assert_eq!(
            manager.get_pricing_source_info(),
            "Live (will fetch fresh data)"
        );
    }

    #[test]
    fn test_pricing_manager_set_live_pricing() {
        let mut manager = PricingManager::new();
        assert!(!manager.is_live_pricing_enabled());

        manager.set_live_pricing(true);
        assert!(manager.is_live_pricing_enabled());

        manager.set_live_pricing(false);
        assert!(!manager.is_live_pricing_enabled());
    }

    #[tokio::test]
    async fn test_enhanced_pricing_fallback() {
        let mut manager = PricingManager::new();

        // Without live pricing, should use static fallback
        let pricing = manager
            .get_enhanced_pricing("claude-sonnet-4-20250514")
            .await;
        assert_eq!(pricing.source, PricingSource::StaticFallback);
        assert_eq!(pricing.input_cost_per_mtok, 3.0);
        assert_eq!(pricing.output_cost_per_mtok, 15.0);
    }

    #[tokio::test]
    async fn test_calculate_enhanced_cost() {
        let mut manager = PricingManager::new();

        let (cost, source) = manager
            .calculate_enhanced_cost("claude-sonnet-4-20250514", 1_000_000, 1_000_000, 0, 0)
            .await;

        assert_eq!(source, PricingSource::StaticFallback);
        let expected = 3.0 + 15.0; // 18.0
        assert!(
            (cost - expected).abs() < 0.001,
            "Expected {}, got {}",
            expected,
            cost
        );
    }

    // TODO: Implement set_pricing and delete_pricing methods before enabling this test
    // #[test]
    // fn test_pricing_manager_delete_pricing() {
    //     let mut manager = PricingManager::new();
    //     let custom_pricing = ModelPricing::new(5.0, 25.0, 0.5);

    //     // Set pricing
    //     manager
    //         .set_pricing("delete-test".to_string(), custom_pricing)
    //         .expect("Should set pricing");
    //     assert!(manager.get_pricing("delete-test").is_some());

    //     // Delete pricing
    //     let deleted = manager
    //         .delete_pricing("delete-test")
    //         .expect("Should delete pricing");
    //     assert!(deleted);
    //     assert!(manager.get_pricing("delete-test").is_none());
    // }
}
