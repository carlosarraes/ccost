// Shared utilities for timeframe-based commands
use crate::analysis::{
    CostCalculationMode, TimezoneCalculator, UsageFilter, UsageTracker, usage::ProjectUsage,
};
use crate::config::Config;
use crate::models::PricingManager;
use crate::models::currency::CurrencyConverter;
use crate::output::OutputFormat;
use crate::parser::deduplication::DeduplicationEngine;
use crate::parser::jsonl::JsonlParser;
use crate::utils::{DateFormatter, EnhancedUsageData, maybe_hide_project_name};
use std::path::PathBuf;

// Re-export the UsageTimeframe from usage.rs to avoid duplication
pub use crate::commands::usage::UsageTimeframe;

/// Common configuration and setup for timeframe commands
pub struct TimeframeContext {
    pub pricing_manager: PricingManager,
    pub usage_tracker: UsageTracker,
    pub parser: JsonlParser,
    pub dedup_engine: DeduplicationEngine,
    pub date_formatter: DateFormatter,
    pub timezone_calc: TimezoneCalculator,
    pub projects_dir: PathBuf,
}

impl TimeframeContext {
    /// Create a new timeframe context with all necessary components
    pub async fn new(
        timezone_name: &str,
        daily_cutoff_hour: u8,
        date_format: &str,
    ) -> anyhow::Result<Self> {
        // Load config
        let config =
            Config::load().map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;

        // Initialize date formatter
        let date_formatter = DateFormatter::new(date_format)?;

        // Initialize timezone calculator
        let timezone_calc = TimezoneCalculator::new(timezone_name, daily_cutoff_hour)?;

        // Setup projects directory
        let projects_dir = if config.general.claude_projects_path.starts_with("~/") {
            if let Some(home_dir) = dirs::home_dir() {
                home_dir.join(&config.general.claude_projects_path[2..])
            } else {
                PathBuf::from(&config.general.claude_projects_path)
            }
        } else {
            PathBuf::from(&config.general.claude_projects_path)
        };

        // Initialize pricing manager based on configuration
        // Default to static pricing for speed, only use live when explicitly requested
        let mut pricing_manager = match config.pricing.source.as_str() {
            "live" => PricingManager::with_live_pricing(),
            _ => PricingManager::new(), // "auto", "static" or unknown - all use static by default
        };

        // Only enable live pricing when explicitly set to "live"
        pricing_manager.set_live_pricing(config.pricing.source == "live");

        // Pre-fetch pricing data if live pricing is enabled
        if let Err(_) = pricing_manager.initialize_live_pricing().await {
            // If live pricing fails, it will fall back to static during calculations
        }

        let usage_tracker = UsageTracker::new(CostCalculationMode::Auto);
        let parser = JsonlParser::new(projects_dir.clone());
        let dedup_engine = DeduplicationEngine::new();

        Ok(Self {
            pricing_manager,
            usage_tracker,
            parser,
            dedup_engine,
            date_formatter,
            timezone_calc,
            projects_dir,
        })
    }

