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
    fn to_table(&self) -> String;
    fn to_json(&self) -> Result<String, serde_json::Error>;
    fn to_table_with_currency(&self, currency: &str, decimal_places: u8) -> String;
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
    pub fn from_project_usage(usage: &ProjectUsage) -> Self {
        Self {
            project: usage.project_name.clone(),
            input_tokens: format_number(usage.total_input_tokens),
            output_tokens: format_number(usage.total_output_tokens),
            cache_creation: format_number(usage.total_cache_creation_tokens),
            cache_read: format_number(usage.total_cache_read_tokens),
            messages: format_number(usage.message_count),
            total_cost: format_currency_simple(usage.total_cost_usd),
        }
    }

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
    pub fn from_model_usage(usage: &ModelUsage) -> Self {
        Self {
            model: usage.model_name.clone(),
            input_tokens: format_number(usage.input_tokens),
            output_tokens: format_number(usage.output_tokens),
            cache_creation: format_number(usage.cache_creation_tokens),
            cache_read: format_number(usage.cache_read_tokens),
            messages: format_number(usage.message_count),
            cost: format_currency_simple(usage.cost_usd),
        }
    }

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
    pub fn from_project_summary(summary: &ProjectSummary) -> Self {
        let total_tokens = summary.total_input_tokens + summary.total_output_tokens;
        Self {
            project: summary.project_name.clone(),
            total_tokens: format_number(total_tokens),
            messages: format_number(summary.message_count),
            models: summary.model_count.to_string(),
            total_cost: format_currency_simple(summary.total_cost_usd),
        }
    }

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
    fn to_table(&self) -> String {
        if self.is_empty() {
            return "No usage data found.".to_string();
        }

        let mut rows: Vec<ProjectUsageRow> = self
            .iter()
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
            total_cost: format_currency_simple(total_cost),
        });

        apply_table_style_with_color(Table::new(rows), false, TableType::ProjectUsage)
    }

    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    fn to_table_with_currency(&self, currency: &str, decimal_places: u8) -> String {
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

        apply_table_style_with_color(Table::new(rows), false, TableType::ProjectUsage)
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
    fn to_table(&self) -> String {
        if self.is_empty() {
            return "No model usage data found.".to_string();
        }

        let mut rows: Vec<ModelUsageRow> =
            self.iter().map(ModelUsageRow::from_model_usage).collect();

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
            cost: format_currency_simple(total_cost),
        });

        apply_table_style_with_color(Table::new(rows), false, TableType::ModelUsage)
    }

    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    fn to_table_with_currency(&self, currency: &str, decimal_places: u8) -> String {
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

        apply_table_style_with_color(Table::new(rows), false, TableType::ModelUsage)
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
    fn to_table(&self) -> String {
        if self.is_empty() {
            return "No project data found.".to_string();
        }

        let mut rows: Vec<ProjectSummaryRow> = self
            .iter()
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
            total_cost: format_currency_simple(total_cost),
        });

        apply_table_style_with_color(Table::new(rows), false, TableType::ProjectSummary)
    }

    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    fn to_table_with_currency(&self, currency: &str, decimal_places: u8) -> String {
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

        apply_table_style_with_color(Table::new(rows), false, TableType::ProjectSummary)
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

/// Format currency value with proper decimal places and dollar sign (for internal use)
fn format_currency_simple(amount: f64) -> String {
    if amount == 0.0 {
        return "$0.00".to_string();
    }

    format!("${:.2}", amount)
}

/// Format currency value with proper decimal places and currency symbol (for external use)
pub fn format_currency(amount: f64, currency: &str, decimal_places: u8) -> String {
    use crate::models::currency;
    currency::format_currency(amount, currency, decimal_places)
}

/// Apply modern table styling similar to the design reference  
fn apply_table_style(table: Table) -> String {
    apply_table_style_with_color(table, false, TableType::ProjectUsage)
}

/// Table type enum for proper column coloring
#[derive(Debug, Clone)]
pub enum TableType {
    ProjectUsage, // Project, Input Tokens, Output Tokens, Cache Creation, Cache Read, Messages, Total Cost
    ModelUsage,   // Model, Input Tokens, Output Tokens, Cache Creation, Cache Read, Messages, Cost
    ProjectSummary, // Project, Total Tokens, Messages, Models, Total Cost
    DailyUsage, // Date, Input Tokens, Output Tokens, Cache Creation, Cache Read, Messages, Projects, Total Cost
    Conversations, // Conversation ID, Project, Messages, Total Cost, Efficiency, Models, Outliers, Duration
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
    fn test_format_currency_simple() {
        assert_eq!(format_currency_simple(0.0), "$0.00");
        assert_eq!(format_currency_simple(1.5), "$1.50");
        assert_eq!(format_currency_simple(123.456), "$123.46");
        assert_eq!(format_currency_simple(1000.0), "$1000.00");
    }

    #[test]
    fn test_project_usage_row_creation() {
        let mut model_usage = HashMap::new();
        model_usage.insert(
            "claude-3-5-sonnet".to_string(),
            ModelUsage {
                model_name: "claude-3-5-sonnet".to_string(),
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_tokens: 100,
                cache_read_tokens: 50,
                cost_usd: 2.5,
                message_count: 5,
            },
        );

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
