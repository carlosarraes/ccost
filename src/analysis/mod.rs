// Analysis module
pub mod conversations;
pub mod optimization;
pub mod projects;
pub mod timeline;
pub mod timezone;
pub mod usage;

// Re-export key types for easier access (some temporarily unused)
#[allow(unused)]
pub use conversations::{
    ConversationAnalyzer, ConversationFilter, ConversationInsight, ConversationInsightList,
    ConversationSortBy,
};
#[allow(unused)]
pub use optimization::OptimizationEngine;
#[allow(unused)]
pub use projects::{ProjectAnalyzer, ProjectSortBy};
pub use timezone::TimezoneCalculator;
pub use usage::{CostCalculationMode, UsageFilter, UsageTracker};

use crate::output::OutputFormat;
use serde::Serialize;

// Daily usage analysis structures
#[derive(Debug, Clone, Serialize)]
pub struct DailyUsage {
    pub date: String,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cost_usd: f64,
    pub message_count: u64,
    pub projects_count: usize,
}

// Wrapper for daily usage vector to implement OutputFormat
#[derive(Debug, Clone, Serialize)]
pub struct DailyUsageList(pub Vec<DailyUsage>);

impl OutputFormat for DailyUsageList {
    fn to_table(&self) -> String {
        self.to_table_with_currency_and_color("USD", 2, false)
    }

    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.0)
    }

    fn to_table_with_currency(&self, currency: &str, decimal_places: u8) -> String {
        self.to_table_with_currency_and_color(currency, decimal_places, false)
    }

    fn to_table_with_currency_and_color(
        &self,
        currency: &str,
        decimal_places: u8,
        colored: bool,
    ) -> String {
        if self.0.is_empty() {
            return "No daily usage data found.".to_string();
        }

        // Convert to DailyUsageRow using the proper tabled infrastructure
        let mut rows: Vec<crate::output::DailyUsageRow> = self
            .0
            .iter()
            .map(|usage| {
                crate::output::DailyUsageRow::from_daily_usage_with_currency(
                    usage,
                    currency,
                    decimal_places,
                )
            })
            .collect();

        // Calculate totals for summary row
        let total_input: u64 = self.0.iter().map(|d| d.total_input_tokens).sum();
        let total_output: u64 = self.0.iter().map(|d| d.total_output_tokens).sum();
        let total_cache_creation: u64 = self.0.iter().map(|d| d.total_cache_creation_tokens).sum();
        let total_cache_read: u64 = self.0.iter().map(|d| d.total_cache_read_tokens).sum();
        let total_messages: u64 = self.0.iter().map(|d| d.message_count).sum();
        let total_cost: f64 = self.0.iter().map(|d| d.total_cost_usd).sum();
        let total_projects: usize = self.0.iter().map(|d| d.projects_count).sum();

        // Add totals row
        rows.push(crate::output::DailyUsageRow {
            date: "TOTAL".to_string(),
            input_tokens: crate::output::table::format_number(total_input),
            output_tokens: crate::output::table::format_number(total_output),
            cache_creation: crate::output::table::format_number(total_cache_creation),
            cache_read: crate::output::table::format_number(total_cache_read),
            messages: crate::output::table::format_number(total_messages),
            projects: total_projects.to_string(),
            total_cost: crate::models::currency::format_currency(
                total_cost,
                currency,
                decimal_places,
            ),
        });

        crate::output::table::apply_table_style_with_color(
            tabled::Table::new(rows),
            colored,
            crate::output::table::TableType::DailyUsage,
        )
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::parser::jsonl::{Message, Usage, UsageData};

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
                session_id: None,
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
                session_id: None,
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

        let enhanced_data: Vec<(UsageData, String)> = messages
            .into_iter()
            .map(|data| (data, "auto_test".to_string()))
            .collect();
        let usage_results = tracker_auto
            .calculate_usage_with_projects(enhanced_data, &crate::models::PricingManager::new())
            .unwrap();
        let usage = &usage_results[0];

        // Should use embedded cost for first message, calculated cost for second
        assert_eq!(usage.total_cost_usd, 0.75); // 0.75 + 0.0 (calculated)
        assert_eq!(usage.total_input_tokens, 300);
        assert_eq!(usage.total_output_tokens, 150);
    }
}
