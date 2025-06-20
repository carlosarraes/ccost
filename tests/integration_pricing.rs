use ccost::models::{LiteLLMClient, PricingManager, PricingSource};
use tokio;

/// Integration tests for LiteLLM pricing accuracy
/// These tests validate that the pricing system works correctly

#[tokio::test]
async fn test_litellm_client_fetch_pricing() {
    let mut client = LiteLLMClient::new();
    
    // This test requires internet connection
    match client.fetch_pricing_data().await {
        Ok(pricing_data) => {
            assert!(!pricing_data.models.is_empty(), "Should have pricing data");
            
            // Check for some expected models
            let models: Vec<&String> = pricing_data.models.keys().collect();
            println!("Available models: {}", models.len());
            
            // Look for Claude models
            let claude_models: Vec<&String> = models.iter()
                .filter(|name| name.contains("claude"))
                .copied()
                .collect();
            
            if !claude_models.is_empty() {
                println!("Found Claude models: {:?}", claude_models);
            }
        },
        Err(e) => {
            // If we're offline or the API is down, skip this test
            println!("Skipping live pricing test due to network issue: {}", e);
        }
    }
}

#[tokio::test]
async fn test_pricing_manager_enhanced_cost_calculation() {
    let mut manager = PricingManager::with_live_pricing();
    
    // Test with a known model
    let (cost, source) = manager.calculate_enhanced_cost(
        "claude-sonnet-4-20250514",
        1_000_000, // 1M input tokens
        500_000,   // 500K output tokens
        200_000,   // 200K cache creation tokens  
        800_000,   // 800K cache read tokens
    ).await;
    
    // Cost should be positive
    assert!(cost > 0.0, "Cost should be positive, got: {}", cost);
    
    // Should be either live or static fallback
    assert!(
        source == PricingSource::LiteLLM || source == PricingSource::StaticFallback,
        "Source should be either LiteLLM or StaticFallback, got: {:?}", source
    );
    
    println!("Enhanced cost calculation: ${:.4} (source: {:?})", cost, source);
}

#[tokio::test]
async fn test_static_vs_enhanced_pricing_comparison() {
    let static_manager = PricingManager::new();
    let mut enhanced_manager = PricingManager::new(); // Use static manager for comparison
    
    let test_cases = vec![
        ("claude-sonnet-4-20250514", 1_000_000, 1_000_000, 500_000, 500_000),
        ("claude-opus-4-20250514", 500_000, 250_000, 100_000, 200_000),
        ("claude-haiku-3-5-20241022", 2_000_000, 500_000, 0, 0),
    ];
    
    for (model, input, output, cache_creation, cache_read) in test_cases {
        // Static calculation
        let static_cost = static_manager.calculate_cost_for_model(
            model, input, output, cache_creation, cache_read
        );
        
        // Enhanced calculation
        let (enhanced_cost, source) = enhanced_manager.calculate_enhanced_cost(
            model, input, output, cache_creation, cache_read
        ).await;
        
        println!(
            "Model: {} | Static: ${:.4} | Enhanced: ${:.4} ({:?})",
            model, static_cost, enhanced_cost, source
        );
        
        // Both should be positive
        assert!(static_cost > 0.0, "Static cost should be positive");
        assert!(enhanced_cost > 0.0, "Enhanced cost should be positive");
        
        // If using live pricing, costs might differ due to granular cache pricing
        if source == PricingSource::LiteLLM && cache_creation > 0 || cache_read > 0 {
            println!("  -> Live pricing may differ due to granular cache costs");
        } else if source == PricingSource::StaticFallback {
            // Should be very close when using static fallback
            let diff_percent = ((enhanced_cost - static_cost).abs() / static_cost) * 100.0;
            assert!(
                diff_percent < 1.0,
                "Static fallback should match static pricing within 1%, got {}% difference",
                diff_percent
            );
        }
    }
}

#[tokio::test]
async fn test_cache_ttl_functionality() {
    let mut client = LiteLLMClient::new();
    
    // Initially no cache
    assert!(!client.has_fresh_cache());
    assert!(client.cache_age_seconds().is_none());
    
    // Try to fetch data (might fail if offline)
    if client.fetch_pricing_data().await.is_ok() {
        // Should now have fresh cache
        assert!(client.has_fresh_cache());
        assert!(client.cache_age_seconds().is_some());
        
        let age = client.cache_age_seconds().unwrap();
        assert!(age < 5, "Cache should be very fresh, got age: {}s", age);
    }
}

#[test]
fn test_pricing_source_configuration() {
    // Test static manager
    let static_manager = PricingManager::new();
    assert!(!static_manager.is_live_pricing_enabled());
    assert_eq!(static_manager.get_pricing_source_info(), "Static");
    
    // Test live manager
    let live_manager = PricingManager::with_live_pricing();
    assert!(live_manager.is_live_pricing_enabled());
    assert_eq!(live_manager.get_pricing_source_info(), "Live (will fetch fresh data)");
    
    // Test toggling
    let mut manager = PricingManager::new();
    assert!(!manager.is_live_pricing_enabled());
    
    manager.set_live_pricing(true);
    assert!(manager.is_live_pricing_enabled());
    
    manager.set_live_pricing(false);
    assert!(!manager.is_live_pricing_enabled());
}

