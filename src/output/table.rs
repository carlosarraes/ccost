use crate::analysis::projects::ProjectSummary;
use crate::analysis::usage::{ModelUsage, ProjectUsage};
use serde::Serialize;
use tabled::{
    Table, Tabled,
    settings::{
        Alignment, Color, Style,
        object::{Columns, Object, Rows},
    },
};

/// Trait for items that can be displayed as tables or JSON
pub trait OutputFormat {
    fn to_json(&self) -> Result<String, serde_json::Error>;
    fn to_table_with_currency_and_color(
        &self,
        currency: &str,
        decimal_places: u8,
        colored: bool,
    ) -> String;
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

/// Row for daily usage table
#[derive(Tabled, Serialize, Debug)]
pub struct DailyUsageRow {
    #[tabled(rename = "Date")]
    pub date: String,
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
    #[tabled(rename = "Projects")]
    pub projects: String,
    #[tabled(rename = "Total Cost")]
    pub total_cost: String,
}

impl ProjectUsageRow {
    pub fn from_project_usage_with_currency(
        usage: &ProjectUsage,
        currency: &str,
        decimal_places: u8,
    ) -> Self {
        Self {
            project: usage.project_name.clone(),
            input_tokens: format_number(usage.total_input_tokens),
            output_tokens: format_number(usage.total_output_tokens),
            cache_creation: format_number(usage.total_cache_creation_tokens),
            cache_read: format_number(usage.total_cache_read_tokens),
            messages: format_number(usage.message_count),
            total_cost: crate::models::currency::format_currency(
                usage.total_cost_usd,
                currency,
                decimal_places,
            ),
        }
    }
}

impl ModelUsageRow {
    pub fn from_model_usage_with_currency(
        usage: &ModelUsage,
        currency: &str,
        decimal_places: u8,
    ) -> Self {
        Self {
            model: usage.model_name.clone(),
            input_tokens: format_number(usage.input_tokens),
            output_tokens: format_number(usage.output_tokens),
            cache_creation: format_number(usage.cache_creation_tokens),
            cache_read: format_number(usage.cache_read_tokens),
            messages: format_number(usage.message_count),
            cost: crate::models::currency::format_currency(
                usage.cost_usd,
                currency,
                decimal_places,
            ),
        }
    }
}

impl ProjectSummaryRow {
    pub fn from_project_summary_with_currency(
        summary: &ProjectSummary,
        currency: &str,
        decimal_places: u8,
    ) -> Self {
        let total_tokens = summary.total_input_tokens + summary.total_output_tokens;
        Self {
            project: summary.project_name.clone(),
            total_tokens: format_number(total_tokens),
            messages: format_number(summary.message_count),
            models: summary.model_count.to_string(),
            total_cost: crate::models::currency::format_currency(
                summary.total_cost_usd,
                currency,
                decimal_places,
            ),
        }
    }
}

impl DailyUsageRow {
    pub fn from_daily_usage_with_currency(
        usage: &crate::analysis::DailyUsage,
        currency: &str,
        decimal_places: u8,
    ) -> Self {
        Self {
            date: usage.date.clone(),
            input_tokens: format_number(usage.total_input_tokens),
            output_tokens: format_number(usage.total_output_tokens),
            cache_creation: format_number(usage.total_cache_creation_tokens),
            cache_read: format_number(usage.total_cache_read_tokens),
            messages: format_number(usage.message_count),
            projects: usage.projects_count.to_string(),
            total_cost: crate::models::currency::format_currency(
                usage.total_cost_usd,
                currency,
                decimal_places,
            ),
        }
    }
}

impl OutputFormat for Vec<ProjectUsage> {
    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    fn to_table_with_currency_and_color(
        &self,
        currency: &str,
        decimal_places: u8,
        colored: bool,
    ) -> String {
        if self.is_empty() {
            return "No usage data found.".to_string();
        }

        let mut rows: Vec<ProjectUsageRow> = self
            .iter()
            .map(|usage| {
                ProjectUsageRow::from_project_usage_with_currency(usage, currency, decimal_places)
            })
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
            total_cost: crate::models::currency::format_currency(
                total_cost,
                currency,
                decimal_places,
            ),
        });

        apply_table_style_with_color(Table::new(rows), colored, TableType::ProjectUsage)
    }
}

impl OutputFormat for Vec<ModelUsage> {
    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    fn to_table_with_currency_and_color(
        &self,
        currency: &str,
        decimal_places: u8,
        colored: bool,
    ) -> String {
        if self.is_empty() {
            return "No model usage data found.".to_string();
        }

        let mut rows: Vec<ModelUsageRow> = self
            .iter()
            .map(|usage| {
                ModelUsageRow::from_model_usage_with_currency(usage, currency, decimal_places)
            })
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

        apply_table_style_with_color(Table::new(rows), colored, TableType::ModelUsage)
    }
}

impl OutputFormat for Vec<ProjectSummary> {
    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    fn to_table_with_currency_and_color(
        &self,
        currency: &str,
        decimal_places: u8,
        colored: bool,
    ) -> String {
        if self.is_empty() {
            return "No project data found.".to_string();
        }

        let mut rows: Vec<ProjectSummaryRow> = self
            .iter()
            .map(|summary| {
                ProjectSummaryRow::from_project_summary_with_currency(
                    summary,
                    currency,
                    decimal_places,
                )
            })
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
            total_cost: crate::models::currency::format_currency(
                total_cost,
                currency,
                decimal_places,
            ),
        });

        apply_table_style_with_color(Table::new(rows), colored, TableType::ProjectSummary)
    }
}

