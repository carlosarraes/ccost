use tabled::{Table, Tabled, settings::{Style, Alignment, Modify, object::{Rows, Columns}, Color}};
use serde::Serialize;
use crate::analysis::usage::{ProjectUsage, ModelUsage};
use crate::analysis::projects::ProjectSummary;

/// Trait for items that can be displayed as tables or JSON
pub trait OutputFormat {
    fn to_table(&self) -> String;
    fn to_json(&self) -> Result<String, serde_json::Error>;
    fn to_table_with_currency(&self, currency: &str, decimal_places: u8) -> String;
}

/// Row for project usage summary table
#[derive(Tabled, Serialize, Debug)]
pub struct ProjectUsageRow {
    #[tabled(rename = "Project")]
    pub project: String,
    #[tabled(rename = "Input Tokens")]
    pub input_tokens: String,
    #[tabled(rename = "Output Tokens")]
    pub output_tokens: String,
    #[tabled(rename = "Cache Creation")]
    pub cache_creation: String,
    #[tabled(rename = "Cache Read")]
    pub cache_read: String,
    #[tabled(rename = "Messages")]
    pub messages: String,
    #[tabled(rename = "Total Cost")]
    pub total_cost: String,
}

/// Row for model usage breakdown table
#[derive(Tabled, Serialize, Debug)]
pub struct ModelUsageRow {
    #[tabled(rename = "Model")]
    pub model: String,
    #[tabled(rename = "Input Tokens")]
    pub input_tokens: String,
    #[tabled(rename = "Output Tokens")]
    pub output_tokens: String,
    #[tabled(rename = "Cache Creation")]
    pub cache_creation: String,
    #[tabled(rename = "Cache Read")]
    pub cache_read: String,
    #[tabled(rename = "Messages")]
    pub messages: String,
    #[tabled(rename = "Cost")]
    pub cost: String,
}

/// Row for project summary table (simplified view)
#[derive(Tabled, Serialize, Debug)]
pub struct ProjectSummaryRow {
    #[tabled(rename = "Project")]
    pub project: String,
    #[tabled(rename = "Total Tokens")]
    pub total_tokens: String,
    #[tabled(rename = "Messages")]
    pub messages: String,
    #[tabled(rename = "Models")]
    pub models: String,
    #[tabled(rename = "Total Cost")]
    pub total_cost: String,
}

impl ProjectUsageRow {
    pub fn from_project_usage(usage: &ProjectUsage) -> Self {
        Self {
            project: usage.project_name.clone(),
            input_tokens: format_number(usage.total_input_tokens),
            output_tokens: format_number(usage.total_output_tokens),
            cache_creation: format_number(usage.total_cache_creation_tokens),
            cache_read: format_number(usage.total_cache_read_tokens),
            messages: format_number(usage.message_count),
            total_cost: format_currency(usage.total_cost_usd),
        }
    }

    pub fn from_project_usage_with_currency(usage: &ProjectUsage, currency: &str, decimal_places: u8) -> Self {
        Self {
            project: usage.project_name.clone(),
            input_tokens: format_number(usage.total_input_tokens),
            output_tokens: format_number(usage.total_output_tokens),
            cache_creation: format_number(usage.total_cache_creation_tokens),
            cache_read: format_number(usage.total_cache_read_tokens),
            messages: format_number(usage.message_count),
            total_cost: crate::models::currency::format_currency(usage.total_cost_usd, currency, decimal_places),
        }
    }
}

impl ModelUsageRow {
    pub fn from_model_usage(usage: &ModelUsage) -> Self {
        Self {
            model: usage.model_name.clone(),
            input_tokens: format_number(usage.input_tokens),
            output_tokens: format_number(usage.output_tokens),
            cache_creation: format_number(usage.cache_creation_tokens),
            cache_read: format_number(usage.cache_read_tokens),
            messages: format_number(usage.message_count),
            cost: format_currency(usage.cost_usd),
        }
    }

    pub fn from_model_usage_with_currency(usage: &ModelUsage, currency: &str, decimal_places: u8) -> Self {
        Self {
            model: usage.model_name.clone(),
            input_tokens: format_number(usage.input_tokens),
            output_tokens: format_number(usage.output_tokens),
            cache_creation: format_number(usage.cache_creation_tokens),
            cache_read: format_number(usage.cache_read_tokens),
            messages: format_number(usage.message_count),
            cost: crate::models::currency::format_currency(usage.cost_usd, currency, decimal_places),
        }
    }
}