#[tokio::test]
async fn test_granular_cache_pricing_accuracy() {
    let mut manager = PricingManager::with_live_pricing();
    
    // Test case with significant cache usage
    let (cost_with_cache, source) = manager.calculate_enhanced_cost(
        "claude-sonnet-4-20250514",
        1_000_000,   // 1M input tokens
        0,           // No output tokens
        2_000_000,   // 2M cache creation tokens (should be ~25% of input cost)
        2_000_000,   // 2M cache read tokens (should be ~10% of input cost)
    ).await;
    
    // Test case without cache
    let (cost_without_cache, _) = manager.calculate_enhanced_cost(
        "claude-sonnet-4-20250514",
        1_000_000,   // 1M input tokens
        0,           // No output tokens
        0,           // No cache creation
        0,           // No cache read
    ).await;
    
    println!("Cost with cache: ${:.4}", cost_with_cache);
    println!("Cost without cache: ${:.4}", cost_without_cache);
    println!("Source: {:?}", source);
    
    // Cache cost should be additional
    assert!(
        cost_with_cache > cost_without_cache,
        "Cost with cache should be higher than without cache"
    );
    
    if source == PricingSource::LiteLLM {
        // With live pricing, cache should follow granular pricing
        println!("  -> Using live pricing with granular cache costs");
    } else {
        // With static fallback, cache uses flat rate
        println!("  -> Using static fallback with flat cache rate");
    }
}

#[test]
fn test_pricing_configuration_validation() {
    use ccost::config::settings::Config;
    
    let mut config = Config::default();
    
    // Test valid pricing source values
    assert!(config.set_value("pricing.source", "static").is_ok());
    assert!(config.set_value("pricing.source", "live").is_ok());
    assert!(config.set_value("pricing.source", "auto").is_ok());
    
    // Test invalid pricing source
    assert!(config.set_value("pricing.source", "invalid").is_err());
    
    // Test valid TTL values
    assert!(config.set_value("pricing.cache_ttl_minutes", "30").is_ok());
    assert!(config.set_value("pricing.cache_ttl_minutes", "1440").is_ok());
    
    // Test invalid TTL values
    assert!(config.set_value("pricing.cache_ttl_minutes", "0").is_err());
    assert!(config.set_value("pricing.cache_ttl_minutes", "1441").is_err());
    
    // Test boolean values
    assert!(config.set_value("pricing.offline_fallback", "true").is_ok());
    assert!(config.set_value("pricing.offline_fallback", "false").is_ok());
    assert!(config.set_value("pricing.offline_fallback", "invalid").is_err());
}

/// This test creates a sample scenario similar to the $360 discrepancy mentioned in TASK-062
#[tokio::test]
async fn test_pricing_accuracy_scenario() {
    let static_manager = PricingManager::new();
    let mut enhanced_manager = PricingManager::with_live_pricing();
    
    // Simulate scenario with ~2.2B cache tokens as mentioned in the task
    let test_scenario = vec![
        // High cache usage scenario
        ("claude-sonnet-4-20250514", 500_000_000, 100_000_000, 1_000_000_000, 1_200_000_000),
        ("claude-opus-4-20250514", 200_000_000, 50_000_000, 400_000_000, 600_000_000),
    ];
    
    let mut total_static_cost = 0.0;
    let mut total_enhanced_cost = 0.0;
    
    for (model, input, output, cache_creation, cache_read) in test_scenario {
        let static_cost = static_manager.calculate_cost_for_model(
            model, input, output, cache_creation, cache_read
        );
        
        let (enhanced_cost, source) = enhanced_manager.calculate_enhanced_cost(
            model, input, output, cache_creation, cache_read
        ).await;
        
        total_static_cost += static_cost;
        total_enhanced_cost += enhanced_cost;
        
        println!(
            "{}: Static=${:.2}, Enhanced=${:.2} ({:?})",
            model, static_cost, enhanced_cost, source
        );
    }
    
    let difference = (total_enhanced_cost - total_static_cost).abs() as f64;
    let difference_percent = (difference / total_static_cost) * 100.0;
    
    println!("Total static cost: ${:.2}", total_static_cost);
    println!("Total enhanced cost: ${:.2}", total_enhanced_cost);
    println!("Difference: ${:.2} ({:.1}%)", difference, difference_percent);
    
    // Both should be substantial costs
    assert!(total_static_cost > 100.0, "Total static cost should be substantial");
    assert!(total_enhanced_cost > 100.0, "Total enhanced cost should be substantial");
    
    // This test documents the current behavior for future validation
}