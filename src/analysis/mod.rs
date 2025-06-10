// Analysis module
pub mod usage;
pub mod projects;
pub mod timeline;
pub mod timezone;

// Re-export key types for easier access
pub use usage::{UsageTracker, ProjectUsage, ModelUsage, UsageFilter, CostCalculationMode};
pub use projects::{ProjectAnalyzer, ProjectSummary, ProjectSortBy, ProjectStatistics};
pub use timezone::TimezoneCalculator;

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::parser::jsonl::{UsageData, Usage, Message};
    use tempfile::TempDir;
    use crate::storage::Database;

    #[test]
    fn test_end_to_end_usage_tracking_workflow() {
        // Create test database
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).unwrap();
        
        // Create usage tracker with database
        let tracker = UsageTracker::with_database(CostCalculationMode::Auto, database);
        
        // Create realistic usage data simulating a Claude conversation
        let messages = vec![
            UsageData {
                timestamp: Some("2025-06-09T10:00:00Z".to_string()),
                uuid: Some("conv-123".to_string()),
                request_id: Some("req-1".to_string()),
                message: Some(Message {
                    model: Some("claude-sonnet-4".to_string()),
                    role: Some("user".to_string()),
                    content: Some("Hello, can you help me?".to_string()),
                    usage: None,
                }),
                usage: Some(Usage {
                    input_tokens: Some(50),
                    output_tokens: Some(100),
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                }),
                cost_usd: Some(0.45),
            },
            UsageData {
                timestamp: Some("2025-06-09T10:01:00Z".to_string()),
                uuid: Some("conv-123".to_string()),
                request_id: Some("req-2".to_string()),
                message: Some(Message {
                    model: Some("claude-opus-4".to_string()),
                    role: Some("assistant".to_string()),
                    content: Some("Of course! I'd be happy to help.".to_string()),
                    usage: None,
                }),
                usage: Some(Usage {
                    input_tokens: Some(25),
                    output_tokens: Some(75),
                    cache_creation_input_tokens: Some(10),
                    cache_read_input_tokens: Some(5),
                }),
                cost_usd: Some(1.20),
            },
            UsageData {
                timestamp: Some("2025-06-09T10:02:00Z".to_string()),
                uuid: Some("conv-123".to_string()),
                request_id: Some("req-3".to_string()),
                message: Some(Message {
                    model: Some("claude-sonnet-4".to_string()),
                    role: Some("user".to_string()),
                    content: Some("Thanks! Can you explain recursion?".to_string()),
                    usage: None,
                }),
                usage: Some(Usage {
                    input_tokens: Some(75),
                    output_tokens: Some(200),
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: Some(15),
                }),
                cost_usd: Some(0.82),
            },
        ];
        
        // Process usage data
        let project_usage = tracker.process_usage_data(messages, "test_project").unwrap();
        
        // Verify overall project totals
        assert_eq!(project_usage.project_name, "test_project");
        assert_eq!(project_usage.total_input_tokens, 150);    // 50 + 25 + 75
        assert_eq!(project_usage.total_output_tokens, 375);   // 100 + 75 + 200
        assert_eq!(project_usage.total_cache_creation_tokens, 10);
        assert_eq!(project_usage.total_cache_read_tokens, 20); // 0 + 5 + 15
        assert!((project_usage.total_cost_usd - 2.47).abs() < 0.01); // 0.45 + 1.20 + 0.82 (floating point precision)
        assert_eq!(project_usage.message_count, 3);
        
        // Verify model-specific breakdown
        assert_eq!(project_usage.model_usage.len(), 2);
        
        // Check Claude Sonnet usage
        let sonnet_usage = project_usage.model_usage.get("claude-sonnet-4").unwrap();
        assert_eq!(sonnet_usage.input_tokens, 125);           // 50 + 75
        assert_eq!(sonnet_usage.output_tokens, 300);          // 100 + 200
        assert_eq!(sonnet_usage.cache_creation_tokens, 0);
        assert_eq!(sonnet_usage.cache_read_tokens, 15);       // 0 + 15
        assert_eq!(sonnet_usage.cost_usd, 1.27);              // 0.45 + 0.82
        assert_eq!(sonnet_usage.message_count, 2);
        
        // Check Claude Opus usage
        let opus_usage = project_usage.model_usage.get("claude-opus-4").unwrap();
        assert_eq!(opus_usage.input_tokens, 25);
        assert_eq!(opus_usage.output_tokens, 75);
        assert_eq!(opus_usage.cache_creation_tokens, 10);
        assert_eq!(opus_usage.cache_read_tokens, 5);
        assert_eq!(opus_usage.cost_usd, 1.20);
        assert_eq!(opus_usage.message_count, 1);
        
        // Verify model switching tracking works correctly
        assert_eq!(
            project_usage.total_cost_usd,
            sonnet_usage.cost_usd + opus_usage.cost_usd
        );
    }

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
            },
        ];
        
        let usage = tracker_auto.process_usage_data(messages, "auto_test").unwrap();
        
        // Should use embedded cost for first message, calculated cost for second
        assert_eq!(usage.total_cost_usd, 0.75); // 0.75 + 0.0 (calculated)
        assert_eq!(usage.total_input_tokens, 300);
        assert_eq!(usage.total_output_tokens, 150);
    }
}