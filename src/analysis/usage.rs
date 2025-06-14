use crate::parser::jsonl::{Usage, UsageData};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum CostCalculationMode {
    Auto, // Use embedded costUSD if available, otherwise calculate
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
}

impl UsageTracker {
    pub fn new(mode: CostCalculationMode) -> Self {
        Self {
            calculation_mode: mode,
        }
    }

    pub fn calculate_cost(
        &self,
        usage: &Usage,
        model_name: &str,
        pricing_manager: &crate::models::PricingManager,
    ) -> Result<f64> {
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

    pub fn calculate_usage_with_projects_filtered(
        &self,
        enhanced_data: Vec<(UsageData, String)>,
        pricing_manager: &crate::models::PricingManager,
        filter: &UsageFilter,
    ) -> Result<Vec<ProjectUsage>> {
        let mut projects: HashMap<String, ProjectUsage> = HashMap::new();

        for (message, project_name) in enhanced_data {
            // Apply timestamp filtering
            if let (Some(since), Some(timestamp_str)) = (&filter.since, &message.timestamp) {
                if let Ok(message_time) = self.parse_timestamp(timestamp_str) {
                    if message_time < *since {
                        continue;
                    }
                }
            }

            if let (Some(until), Some(timestamp_str)) = (&filter.until, &message.timestamp) {
                if let Ok(message_time) = self.parse_timestamp(timestamp_str) {
                    if message_time > *until {
                        continue;
                    }
                }
            }

            // Skip messages without usage data
            let usage = match &message.usage {
                Some(usage) => usage,
                None => continue,
            };

            // Extract model name
            let model_name = self.extract_model_from_message(&message);

            // Get or create project usage entry
            let project_usage =
                projects
                    .entry(project_name.clone())
                    .or_insert_with(|| ProjectUsage {
                        project_name: project_name.clone(),
                        ..Default::default()
                    });

            // Get or create model usage entry
            let model_usage = project_usage
                .model_usage
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
                CostCalculationMode::Auto => {
                    // Use embedded cost if available, otherwise calculate
                    if let Some(embedded_cost) = message.cost_usd {
                        embedded_cost
                    } else {
                        self.calculate_cost(usage, &model_name, pricing_manager)?
                    }
                }
            };

            project_usage.total_cost_usd += cost;
            model_usage.cost_usd += cost;
        }

        Ok(projects.into_values().collect())
    }

    fn extract_model_from_message(&self, message: &UsageData) -> String {
        message
            .message
            .as_ref()
            .and_then(|m| m.model.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }

    pub fn parse_timestamp(&self, timestamp: &str) -> Result<DateTime<Utc>> {
        use chrono::DateTime;

        // Try to parse as RFC 3339 (ISO 8601)
        DateTime::parse_from_rfc3339(timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|_| {
                // Try to parse as RFC 2822
                DateTime::parse_from_rfc2822(timestamp).map(|dt| dt.with_timezone(&Utc))
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