    /// Process JSONL files and return enhanced usage data
    pub fn process_jsonl_files(
        &mut self,
        project: Option<String>,
        verbose: bool,
        json_output: bool,
        hidden: bool,
    ) -> anyhow::Result<Vec<EnhancedUsageData>> {
        if verbose && !json_output {
            println!(
                "Searching for JSONL files in: {}",
                self.projects_dir.display()
            );
        }

        let jsonl_files = self
            .parser
            .find_jsonl_files()
            .map_err(|e| anyhow::anyhow!("Failed to find JSONL files: {}", e))?;

        if jsonl_files.is_empty() {
            if json_output {
                println!(
                    r#"{{"status": "warning", "message": "No JSONL files found", "data": []}}"#
                );
            } else {
                println!(
                    "No Claude usage data found in {}",
                    self.projects_dir.display()
                );
                println!("Make sure you have conversations saved in Claude Desktop or CLI.");
            }
            return Ok(Vec::new());
        }

        if verbose && !json_output {
            println!("Found {} JSONL files", jsonl_files.len());
        }

        // Parse all files with deduplication
        let mut all_usage_data = Vec::new();
        let mut files_processed = 0;
        let mut total_messages = 0;
        let mut unique_messages = 0;

        for file_path in jsonl_files {
            match self.parser.parse_file_with_verbose(&file_path, verbose) {
                Ok(parsed_conversation) => {
                    // Use unified project name extraction for consistency
                    let raw_project_name = self
                        .parser
                        .get_unified_project_name(&file_path, &parsed_conversation.messages);
                    let project_name = maybe_hide_project_name(&raw_project_name, hidden);

                    // Apply project filter if specified
                    if let Some(ref filter_project) = project {
                        if raw_project_name != *filter_project {
                            continue;
                        }
                    }

                    total_messages += parsed_conversation.messages.len();

                    // Apply deduplication
                    match self
                        .dedup_engine
                        .filter_duplicates(parsed_conversation.messages, &project_name)
                    {
                        Ok(unique_data) => {
                            unique_messages += unique_data.len();

                            // Create enhanced usage data with project name
                            for data in unique_data {
                                let enhanced_data = EnhancedUsageData {
                                    usage_data: data,
                                    project_name: project_name.clone(),
                                };
                                all_usage_data.push(enhanced_data);
                            }
                        }
                        Err(e) => {
                            if verbose {
                                let error_msg = format!(
                                    "Failed to deduplicate file {}: {}",
                                    file_path.display(),
                                    e
                                );
                                if json_output {
                                    eprintln!(
                                        r#"{{"status": "warning", "message": "{}"}}"#,
                                        error_msg
                                    );
                                } else {
                                    eprintln!("Warning: {}", error_msg);
                                }
                            }
                            continue;
                        }
                    }

                    files_processed += 1;
                }
                Err(e) => {
                    if verbose {
                        let error_msg =
                            format!("Failed to parse file {}: {}", file_path.display(), e);
                        if json_output {
                            eprintln!(r#"{{"status": "warning", "message": "{}"}}"#, error_msg);
                        } else {
                            eprintln!("Warning: {}", error_msg);
                        }
                    }
                }
            }
        }

        if verbose && !json_output {
            println!(
                "Processed {files_processed} files, {total_messages} total messages, {unique_messages} unique messages"
            );
        }

        Ok(all_usage_data)
    }

    /// Apply currency conversion to usage data
    pub async fn convert_currency(
        &self,
        usage: &mut Vec<crate::analysis::usage::ProjectUsage>,
        target_currency: &str,
        verbose: bool,
        json_output: bool,
    ) -> anyhow::Result<()> {
        if target_currency == "USD" {
            return Ok(());
        }

        let currency_converter = CurrencyConverter::new();

        // Convert all USD amounts to target currency
        for project in usage.iter_mut() {
            match currency_converter
                .convert_from_usd(project.total_cost_usd, target_currency)
                .await
            {
                Ok(converted_cost) => {
                    project.total_cost_usd = converted_cost; // Reusing the USD field for converted amount
                }
                Err(e) => {
                    if verbose {
                        let error_msg = format!(
                            "Failed to convert currency for {}: {}",
                            project.project_name, e
                        );
                        if json_output {
                            eprintln!(r#"{{"status": "warning", "message": "{}"}}"#, error_msg);
                        } else {
                            eprintln!("Warning: {}", error_msg);
                        }
                    }
                    // Keep USD amounts if conversion fails
                }
            }

            // Convert model-level costs too
            for model_usage in project.model_usage.values_mut() {
                match currency_converter
                    .convert_from_usd(model_usage.cost_usd, target_currency)
                    .await
                {
                    Ok(converted_cost) => {
                        model_usage.cost_usd = converted_cost;
                    }
                    Err(_) => {
                        // Keep USD amount if conversion fails
                    }
                }
            }
        }

        Ok(())
    }

    /// Display results in the appropriate format
    pub fn display_results(
        &self,
        usage: &Vec<crate::analysis::usage::ProjectUsage>,
        target_currency: &str,
        decimal_places: u8,
        json_output: bool,
        colored: bool,
    ) -> anyhow::Result<()> {
        if usage.is_empty() {
            if json_output {
                println!(
                    r#"{{"status": "success", "message": "No usage data found matching filters", "data": []}}"#
                );
            } else {
                println!("No usage data found matching your filters.");
            }
            return Ok(());
        }

        if json_output {
            match usage.to_json() {
                Ok(json) => println!("{json}"),
                Err(e) => {
                    println!(
                        r#"{{"status": "error", "message": "Failed to serialize results: {e}"}}"#
                    );
                    std::process::exit(1);
                }
            }
        } else {
            println!(
                "{}",
                usage.to_table_with_currency_and_color(target_currency, decimal_places, colored)
            );
        }

        Ok(())
    }

    /// Calculate usage with enhanced pricing that supports live pricing data
    pub async fn calculate_usage_enhanced(
        &mut self,
        usage_tuples: Vec<(crate::parser::jsonl::UsageData, String)>,
        usage_filter: &UsageFilter,
    ) -> anyhow::Result<(Vec<ProjectUsage>, Option<String>)> {
        self.usage_tracker
            .calculate_usage_with_projects_filtered_enhanced(
                usage_tuples,
                &mut self.pricing_manager,
                usage_filter,
            )
            .await
    }
}

/// Handle error display consistently across all commands
pub fn handle_error(error: &anyhow::Error, json_output: bool) {
    if json_output {
        println!(r#"{{"status": "error", "message": "{}"}}"#, error);
    } else {
        eprintln!("Error: {}", error);
    }
    std::process::exit(1);
}
