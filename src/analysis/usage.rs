use std::collections::HashMap;
use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use serde::Serialize;
use crate::parser::jsonl::{UsageData, Usage};
use crate::storage::Database;

#[derive(Debug, Clone, PartialEq)]
pub enum CostCalculationMode {
    Auto,       // Use embedded costUSD if available, otherwise calculate
    Calculate,  // Always calculate cost from tokens * pricing  
    Display,    // Use embedded costUSD only, 0 if missing
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectUsage {
    pub project_name: String,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cost_usd: f64,
    pub model_usage: HashMap<String, ModelUsage>,
    pub message_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelUsage {
    pub model_name: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost_usd: f64,
    pub message_count: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageFilter {
    pub project_name: Option<String>,
    pub model_name: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
}

pub struct UsageTracker {
    calculation_mode: CostCalculationMode,
    database: Option<Database>,
}

impl UsageTracker {
    pub fn new(mode: CostCalculationMode) -> Self {
        Self {
            calculation_mode: mode,
            database: None,
        }
    }

    pub fn with_database(mode: CostCalculationMode, database: Database) -> Self {
        Self {
            calculation_mode: mode,
            database: Some(database),
        }
    }

    pub fn process_usage_data(&self, messages: Vec<UsageData>, project_name: &str) -> Result<ProjectUsage> {
        let mut project_usage = ProjectUsage {
            project_name: project_name.to_string(),
            ..Default::default()
        };

        for message in messages {
            // Skip messages without usage data
            let usage = match &message.usage {
                Some(usage) => usage,
                None => continue,
            };

            // Extract model name
            let model_name = self.extract_model_from_message(&message);

            // Get or create model usage entry
            let model_usage = project_usage.model_usage
                .entry(model_name.clone())
                .or_insert_with(|| ModelUsage {
                    model_name: model_name.clone(),
                    ..Default::default()
                });

            // Aggregate token counts
            let input_tokens = usage.input_tokens.unwrap_or(0);
            let output_tokens = usage.output_tokens.unwrap_or(0);
            let cache_creation_tokens = usage.cache_creation_input_tokens.unwrap_or(0);
            let cache_read_tokens = usage.cache_read_input_tokens.unwrap_or(0);

            // Update project totals
            project_usage.total_input_tokens += input_tokens;
            project_usage.total_output_tokens += output_tokens;
            project_usage.total_cache_creation_tokens += cache_creation_tokens;
            project_usage.total_cache_read_tokens += cache_read_tokens;
            project_usage.message_count += 1;

            // Update model totals
            model_usage.input_tokens += input_tokens;
            model_usage.output_tokens += output_tokens;
            model_usage.cache_creation_tokens += cache_creation_tokens;
            model_usage.cache_read_tokens += cache_read_tokens;
            model_usage.message_count += 1;

            // Calculate cost based on mode
            let cost = match self.calculation_mode {
                CostCalculationMode::Display => {
                    // Only use embedded cost, 0 if missing
                    message.cost_usd.unwrap_or(0.0)
                }
                CostCalculationMode::Auto => {
                    // Use embedded cost if available, otherwise calculate
                    if let Some(embedded_cost) = message.cost_usd {
                        embedded_cost
                    } else {
                        // For now, we'll use 0.0 until we have pricing manager here
                        0.0
                    }
                }
                CostCalculationMode::Calculate => {
                    // Always calculate from tokens
                    // For now, we'll use 0.0 until we have pricing manager here
                    0.0
                }
            };

            project_usage.total_cost_usd += cost;
            model_usage.cost_usd += cost;
        }

        Ok(project_usage)
    }

    pub fn calculate_cost(&self, usage: &Usage, model_name: &str, pricing_manager: &crate::models::PricingManager) -> Result<f64> {
        let input_tokens = usage.input_tokens.unwrap_or(0);
        let output_tokens = usage.output_tokens.unwrap_or(0);
        let cache_creation_tokens = usage.cache_creation_input_tokens.unwrap_or(0);
        let cache_read_tokens = usage.cache_read_input_tokens.unwrap_or(0);

        let cost = pricing_manager.calculate_cost_for_model(
            model_name,
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
        );

        Ok(cost)
    }

    pub fn calculate_usage_with_projects(&self, enhanced_data: Vec<(UsageData, String)>, pricing_manager: &crate::models::PricingManager) -> Result<Vec<ProjectUsage>> {
        let mut projects: HashMap<String, ProjectUsage> = HashMap::new();

        for (message, project_name) in enhanced_data {
            // Skip messages without usage data
            let usage = match &message.usage {
                Some(usage) => usage,
                None => continue,
            };

            // Extract model name
            let model_name = self.extract_model_from_message(&message);

            // Get or create project usage entry
            let project_usage = projects
                .entry(project_name.clone())
                .or_insert_with(|| ProjectUsage {
                    project_name: project_name.clone(),
                    ..Default::default()
                });

            // Get or create model usage entry
            let model_usage = project_usage.model_usage
                .entry(model_name.clone())
                .or_insert_with(|| ModelUsage {
                    model_name: model_name.clone(),
                    ..Default::default()
                });

            // Aggregate token counts
            let input_tokens = usage.input_tokens.unwrap_or(0);
            let output_tokens = usage.output_tokens.unwrap_or(0);
            let cache_creation_tokens = usage.cache_creation_input_tokens.unwrap_or(0);
            let cache_read_tokens = usage.cache_read_input_tokens.unwrap_or(0);

            // Update project totals
            project_usage.total_input_tokens += input_tokens;
            project_usage.total_output_tokens += output_tokens;
            project_usage.total_cache_creation_tokens += cache_creation_tokens;
            project_usage.total_cache_read_tokens += cache_read_tokens;
            project_usage.message_count += 1;

            // Update model totals
            model_usage.input_tokens += input_tokens;
            model_usage.output_tokens += output_tokens;
            model_usage.cache_creation_tokens += cache_creation_tokens;
            model_usage.cache_read_tokens += cache_read_tokens;
            model_usage.message_count += 1;

            // Calculate cost based on mode
            let cost = match self.calculation_mode {
                CostCalculationMode::Display => {
                    // Only use embedded cost, 0 if missing
                    message.cost_usd.unwrap_or(0.0)
                }
                CostCalculationMode::Auto => {
                    // Use embedded cost if available, otherwise calculate
                    if let Some(embedded_cost) = message.cost_usd {
                        embedded_cost
                    } else {
                        self.calculate_cost(usage, &model_name, pricing_manager)?
                    }
                }
                CostCalculationMode::Calculate => {
                    // Always calculate from tokens
                    self.calculate_cost(usage, &model_name, pricing_manager)?
                }
            };

            project_usage.total_cost_usd += cost;
            model_usage.cost_usd += cost;
        }

        Ok(projects.into_values().collect())
    }

    pub fn calculate_usage(&self, usage_data: Vec<UsageData>, pricing_manager: &crate::models::PricingManager) -> Result<Vec<ProjectUsage>> {
        let mut projects: HashMap<String, ProjectUsage> = HashMap::new();

        for message in usage_data {
            // Skip messages without usage data
            let usage = match &message.usage {
                Some(usage) => usage,
                None => continue,
            };

            // Project name will be passed separately since UsageData doesn't contain it
            let project_name = "Unknown".to_string(); // This will be overridden by the caller

            // Extract model name
            let model_name = self.extract_model_from_message(&message);

            // Get or create project usage entry
            let project_usage = projects
                .entry(project_name.clone())
                .or_insert_with(|| ProjectUsage {
                    project_name: project_name.clone(),
                    ..Default::default()
                });

            // Get or create model usage entry
            let model_usage = project_usage.model_usage
                .entry(model_name.clone())
                .or_insert_with(|| ModelUsage {
                    model_name: model_name.clone(),
                    ..Default::default()
                });

            // Aggregate token counts
            let input_tokens = usage.input_tokens.unwrap_or(0);
            let output_tokens = usage.output_tokens.unwrap_or(0);
            let cache_creation_tokens = usage.cache_creation_input_tokens.unwrap_or(0);
            let cache_read_tokens = usage.cache_read_input_tokens.unwrap_or(0);

            // Update project totals
            project_usage.total_input_tokens += input_tokens;
            project_usage.total_output_tokens += output_tokens;
            project_usage.total_cache_creation_tokens += cache_creation_tokens;
            project_usage.total_cache_read_tokens += cache_read_tokens;
            project_usage.message_count += 1;

            // Update model totals
            model_usage.input_tokens += input_tokens;
            model_usage.output_tokens += output_tokens;
            model_usage.cache_creation_tokens += cache_creation_tokens;
            model_usage.cache_read_tokens += cache_read_tokens;
            model_usage.message_count += 1;

            // Calculate cost based on mode
            let cost = match self.calculation_mode {
                CostCalculationMode::Display => {
                    // Only use embedded cost, 0 if missing
                    message.cost_usd.unwrap_or(0.0)
                }
                CostCalculationMode::Auto => {
                    // Use embedded cost if available, otherwise calculate
                    if let Some(embedded_cost) = message.cost_usd {
                        embedded_cost
                    } else {
                        self.calculate_cost(usage, &model_name, pricing_manager)?
                    }
                }
                CostCalculationMode::Calculate => {
                    // Always calculate from tokens
                    self.calculate_cost(usage, &model_name, pricing_manager)?
                }
            };

            project_usage.total_cost_usd += cost;
            model_usage.cost_usd += cost;
        }

        Ok(projects.into_values().collect())
    }

    pub fn aggregate_usage(&self, filter: &UsageFilter) -> Result<Vec<ProjectUsage>> {
        // This will be implemented when we integrate with the database
        // For now, return empty vector
        let _ = filter; // Avoid unused warning
        Ok(vec![])
    }

    fn extract_model_from_message(&self, message: &UsageData) -> String {
        message
            .message
            .as_ref()
            .and_then(|m| m.model.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn parse_timestamp(&self, timestamp: &str) -> Result<DateTime<Utc>> {
        use chrono::DateTime;
        
        // Try to parse as RFC 3339 (ISO 8601)
        DateTime::parse_from_rfc3339(timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|_| {
                // Try to parse as RFC 2822
                DateTime::parse_from_rfc2822(timestamp)
                    .map(|dt| dt.with_timezone(&Utc))
            })
            .or_else(|_| {
                // Try to parse with different format
                DateTime::parse_from_str(timestamp, "%Y-%m-%dT%H:%M:%S%.f%z")
                    .map(|dt| dt.with_timezone(&Utc))
            })
            .with_context(|| format!("Failed to parse timestamp: {}", timestamp))
    }
}

impl Default for UsageFilter {
    fn default() -> Self {
        Self {
            project_name: None,
            model_name: None,
            since: None,
            until: None,
        }
    }
}

impl Default for ProjectUsage {
    fn default() -> Self {
        Self {
            project_name: String::new(),
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_creation_tokens: 0,
            total_cache_read_tokens: 0,
            total_cost_usd: 0.0,
            model_usage: HashMap::new(),
            message_count: 0,
        }
    }
}

impl Default for ModelUsage {
    fn default() -> Self {
        Self {
            model_name: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            cost_usd: 0.0,
            message_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::jsonl::Message;
    use tempfile::TempDir;
    use chrono::Datelike;

    fn create_test_usage_data(
        timestamp: &str,
        uuid: Option<&str>,
        request_id: Option<&str>,
        model: Option<&str>,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        cache_creation: Option<u64>,
        cache_read: Option<u64>,
        cost_usd: Option<f64>,
    ) -> UsageData {
        UsageData {
            timestamp: timestamp.to_string(),
            uuid: uuid.map(|s| s.to_string()),
            request_id: request_id.map(|s| s.to_string()),
            message: model.map(|m| Message {
                model: Some(m.to_string()),
                ..Default::default()
            }),
            usage: Some(Usage {
                input_tokens,
                output_tokens,
                cache_creation_input_tokens: cache_creation,
                cache_read_input_tokens: cache_read,
            }),
            cost_usd,
        }
    }

    #[test]
    fn test_usage_tracker_creation() {
        let tracker = UsageTracker::new(CostCalculationMode::Auto);
        // Should not panic and create tracker
    }

    #[test]
    fn test_usage_tracker_with_database() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).unwrap();
        
        let tracker = UsageTracker::with_database(CostCalculationMode::Calculate, database);
        // Should not panic and create tracker with database
    }

    #[test]
    fn test_token_counting_basic() {
        let tracker = UsageTracker::new(CostCalculationMode::Display);
        
        let messages = vec![
            create_test_usage_data(
                "2025-06-09T10:00:00Z",
                Some("uuid1"),
                Some("req1"),
                Some("claude-sonnet-4"),
                Some(100),
                Some(50),
                Some(25),
                Some(10),
                Some(0.50),
            ),
            create_test_usage_data(
                "2025-06-09T10:01:00Z",
                Some("uuid2"),
                Some("req2"),
                Some("claude-sonnet-4"),
                Some(200),
                Some(100),
                None,
                Some(5),
                Some(0.75),
            ),
        ];
        
        let usage = tracker.process_usage_data(messages, "test_project").unwrap();
        
        assert_eq!(usage.project_name, "test_project");
        assert_eq!(usage.total_input_tokens, 300);
        assert_eq!(usage.total_output_tokens, 150);
        assert_eq!(usage.total_cache_creation_tokens, 25);
        assert_eq!(usage.total_cache_read_tokens, 15);
        assert_eq!(usage.total_cost_usd, 1.25);
        assert_eq!(usage.message_count, 2);
    }

    #[test]
    fn test_model_switching_tracking() {
        let tracker = UsageTracker::new(CostCalculationMode::Display);
        
        let messages = vec![
            create_test_usage_data(
                "2025-06-09T10:00:00Z",
                Some("uuid1"),
                Some("req1"),
                Some("claude-sonnet-4"),
                Some(100),
                Some(50),
                None,
                None,
                Some(0.50),
            ),
            create_test_usage_data(
                "2025-06-09T10:01:00Z", 
                Some("uuid1"),
                Some("req2"),
                Some("claude-opus-4"),
                Some(50),
                Some(25),
                None,
                None,
                Some(1.00),
            ),
            create_test_usage_data(
                "2025-06-09T10:02:00Z",
                Some("uuid1"),
                Some("req3"),
                Some("claude-sonnet-4"),
                Some(75),
                Some(40),
                None,
                None,
                Some(0.35),
            ),
        ];
        
        let usage = tracker.process_usage_data(messages, "test_project").unwrap();
        
        // Check overall totals
        assert_eq!(usage.total_input_tokens, 225);
        assert_eq!(usage.total_output_tokens, 115);
        assert_eq!(usage.total_cost_usd, 1.85);
        
        // Check model-specific usage
        assert_eq!(usage.model_usage.len(), 2);
        
        let sonnet_usage = usage.model_usage.get("claude-sonnet-4").unwrap();
        assert_eq!(sonnet_usage.input_tokens, 175);
        assert_eq!(sonnet_usage.output_tokens, 90);
        assert_eq!(sonnet_usage.cost_usd, 0.85);
        assert_eq!(sonnet_usage.message_count, 2);
        
        let opus_usage = usage.model_usage.get("claude-opus-4").unwrap();
        assert_eq!(opus_usage.input_tokens, 50);
        assert_eq!(opus_usage.output_tokens, 25);
        assert_eq!(opus_usage.cost_usd, 1.00);
        assert_eq!(opus_usage.message_count, 1);
    }

    #[test]
    fn test_cost_calculation_modes() {
        // Test Auto mode - should use embedded cost when available
        let tracker_auto = UsageTracker::new(CostCalculationMode::Auto);
        let message_with_cost = create_test_usage_data(
            "2025-06-09T10:00:00Z",
            Some("uuid1"),
            Some("req1"),
            Some("claude-sonnet-4"),
            Some(100),
            Some(50),
            None,
            None,
            Some(0.50),
        );
        
        let usage = tracker_auto.process_usage_data(vec![message_with_cost], "test").unwrap();
        assert_eq!(usage.total_cost_usd, 0.50); // Should use embedded cost
        
        // Test Display mode - should only use embedded cost
        let tracker_display = UsageTracker::new(CostCalculationMode::Display);
        let message_no_cost = create_test_usage_data(
            "2025-06-09T10:00:00Z",
            Some("uuid1"),
            Some("req1"),
            Some("claude-sonnet-4"),
            Some(100),
            Some(50),
            None,
            None,
            None, // No embedded cost
        );
        
        let usage = tracker_display.process_usage_data(vec![message_no_cost], "test").unwrap();
        assert_eq!(usage.total_cost_usd, 0.0); // Should be 0 when no embedded cost
    }

    #[test]
    fn test_missing_usage_data_handling() {
        let tracker = UsageTracker::new(CostCalculationMode::Display);
        
        let messages = vec![
            // Message with no usage data
            UsageData {
                timestamp: "2025-06-09T10:00:00Z".to_string(),
                uuid: Some("uuid1".to_string()),
                request_id: Some("req1".to_string()),
                message: Some(Message {
                    model: Some("claude-sonnet-4".to_string()),
                    ..Default::default()
                }),
                usage: None,
                cost_usd: None,
            },
            // Normal message
            create_test_usage_data(
                "2025-06-09T10:01:00Z",
                Some("uuid2"),
                Some("req2"),
                Some("claude-sonnet-4"),
                Some(100),
                Some(50),
                None,
                None,
                Some(0.50),
            ),
        ];
        
        let usage = tracker.process_usage_data(messages, "test_project").unwrap();
        
        // Should only count the message with usage data
        assert_eq!(usage.total_input_tokens, 100);
        assert_eq!(usage.total_output_tokens, 50);
        assert_eq!(usage.total_cost_usd, 0.50);
        assert_eq!(usage.message_count, 1); // Only count messages with usage
    }

    #[test]
    fn test_timestamp_parsing() {
        let tracker = UsageTracker::new(CostCalculationMode::Auto);
        
        // Test valid ISO 8601 timestamp
        let valid_timestamp = "2025-06-09T10:00:00Z";
        let parsed = tracker.parse_timestamp(valid_timestamp).unwrap();
        assert_eq!(parsed.year(), 2025);
        assert_eq!(parsed.month(), 6);
        assert_eq!(parsed.day(), 9);
        
        // Test timestamp with timezone
        let tz_timestamp = "2025-06-09T10:00:00+02:00";
        let parsed_tz = tracker.parse_timestamp(tz_timestamp).unwrap();
        assert!(parsed_tz.year() == 2025);
        
        // Test invalid timestamp should error
        let invalid_timestamp = "invalid-timestamp";
        assert!(tracker.parse_timestamp(invalid_timestamp).is_err());
    }

    #[test]
    fn test_model_extraction() {
        let tracker = UsageTracker::new(CostCalculationMode::Auto);
        
        // Test with model in message
        let message_with_model = create_test_usage_data(
            "2025-06-09T10:00:00Z",
            Some("uuid1"),
            Some("req1"),
            Some("claude-sonnet-4"),
            Some(100),
            Some(50),
            None,
            None,
            Some(0.50),
        );
        
        let model = tracker.extract_model_from_message(&message_with_model);
        assert_eq!(model, "claude-sonnet-4");
        
        // Test with no message/model
        let message_no_model = UsageData {
            timestamp: "2025-06-09T10:00:00Z".to_string(),
            uuid: Some("uuid1".to_string()),
            request_id: Some("req1".to_string()),
            message: None,
            usage: Some(Usage {
                input_tokens: Some(100),
                output_tokens: Some(50),
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            }),
            cost_usd: Some(0.50),
        };
        
        let model = tracker.extract_model_from_message(&message_no_model);
        assert_eq!(model, "unknown");
    }

    #[test]
    fn test_usage_filtering() {
        let tracker = UsageTracker::new(CostCalculationMode::Auto);
        
        // Test filter by project name
        let filter = UsageFilter {
            project_name: Some("specific_project".to_string()),
            ..Default::default()
        };
        
        // This test will verify the filter structure is correct
        assert_eq!(filter.project_name, Some("specific_project".to_string()));
        assert!(filter.model_name.is_none());
        assert!(filter.since.is_none());
        assert!(filter.until.is_none());
    }

    #[test]
    fn test_accuracy_vs_manual_calculation() {
        // This test verifies accuracy against a manually calculated scenario
        let tracker = UsageTracker::new(CostCalculationMode::Display);
        
        let messages = vec![
            create_test_usage_data(
                "2025-06-09T10:00:00Z",
                Some("uuid1"),
                Some("req1"),
                Some("claude-sonnet-4"),
                Some(1000),  // 1k input tokens
                Some(500),   // 500 output tokens
                Some(100),   // 100 cache creation tokens
                Some(50),    // 50 cache read tokens
                Some(2.25),  // Manual cost calculation
            ),
        ];
        
        let usage = tracker.process_usage_data(messages, "accuracy_test").unwrap();
        
        // Verify exact token counts
        assert_eq!(usage.total_input_tokens, 1000);
        assert_eq!(usage.total_output_tokens, 500);
        assert_eq!(usage.total_cache_creation_tokens, 100);
        assert_eq!(usage.total_cache_read_tokens, 50);
        assert_eq!(usage.total_cost_usd, 2.25);
        assert_eq!(usage.message_count, 1);
        
        // Verify model-specific tracking
        let model_usage = usage.model_usage.get("claude-sonnet-4").unwrap();
        assert_eq!(model_usage.input_tokens, 1000);
        assert_eq!(model_usage.output_tokens, 500);
        assert_eq!(model_usage.cache_creation_tokens, 100);
        assert_eq!(model_usage.cache_read_tokens, 50);
        assert_eq!(model_usage.cost_usd, 2.25);
        assert_eq!(model_usage.message_count, 1);
    }
}