use crate::output::OutputFormat;
use crate::parser::jsonl::UsageData;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;

/// Represents a conversation with all its messages and metadata
#[derive(Debug, Clone, Serialize)]
pub struct Conversation {
    pub conversation_id: String,
    pub project_name: String,
    pub messages: Vec<UsageData>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_minutes: f64,
}

/// Analysis results for a single conversation
#[derive(Debug, Clone, Serialize)]
pub struct ConversationInsight {
    pub conversation_id: String,
    pub project_name: String,
    pub total_cost: f64,
    pub message_count: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub efficiency_score: f32,
    pub cost_per_message: f64,
    pub cost_per_token: f64,
    pub model_usage: HashMap<String, ConversationModelUsage>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_minutes: f64,
    pub cache_hit_rate: f32,
}

/// Model usage within a conversation
#[derive(Debug, Clone, Serialize)]
pub struct ConversationModelUsage {
    pub model_name: String,
    pub message_count: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost_usd: f64,
    pub cost_percentage: f32,
}

/// Wrapper for conversation insights to implement OutputFormat
#[derive(Debug, Clone, Serialize)]
pub struct ConversationInsightList(pub Vec<ConversationInsight>);

impl OutputFormat for ConversationInsightList {
    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.0)
    }

    fn to_table_with_currency_and_color(
        &self,
        currency: &str,
        decimal_places: u8,
        colored: bool,
    ) -> String {
        if self.0.is_empty() {
            return "No conversation insights found.".to_string();
        }

        use crate::output::table::{TableType, apply_table_style_with_color, format_number};
        use tabled::{Table, Tabled};

        #[derive(Tabled)]
        struct ConversationRow {
            #[tabled(rename = "Conversation ID")]
            conversation_id: String,
            #[tabled(rename = "Project")]
            project: String,
            #[tabled(rename = "Messages")]
            messages: String,
            #[tabled(rename = "Total Cost")]
            total_cost: String,
            #[tabled(rename = "Efficiency")]
            efficiency: String,
            #[tabled(rename = "Models")]
            models: String,
            #[tabled(rename = "Duration")]
            duration: String,
        }

        let rows: Vec<ConversationRow> = self
            .0
            .iter()
            .map(|insight| {
                let conversation_id = if insight.conversation_id.len() > 12 {
                    format!("{}...", &insight.conversation_id[..12])
                } else {
                    insight.conversation_id.clone()
                };

                let models: Vec<String> = insight.model_usage.keys().cloned().collect();
                let models_str = if models.len() > 2 {
                    format!("{}, {} (+{})", models[0], models[1], models.len() - 2)
                } else {
                    models.join(", ")
                };

                let duration_str = if insight.duration_minutes > 60.0 {
                    format!("{:.1}h", insight.duration_minutes / 60.0)
                } else {
                    format!("{:.1}m", insight.duration_minutes)
                };

                ConversationRow {
                    conversation_id,
                    project: insight.project_name.clone(),
                    messages: format_number(insight.message_count),
                    total_cost: crate::models::currency::format_currency(
                        insight.total_cost,
                        currency,
                        decimal_places,
                    ),
                    efficiency: format!("{:.1}%", insight.efficiency_score),
                    models: models_str,
                    duration: duration_str,
                }
            })
            .collect();

        apply_table_style_with_color(Table::new(rows), colored, TableType::Conversations)
    }
}