use tabled::{Table, Tabled};
use serde::Serialize;
use crate::analysis::usage::{ProjectUsage, ModelUsage};

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

impl OutputFormat for Vec<ProjectUsage> {
    fn to_table(&self) -> String {
        if self.is_empty() {
            return "No usage data found.".to_string();
        }
        
        let rows: Vec<ProjectUsageRow> = self.iter()
            .map(ProjectUsageRow::from_project_usage)
            .collect();
        
        Table::new(rows).to_string()
    }
    
    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    fn to_table_with_currency(&self, currency: &str, decimal_places: u8) -> String {
        if self.is_empty() {
            return "No usage data found.".to_string();
        }
        
        let rows: Vec<ProjectUsageRow> = self.iter()
            .map(|usage| ProjectUsageRow::from_project_usage_with_currency(usage, currency, decimal_places))
            .collect();
        
        Table::new(rows).to_string()
    }
}

impl OutputFormat for Vec<ModelUsage> {
    fn to_table(&self) -> String {
        if self.is_empty() {
            return "No model usage data found.".to_string();
        }
        
        let rows: Vec<ModelUsageRow> = self.iter()
            .map(ModelUsageRow::from_model_usage)
            .collect();
        
        Table::new(rows).to_string()
    }
    
    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    fn to_table_with_currency(&self, currency: &str, decimal_places: u8) -> String {
        if self.is_empty() {
            return "No model usage data found.".to_string();
        }
        
        let rows: Vec<ModelUsageRow> = self.iter()
            .map(|usage| ModelUsageRow::from_model_usage_with_currency(usage, currency, decimal_places))
            .collect();
        
        Table::new(rows).to_string()
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
}