use crate::parser::jsonl::{Usage, UsageData};
use crate::models::{PricingManager, PricingSource};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pricing_source: Option<String>,
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
#[derive(Default)]
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

    /// Enhanced cost calculation using live pricing when available
    pub async fn calculate_enhanced_cost(
        &self,
        usage: &Usage,
        model_name: &str,
        pricing_manager: &mut PricingManager,
    ) -> Result<(f64, PricingSource)> {
        let input_tokens = usage.input_tokens.unwrap_or(0);
        let output_tokens = usage.output_tokens.unwrap_or(0);
        let cache_creation_tokens = usage.cache_creation_input_tokens.unwrap_or(0);
        let cache_read_tokens = usage.cache_read_input_tokens.unwrap_or(0);

        let (cost, source) = pricing_manager.calculate_enhanced_cost(
            model_name,
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
        ).await;

        Ok((cost, source))
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
            if let (Some(since), Some(timestamp_str)) = (&filter.since, &message.timestamp)
                && let Ok(message_time) = self.parse_timestamp(timestamp_str)
                && message_time < *since
            {
                continue;
            }

            if let (Some(until), Some(timestamp_str)) = (&filter.until, &message.timestamp)
                && let Ok(message_time) = self.parse_timestamp(timestamp_str)
                && message_time > *until
            {
                continue;
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

    /// Enhanced version that uses live pricing when available
    pub async fn calculate_usage_with_projects_filtered_enhanced(
        &self,
        enhanced_data: Vec<(UsageData, String)>,
        pricing_manager: &mut PricingManager,
        filter: &UsageFilter,
    ) -> Result<(Vec<ProjectUsage>, Option<String>)> {
        let mut projects: HashMap<String, ProjectUsage> = HashMap::new();
        let mut pricing_sources: Vec<PricingSource> = Vec::new();

        for (message, project_name) in enhanced_data {
            // Apply timestamp filtering
            if let (Some(since), Some(timestamp_str)) = (&filter.since, &message.timestamp)
                && let Ok(message_time) = self.parse_timestamp(timestamp_str)
                && message_time < *since
            {
                continue;
            }

            if let (Some(until), Some(timestamp_str)) = (&filter.until, &message.timestamp)
                && let Ok(message_time) = self.parse_timestamp(timestamp_str)
                && message_time > *until
            {
                continue;
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

            // Calculate cost based on mode using enhanced pricing
            let (cost, source) = match self.calculation_mode {
                CostCalculationMode::Auto => {
                    // Use embedded cost if available, otherwise calculate with enhanced pricing
                    if let Some(embedded_cost) = message.cost_usd {
                        (embedded_cost, PricingSource::StaticFallback) // Treat embedded as static
                    } else {
                        self.calculate_enhanced_cost(usage, &model_name, pricing_manager).await?
                    }
                }
            };

            project_usage.total_cost_usd += cost;
            model_usage.cost_usd += cost;
            pricing_sources.push(source);
        }

        // Determine overall pricing source
        let overall_source = if pricing_sources.is_empty() {
            None
        } else {
            let live_count = pricing_sources.iter().filter(|&s| *s == PricingSource::LiteLLM).count();
            let total_count = pricing_sources.len();
            
            if live_count == total_count {
                Some("Live (LiteLLM)".to_string())
            } else if live_count > 0 {
                Some(format!("Mixed ({} live, {} static)", live_count, total_count - live_count))
            } else {
                Some("Static".to_string())
            }
        };

        // Set pricing source for each project
        for project in projects.values_mut() {
            project.pricing_source = overall_source.clone();
        }

        Ok((projects.into_values().collect(), overall_source))
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
            .with_context(|| format!("Failed to parse timestamp: {timestamp}"))
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
            pricing_source: None,
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
