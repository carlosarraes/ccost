// Analysis module
pub mod usage;
pub mod projects;
pub mod timeline;
pub mod timezone;
pub mod optimization;
pub mod conversations;

// Re-export key types for easier access
pub use usage::{UsageTracker, UsageFilter, CostCalculationMode};
pub use projects::{ProjectAnalyzer, ProjectSortBy};
pub use timezone::TimezoneCalculator;
pub use optimization::OptimizationEngine;
pub use conversations::{ConversationAnalyzer, ConversationInsight, ConversationInsightList, ConversationFilter, ConversationSortBy};

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::parser::jsonl::{UsageData, Usage, Message};


    #[test]
    fn test_cost_calculation_mode_integration() {
        // Test Auto mode with mixed embedded/missing costs
        let tracker_auto = UsageTracker::new(CostCalculationMode::Auto);
        
        let messages = vec![
            // Message with embedded cost
            UsageData {
                timestamp: Some("2025-06-09T10:00:00Z".to_string()),
                uuid: Some("uuid1".to_string()),
                request_id: Some("req1".to_string()),
                message: Some(Message {
                    model: Some("claude-sonnet-4".to_string()),
                    ..Default::default()
                }),
                usage: Some(Usage {
                    input_tokens: Some(100),
                    output_tokens: Some(50),
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                }),
                cost_usd: Some(0.75), // Has embedded cost
                cwd: None,
                original_cwd: None,
            },
            // Message without embedded cost
            UsageData {
                timestamp: Some("2025-06-09T10:01:00Z".to_string()),
                uuid: Some("uuid2".to_string()),
                request_id: Some("req2".to_string()),
                message: Some(Message {
                    model: Some("claude-sonnet-4".to_string()),
                    ..Default::default()
                }),
                usage: Some(Usage {
                    input_tokens: Some(200),
                    output_tokens: Some(100),
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                }),
                cost_usd: None, // No embedded cost - will calculate (currently 0.0)
                cwd: None,
                original_cwd: None,
            },
        ];
        
        let usage = tracker_auto.process_usage_data(messages, "auto_test").unwrap();
        
        // Should use embedded cost for first message, calculated cost for second
        assert_eq!(usage.total_cost_usd, 0.75); // 0.75 + 0.0 (calculated)
        assert_eq!(usage.total_input_tokens, 300);
        assert_eq!(usage.total_output_tokens, 150);
    }
}