/// Format a number with commas for thousands separator
pub fn format_number(n: u64) -> String {
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

/// Table type enum for proper column coloring
#[derive(Debug, Clone)]
pub enum TableType {
    ProjectUsage,
    ModelUsage,
    ProjectSummary,
    DailyUsage,
    Conversations,
}

/// Strip ANSI escape codes from a string to get its visual length
fn strip_ansi_codes(s: &str) -> String {
    // This regex matches ANSI escape sequences
    let ansi_regex = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    ansi_regex.replace_all(s, "").to_string()
}

/// Apply modern table styling with optional column colors
pub fn apply_table_style_with_color(
    mut table: Table,
    colored: bool,
    table_type: TableType,
) -> String {
    // Start with no borders
    let style = Style::blank();

    // First apply the base style
    table.with(style);

    // Set alignment
    table.modify(Rows::new(1..), Alignment::right()); // Right-align all data rows
    table.modify(Columns::new(0..1), Alignment::left()); // Left-align first column (project/model names)

    // Apply header styling based on colored flag
    if colored {
        // Apply bold to headers
        table.modify(Rows::first(), Color::BOLD);

        // Apply column-specific colors to headers AND data
        match table_type {
            TableType::ProjectUsage => {
                // Project, Input Tokens, Output Tokens, Cache Creation, Cache Read, Messages, Total Cost
                table.modify(Columns::single(1), Color::FG_BLUE); // Input Tokens
                table.modify(Columns::single(2), Color::FG_BLUE); // Output Tokens  
                table.modify(Columns::single(3), Color::FG_GREEN); // Cache Creation
                table.modify(Columns::single(4), Color::FG_GREEN); // Cache Read
                table.modify(Columns::single(5), Color::FG_YELLOW); // Messages
                table.modify(Columns::last(), Color::FG_RED); // Total Cost
            }
            TableType::ModelUsage => {
                // Model, Input Tokens, Output Tokens, Cache Creation, Cache Read, Messages, Cost
                table.modify(Columns::single(1), Color::FG_BLUE); // Input Tokens
                table.modify(Columns::single(2), Color::FG_BLUE); // Output Tokens  
                table.modify(Columns::single(3), Color::FG_GREEN); // Cache Creation
                table.modify(Columns::single(4), Color::FG_GREEN); // Cache Read
                table.modify(Columns::single(5), Color::FG_YELLOW); // Messages
                table.modify(Columns::last(), Color::FG_RED); // Cost
            }
            TableType::ProjectSummary => {
                // Project, Total Tokens, Messages, Models, Total Cost
                table.modify(Columns::single(1), Color::FG_BLUE); // Total Tokens
                table.modify(Columns::single(2), Color::FG_YELLOW); // Messages
                table.modify(Columns::single(3), Color::FG_CYAN); // Models
                table.modify(Columns::last(), Color::FG_RED); // Total Cost
            }
            TableType::DailyUsage => {
                // Date, Input Tokens, Output Tokens, Cache Creation, Cache Read, Messages, Projects, Total Cost
                table.modify(Columns::single(1), Color::FG_BLUE); // Input Tokens
                table.modify(Columns::single(2), Color::FG_BLUE); // Output Tokens  
                table.modify(Columns::single(3), Color::FG_GREEN); // Cache Creation
                table.modify(Columns::single(4), Color::FG_GREEN); // Cache Read
                table.modify(Columns::single(5), Color::FG_YELLOW); // Messages
                table.modify(Columns::single(6), Color::FG_CYAN); // Projects
                table.modify(Columns::last(), Color::FG_RED); // Total Cost
            }
            TableType::Conversations => {
                // Conversation ID, Project, Messages, Total Cost, Efficiency, Models, Outliers, Duration
                table.modify(Columns::single(2), Color::FG_YELLOW); // Messages
                table.modify(Columns::single(3), Color::FG_RED); // Total Cost
                table.modify(Columns::single(4), Color::FG_GREEN); // Efficiency
                table.modify(Columns::single(5), Color::FG_BLUE); // Models
                table.modify(Columns::single(6), Color::FG_MAGENTA); // Outliers
                table.modify(Columns::single(7), Color::FG_CYAN); // Duration
            }
        }
    } else {
        // Make headers bold and white (default non-colored mode)
        table.modify(Rows::first(), Color::FG_WHITE | Color::BOLD);

        // Always apply red to cost column (last column) even in non-colored mode
        table.modify(Columns::last().not(Rows::first()), Color::FG_RED);
    }

    let mut result = table.to_string();

    // Add horizontal lines manually with proper Unicode characters
    let lines: Vec<&str> = result.lines().collect();
    if lines.len() > 2 {
        // Calculate the visual width by stripping ANSI codes from the first line
        let line_width = strip_ansi_codes(lines[0]).len();
        let separator = "─".repeat(line_width); // Use Unicode box-drawing character

        let mut new_result = String::new();
        // Add header
        new_result.push_str(lines[0]);
        new_result.push('\n');
        // Add separator after header
        new_result.push_str(&separator);
        new_result.push('\n');

        // Add all data rows except the last
        for i in 1..lines.len() - 1 {
            new_result.push_str(lines[i]);
            new_result.push('\n');
        }

        // Add separator before totals
        new_result.push_str(&separator);
        new_result.push('\n');
        // Add totals row
        new_result.push_str(lines[lines.len() - 1]);

        result = new_result;
    }

    result
}