impl ProjectSummaryRow {
    pub fn from_project_summary(summary: &ProjectSummary) -> Self {
        let total_tokens = summary.total_input_tokens + summary.total_output_tokens;
        Self {
            project: summary.project_name.clone(),
            total_tokens: format_number(total_tokens),
            messages: format_number(summary.message_count),
            models: summary.model_count.to_string(),
            total_cost: format_currency(summary.total_cost_usd),
        }
    }

    pub fn from_project_summary_with_currency(summary: &ProjectSummary, currency: &str, decimal_places: u8) -> Self {
        let total_tokens = summary.total_input_tokens + summary.total_output_tokens;
        Self {
            project: summary.project_name.clone(),
            total_tokens: format_number(total_tokens),
            messages: format_number(summary.message_count),
            models: summary.model_count.to_string(),
            total_cost: crate::models::currency::format_currency(summary.total_cost_usd, currency, decimal_places),
        }
    }
}

impl OutputFormat for Vec<ProjectUsage> {
    fn to_table(&self) -> String {
        if self.is_empty() {
            return "No usage data found.".to_string();
        }
        
        let mut rows: Vec<ProjectUsageRow> = self.iter()
            .map(ProjectUsageRow::from_project_usage)
            .collect();
        
        // Calculate totals for summary row
        let total_input: u64 = self.iter().map(|p| p.total_input_tokens).sum();
        let total_output: u64 = self.iter().map(|p| p.total_output_tokens).sum();
        let total_cache_creation: u64 = self.iter().map(|p| p.total_cache_creation_tokens).sum();
        let total_cache_read: u64 = self.iter().map(|p| p.total_cache_read_tokens).sum();
        let total_messages: u64 = self.iter().map(|p| p.message_count).sum();
        let total_cost: f64 = self.iter().map(|p| p.total_cost_usd).sum();
        
        // Add totals row
        rows.push(ProjectUsageRow {
            project: "TOTAL".to_string(),
            input_tokens: format_number(total_input),
            output_tokens: format_number(total_output),
            cache_creation: format_number(total_cache_creation),
            cache_read: format_number(total_cache_read),
            messages: format_number(total_messages),
            total_cost: format_currency(total_cost),
        });
        
        apply_table_style(Table::new(rows))
    }
    
    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    fn to_table_with_currency(&self, currency: &str, decimal_places: u8) -> String {
        if self.is_empty() {
            return "No usage data found.".to_string();
        }
        
        let mut rows: Vec<ProjectUsageRow> = self.iter()
            .map(|usage| ProjectUsageRow::from_project_usage_with_currency(usage, currency, decimal_places))
            .collect();
        
        // Calculate totals for summary row
        let total_input: u64 = self.iter().map(|p| p.total_input_tokens).sum();
        let total_output: u64 = self.iter().map(|p| p.total_output_tokens).sum();
        let total_cache_creation: u64 = self.iter().map(|p| p.total_cache_creation_tokens).sum();
        let total_cache_read: u64 = self.iter().map(|p| p.total_cache_read_tokens).sum();
        let total_messages: u64 = self.iter().map(|p| p.message_count).sum();
        let total_cost: f64 = self.iter().map(|p| p.total_cost_usd).sum();
        
        // Add totals row
        rows.push(ProjectUsageRow {
            project: "TOTAL".to_string(),
            input_tokens: format_number(total_input),
            output_tokens: format_number(total_output),
            cache_creation: format_number(total_cache_creation),
            cache_read: format_number(total_cache_read),
            messages: format_number(total_messages),
            total_cost: crate::models::currency::format_currency(total_cost, currency, decimal_places),
        });
        
        apply_table_style(Table::new(rows))
    }
}

impl OutputFormat for Vec<ModelUsage> {
    fn to_table(&self) -> String {
        if self.is_empty() {
            return "No model usage data found.".to_string();
        }
        
        let mut rows: Vec<ModelUsageRow> = self.iter()
            .map(ModelUsageRow::from_model_usage)
            .collect();
        
        // Calculate totals for summary row
        let total_input: u64 = self.iter().map(|m| m.input_tokens).sum();
        let total_output: u64 = self.iter().map(|m| m.output_tokens).sum();
        let total_cache_creation: u64 = self.iter().map(|m| m.cache_creation_tokens).sum();
        let total_cache_read: u64 = self.iter().map(|m| m.cache_read_tokens).sum();
        let total_messages: u64 = self.iter().map(|m| m.message_count).sum();
        let total_cost: f64 = self.iter().map(|m| m.cost_usd).sum();
        
        // Add totals row
        rows.push(ModelUsageRow {
            model: "TOTAL".to_string(),
            input_tokens: format_number(total_input),
            output_tokens: format_number(total_output),
            cache_creation: format_number(total_cache_creation),
            cache_read: format_number(total_cache_read),
            messages: format_number(total_messages),
            cost: format_currency(total_cost),
        });
        
        apply_table_style(Table::new(rows))
    }
    
    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    fn to_table_with_currency(&self, currency: &str, decimal_places: u8) -> String {
        if self.is_empty() {
            return "No model usage data found.".to_string();
        }
        
        let mut rows: Vec<ModelUsageRow> = self.iter()
            .map(|usage| ModelUsageRow::from_model_usage_with_currency(usage, currency, decimal_places))
            .collect();
        
        // Calculate totals for summary row
        let total_input: u64 = self.iter().map(|m| m.input_tokens).sum();
        let total_output: u64 = self.iter().map(|m| m.output_tokens).sum();
        let total_cache_creation: u64 = self.iter().map(|m| m.cache_creation_tokens).sum();
        let total_cache_read: u64 = self.iter().map(|m| m.cache_read_tokens).sum();
        let total_messages: u64 = self.iter().map(|m| m.message_count).sum();
        let total_cost: f64 = self.iter().map(|m| m.cost_usd).sum();
        
        // Add totals row
        rows.push(ModelUsageRow {
            model: "TOTAL".to_string(),
            input_tokens: format_number(total_input),
            output_tokens: format_number(total_output),
            cache_creation: format_number(total_cache_creation),
            cache_read: format_number(total_cache_read),
            messages: format_number(total_messages),
            cost: crate::models::currency::format_currency(total_cost, currency, decimal_places),
        });
        
        apply_table_style(Table::new(rows))
    }
}

impl OutputFormat for Vec<ProjectSummary> {
    fn to_table(&self) -> String {
        if self.is_empty() {
            return "No project data found.".to_string();
        }
        
        let mut rows: Vec<ProjectSummaryRow> = self.iter()
            .map(ProjectSummaryRow::from_project_summary)
            .collect();
        
        // Calculate totals for summary row
        let total_input: u64 = self.iter().map(|p| p.total_input_tokens).sum();
        let total_output: u64 = self.iter().map(|p| p.total_output_tokens).sum();
        let total_messages: u64 = self.iter().map(|p| p.message_count).sum();
        let total_models: usize = self.iter().map(|p| p.model_count).sum();
        let total_cost: f64 = self.iter().map(|p| p.total_cost_usd).sum();
        
        // Add totals row
        rows.push(ProjectSummaryRow {
            project: "TOTAL".to_string(),
            total_tokens: format_number(total_input + total_output),
            messages: format_number(total_messages),
            models: total_models.to_string(),
            total_cost: format_currency(total_cost),
        });
        
        apply_table_style(Table::new(rows))
    }
    
    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    fn to_table_with_currency(&self, currency: &str, decimal_places: u8) -> String {
        if self.is_empty() {
            return "No project data found.".to_string();
        }
        
        let mut rows: Vec<ProjectSummaryRow> = self.iter()
            .map(|summary| ProjectSummaryRow::from_project_summary_with_currency(summary, currency, decimal_places))
            .collect();
        
        // Calculate totals for summary row
        let total_input: u64 = self.iter().map(|p| p.total_input_tokens).sum();
        let total_output: u64 = self.iter().map(|p| p.total_output_tokens).sum();
        let total_messages: u64 = self.iter().map(|p| p.message_count).sum();
        let total_models: usize = self.iter().map(|p| p.model_count).sum();
        let total_cost: f64 = self.iter().map(|p| p.total_cost_usd).sum();
        
        // Add totals row
        rows.push(ProjectSummaryRow {
            project: "TOTAL".to_string(),
            total_tokens: format_number(total_input + total_output),
            messages: format_number(total_messages),
            models: total_models.to_string(),
            total_cost: crate::models::currency::format_currency(total_cost, currency, decimal_places),
        });
        
        apply_table_style(Table::new(rows))
    }
}

/// Format a number with commas for thousands separator
fn format_number(n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    
    let mut result = String::new();
    let s = n.to_string();
    let chars: Vec<char> = s.chars().collect();
    
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*ch);
    }
    
    result
}

/// Format currency value with proper decimal places and dollar sign
fn format_currency(amount: f64) -> String {
    if amount == 0.0 {
        return "$0.00".to_string();
    }
    
    format!("${:.2}", amount)
}

/// Apply modern table styling similar to the design reference  
fn apply_table_style(mut table: Table) -> String {
    table
        .with(Style::blank()) // Minimal styling - no borders
        // Set alignment
        .modify(Rows::new(1..), Alignment::right()) // Right-align all data rows
        .modify(Columns::new(0..1), Alignment::left())  // Left-align first column (project/model names)
        // Add colors matching the design reference
        .modify(Columns::new(0..1), Color::FG_WHITE)    // Project name in white  
        .modify(Columns::new(1..3), Color::FG_BLUE)     // Token columns in blue
        .modify(Columns::new(3..5), Color::FG_GREEN)    // Cache columns in green
        .modify(Columns::new(5..6), Color::FG_YELLOW)   // Messages in yellow  
        .modify(Columns::new(6..7), Color::FG_RED)      // Cost in red
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(123), "123");
        assert_eq!(format_number(1234), "1,234");
        assert_eq!(format_number(1234567), "1,234,567");
        assert_eq!(format_number(1000000000), "1,000,000,000");
    }

    #[test]
    fn test_format_currency() {
        assert_eq!(format_currency(0.0), "$0.00");
        assert_eq!(format_currency(1.5), "$1.50");
        assert_eq!(format_currency(123.456), "$123.46");
        assert_eq!(format_currency(1000.0), "$1000.00");
    }

    #[test]
    fn test_project_usage_row_creation() {
        let mut model_usage = HashMap::new();
        model_usage.insert("claude-3-5-sonnet".to_string(), ModelUsage {
            model_name: "claude-3-5-sonnet".to_string(),
            input_tokens: 1000,
            output_tokens: 500,
            cache_creation_tokens: 100,
            cache_read_tokens: 50,
            cost_usd: 2.5,
            message_count: 5,
        });

        let project_usage = ProjectUsage {
            project_name: "test-project".to_string(),
            total_input_tokens: 1000,
            total_output_tokens: 500,
            total_cache_creation_tokens: 100,
            total_cache_read_tokens: 50,
            total_cost_usd: 2.5,
            model_usage,
            message_count: 5,
        };

        let row = ProjectUsageRow::from_project_usage(&project_usage);
        assert_eq!(row.project, "test-project");
        assert_eq!(row.input_tokens, "1,000");
        assert_eq!(row.output_tokens, "500");
        assert_eq!(row.cache_creation, "100");
        assert_eq!(row.cache_read, "50");
        assert_eq!(row.messages, "5");
        assert_eq!(row.total_cost, "$2.50");
    }

    #[test]
    fn test_model_usage_row_creation() {
        let model_usage = ModelUsage {
            model_name: "claude-3-5-sonnet".to_string(),
            input_tokens: 2500,
            output_tokens: 1250,
            cache_creation_tokens: 250,
            cache_read_tokens: 125,
            cost_usd: 7.89,
            message_count: 12,
        };

        let row = ModelUsageRow::from_model_usage(&model_usage);
        assert_eq!(row.model, "claude-3-5-sonnet");
        assert_eq!(row.input_tokens, "2,500");
        assert_eq!(row.output_tokens, "1,250");
        assert_eq!(row.cache_creation, "250");
        assert_eq!(row.cache_read, "125");
        assert_eq!(row.messages, "12");
        assert_eq!(row.cost, "$7.89");
    }

    #[test]
    fn test_empty_project_usage_table() {
        let empty_usage: Vec<ProjectUsage> = vec![];
        assert_eq!(empty_usage.to_table(), "No usage data found.");
    }

    #[test]
    fn test_project_usage_json_output() {
        let project_usage = vec![ProjectUsage {
            project_name: "test".to_string(),
            total_input_tokens: 100,
            total_output_tokens: 50,
            total_cache_creation_tokens: 10,
            total_cache_read_tokens: 5,
            total_cost_usd: 1.25,
            model_usage: HashMap::new(),
            message_count: 2,
        }];

        let json = project_usage.to_json().unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("100"));
        assert!(json.contains("1.25"));
    }

    #[test]
    fn test_model_usage_json_output() {
        let model_usage = vec![ModelUsage {
            model_name: "claude-3-5-sonnet".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_tokens: 10,
            cache_read_tokens: 5,
            cost_usd: 1.25,
            message_count: 2,
        }];

        let json = model_usage.to_json().unwrap();
        assert!(json.contains("claude-3-5-sonnet"));
        assert!(json.contains("100"));
        assert!(json.contains("1.25"));
    }

    #[test]
    fn test_project_summary_row_creation() {
        let project_summary = ProjectSummary {
            project_name: "test-project".to_string(),
            total_input_tokens: 1500,
            total_output_tokens: 750,
            total_cost_usd: 3.75,
            message_count: 15,
            model_count: 2,
        };

        let row = ProjectSummaryRow::from_project_summary(&project_summary);
        assert_eq!(row.project, "test-project");
        assert_eq!(row.total_tokens, "2,250"); // 1500 + 750
        assert_eq!(row.messages, "15");
        assert_eq!(row.models, "2");
        assert_eq!(row.total_cost, "$3.75");
    }

    #[test]
    fn test_project_summary_row_with_currency() {
        let project_summary = ProjectSummary {
            project_name: "euro-project".to_string(),
            total_input_tokens: 2000,
            total_output_tokens: 1000,
            total_cost_usd: 5.0, // Actually converted EUR value
            message_count: 20,
            model_count: 3,
        };

        let row = ProjectSummaryRow::from_project_summary_with_currency(&project_summary, "EUR", 2);
        assert_eq!(row.project, "euro-project");
        assert_eq!(row.total_tokens, "3,000"); // 2000 + 1000
        assert_eq!(row.messages, "20");
        assert_eq!(row.models, "3");
        assert_eq!(row.total_cost, "5.00 €");
    }

    #[test]
    fn test_project_summaries_table_output() {
        let project_summaries = vec![
            ProjectSummary {
                project_name: "project-a".to_string(),
                total_input_tokens: 1000,
                total_output_tokens: 500,
                total_cost_usd: 2.5,
                message_count: 10,
                model_count: 1,
            },
            ProjectSummary {
                project_name: "project-b".to_string(),
                total_input_tokens: 2000,
                total_output_tokens: 1000,
                total_cost_usd: 5.0,
                message_count: 20,
                model_count: 2,
            },
        ];

        let table = project_summaries.to_table();
        assert!(table.contains("project-a"));
        assert!(table.contains("project-b"));
        assert!(table.contains("1,500")); // total tokens for project-a
        assert!(table.contains("3,000")); // total tokens for project-b
        assert!(table.contains("$2.50"));
        assert!(table.contains("$5.00"));
    }

    #[test]
    fn test_empty_project_summaries_table() {
        let empty_summaries: Vec<ProjectSummary> = vec![];
        assert_eq!(empty_summaries.to_table(), "No project data found.");
    }

    #[test]
    fn test_project_summaries_json_output() {
        let project_summaries = vec![ProjectSummary {
            project_name: "json-test".to_string(),
            total_input_tokens: 800,
            total_output_tokens: 400,
            total_cost_usd: 1.75,
            message_count: 8,
            model_count: 1,
        }];

        let json = project_summaries.to_json().unwrap();
        assert!(json.contains("json-test"));
        assert!(json.contains("800"));
        assert!(json.contains("400"));
        assert!(json.contains("1.75"));
        assert!(json.contains("8"));
        assert!(json.contains("1"));
    }
}