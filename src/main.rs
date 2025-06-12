// ccost: Claude Cost Tracking Tool
//
use analysis::{
    ConversationAnalyzer, ConversationFilter, ConversationInsight, ConversationInsightList,
    ConversationSortBy, CostCalculationMode, OptimizationEngine, ProjectAnalyzer, ProjectSortBy,
    UsageFilter, UsageTracker,
};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use config::Config;
use models::PricingManager;
use models::currency::CurrencyConverter;
use output::OutputFormat;
use output::table::format_number;
use parser::deduplication::DeduplicationEngine;
use parser::jsonl::JsonlParser;
use std::path::PathBuf;
use clap::Parser;

// Module declarations
mod analysis;
mod cli;
mod commands;
mod config;
mod core;
mod models;
mod output;
mod parser;
mod utils;
mod watch;

// Import CLI types and commands
use cli::args::{Cli, Commands, UsageTimeframe, ProjectSort, ConversationSort, ConfigAction};
use utils::EnhancedUsageData;
use commands::usage::{handle_usage_command, handle_daily_usage_command};


fn handle_config_action(action: ConfigAction, json_output: bool) {
    match action {
        ConfigAction::Init => match Config::default().save() {
            Ok(()) => {
                if json_output {
                    println!(
                        r#"{{"status": "success", "message": "Configuration initialized successfully"}}"#
                    );
                } else {
                    if let Ok(config_path) = Config::default_path() {
                        println!("Configuration initialized at: {}", config_path.display());
                    } else {
                        println!("Configuration initialized successfully");
                    }
                }
            }
            Err(e) => {
                if json_output {
                    println!(
                        r#"{{"status": "error", "message": "Failed to initialize config: {}"}}"#,
                        e
                    );
                } else {
                    eprintln!("Error: Failed to initialize config: {}", e);
                }
                std::process::exit(1);
            }
        },
        ConfigAction::Show => match Config::load() {
            Ok(config) => {
                if json_output {
                    match serde_json::to_string_pretty(&config) {
                        Ok(json) => println!("{}", json),
                        Err(e) => {
                            eprintln!("Error: Failed to serialize config to JSON: {}", e);
                            std::process::exit(1);
                        }
                    }
                } else {
                    match toml::to_string_pretty(&config) {
                        Ok(toml_str) => {
                            if let Ok(config_path) = Config::default_path() {
                                println!("Configuration ({})", config_path.display());
                                println!("{}", toml_str);
                            } else {
                                println!("Configuration:");
                                println!("{}", toml_str);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: Failed to serialize config: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }
            Err(e) => {
                if json_output {
                    println!(
                        r#"{{"status": "error", "message": "Failed to load config: {}"}}"#,
                        e
                    );
                } else {
                    eprintln!("Error: Failed to load config: {}", e);
                }
                std::process::exit(1);
            }
        },
        ConfigAction::Set { key, value } => match Config::load() {
            Ok(mut config) => match config.set_value(&key, &value) {
                Ok(()) => match config.save() {
                    Ok(()) => {
                        if json_output {
                            println!(
                                r#"{{"status": "success", "message": "Configuration updated: {} = {}"}}"#,
                                key, value
                            );
                        } else {
                            println!("Configuration updated: {} = {}", key, value);
                        }
                    }
                    Err(e) => {
                        if json_output {
                            println!(
                                r#"{{"status": "error", "message": "Failed to save config: {}"}}"#,
                                e
                            );
                        } else {
                            eprintln!("Error: Failed to save config: {}", e);
                        }
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    if json_output {
                        println!(
                            r#"{{"status": "error", "message": "Invalid configuration: {}"}}"#,
                            e
                        );
                    } else {
                        eprintln!("Error: Invalid configuration: {}", e);
                    }
                    std::process::exit(1);
                }
            },
            Err(e) => {
                if json_output {
                    println!(
                        r#"{{"status": "error", "message": "Failed to load config: {}"}}"#,
                        e
                    );
                } else {
                    eprintln!("Error: Failed to load config: {}", e);
                }
                std::process::exit(1);
            }
        },
    }
}


async fn handle_watch_command(
    project_filter: Option<String>,
    expensive_threshold: f64,
    _no_charts: bool,
    refresh_rate_ms: u64,
    cli: &Cli,
) -> anyhow::Result<()> {
    use watch::WatchMode;

    // Load configuration with CLI overrides
    let mut config = Config::load()?;

    // Apply CLI overrides
    if let Some(ref currency) = cli.currency {
        config.currency.default_currency = currency.clone();
    }
    if let Some(ref timezone) = cli.timezone {
        config.timezone.timezone = timezone.clone();
    }

    // Create and start watch mode
    let mut watch_mode =
        WatchMode::new(config, project_filter, expensive_threshold, refresh_rate_ms)?;

    // Start watching
    watch_mode.run().await?;

    Ok(())
}


fn resolve_filters(
    timeframe: Option<UsageTimeframe>,
    project: Option<String>,
    since: Option<String>,
    until: Option<String>,
    model: Option<String>,
    timezone_calc: &analysis::TimezoneCalculator,
) -> (
    Option<String>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    Option<String>,
) {
    let (tf_project, tf_model, tf_since, tf_until) = match timeframe {
        Some(UsageTimeframe::Today {
            project: tf_project,
            model: tf_model,
        }) => {
            let start = timezone_calc.today_start();
            let end = timezone_calc.today_end();
            (tf_project, tf_model, Some(start), Some(end))
        }
        Some(UsageTimeframe::Yesterday {
            project: tf_project,
            model: tf_model,
        }) => {
            let start = timezone_calc.yesterday_start();
            let end = timezone_calc.yesterday_end();
            (tf_project, tf_model, Some(start), Some(end))
        }
        Some(UsageTimeframe::ThisWeek {
            project: tf_project,
            model: tf_model,
        }) => {
            let start = timezone_calc.this_week_start();
            (tf_project, tf_model, Some(start), None)
        }
        Some(UsageTimeframe::ThisMonth {
            project: tf_project,
            model: tf_model,
        }) => {
            let start = timezone_calc.this_month_start();
            (tf_project, tf_model, Some(start), None)
        }
        Some(UsageTimeframe::Daily {
            project: tf_project,
            model: tf_model,
            days,
        }) => {
            let today = Utc::now().date_naive();
            let days_ago = today - chrono::Duration::days(days as i64 - 1); // Include today
            let start = match days_ago.and_hms_opt(0, 0, 0) {
                Some(naive_dt) => Utc.from_utc_datetime(&naive_dt),
                None => {
                    eprintln!("Warning: Failed to create start datetime for {} days ago", days);
                    Utc::now() // Fallback to current time
                }
            };
            (tf_project, tf_model, Some(start), None)
        }
        None => (None, None, None, None),
    };

    // Merge timeframe filters with explicit filters
    let final_project = tf_project.or(project);
    let final_model = tf_model.or(model);

    // Parse explicit date filters
    let final_since = tf_since.or_else(|| {
        since.and_then(|s| {
            NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                .ok()
                .and_then(|date| date.and_hms_opt(0, 0, 0)
                    .map(|naive_dt| Utc.from_utc_datetime(&naive_dt)))
        })
    });

    let final_until = tf_until.or_else(|| {
        until.and_then(|s| {
            NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                .ok()
                .and_then(|date| date.and_hms_opt(23, 59, 59)
                    .map(|naive_dt| Utc.from_utc_datetime(&naive_dt)))
        })
    });

    (final_project, final_since, final_until, final_model)
}

fn print_filter_info(filter: &UsageFilter, json_output: bool) {
    if json_output {
        return; // Skip verbose info in JSON mode
    }

    println!("Filters applied:");
    if let Some(ref project) = filter.project_name {
        println!("  Project: {}", project);
    }
    if let Some(ref model) = filter.model_name {
        println!("  Model: {}", model);
    }
    if let Some(ref since) = filter.since {
        println!("  Since: {}", since.format("%Y-%m-%d %H:%M"));
    }
    if let Some(ref until) = filter.until {
        println!("  Until: {}", until.format("%Y-%m-%d %H:%M"));
    }
    println!();
}

fn apply_usage_filters(
    usage: Vec<analysis::usage::ProjectUsage>,
    filter: &UsageFilter,
) -> Vec<analysis::usage::ProjectUsage> {
    usage
        .into_iter()
        .filter(|project| {
            // Project filter already applied during parsing
            if let Some(ref model_filter) = filter.model_name {
                // If model filter specified, only include projects that have usage for that model
                project.model_usage.contains_key(model_filter)
            } else {
                true
            }
        })
        .map(|mut project| {
            // If model filter is specified, filter model usage within each project
            if let Some(ref model_filter) = filter.model_name {
                let filtered_model_usage: std::collections::HashMap<
                    String,
                    analysis::usage::ModelUsage,
                > = project
                    .model_usage
                    .into_iter()
                    .filter(|(model_name, _)| model_name == model_filter)
                    .collect();

                // Recalculate project totals based on filtered models
                let total_input_tokens =
                    filtered_model_usage.values().map(|m| m.input_tokens).sum();
                let total_output_tokens =
                    filtered_model_usage.values().map(|m| m.output_tokens).sum();
                let total_cache_creation_tokens = filtered_model_usage
                    .values()
                    .map(|m| m.cache_creation_tokens)
                    .sum();
                let total_cache_read_tokens = filtered_model_usage
                    .values()
                    .map(|m| m.cache_read_tokens)
                    .sum();
                let total_cost_usd = filtered_model_usage.values().map(|m| m.cost_usd).sum();
                let message_count = filtered_model_usage.values().map(|m| m.message_count).sum();

                project.model_usage = filtered_model_usage;
                project.total_input_tokens = total_input_tokens;
                project.total_output_tokens = total_output_tokens;
                project.total_cache_creation_tokens = total_cache_creation_tokens;
                project.total_cache_read_tokens = total_cache_read_tokens;
                project.total_cost_usd = total_cost_usd;
                project.message_count = message_count;
            }

            project
        })
        .filter(|project| {
            // Remove projects with no usage after model filtering
            project.message_count > 0
        })
        .collect()
}

async fn handle_projects_command(
    sort_by: Option<ProjectSort>,
    target_currency: &str,
    decimal_places: u8,
    json_output: bool,
    verbose: bool,
    colored: bool,
) -> anyhow::Result<()> {

    // Find and parse JSONL files - use config setting
    let config_for_projects = match Config::load() {
        Ok(config) => config,
        Err(_) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Failed to load config for projects path"}}"#
                );
            } else {
                eprintln!("Error: Failed to load config for projects path");
            }
            std::process::exit(1);
        }
    };

    let projects_dir = if config_for_projects
        .general
        .claude_projects_path
        .starts_with("~/")
    {
        // Expand tilde to home directory
        if let Some(home_dir) = dirs::home_dir() {
            home_dir.join(&config_for_projects.general.claude_projects_path[2..])
        } else {
            PathBuf::from(&config_for_projects.general.claude_projects_path)
        }
    } else {
        PathBuf::from(&config_for_projects.general.claude_projects_path)
    };

    let pricing_manager = PricingManager::new();
    let usage_tracker = UsageTracker::new(CostCalculationMode::Auto);
    let parser = JsonlParser::new(projects_dir.clone());
    let mut dedup_engine = DeduplicationEngine::new();
    let project_analyzer = ProjectAnalyzer::new();

    if verbose && !json_output {
        println!("Searching for JSONL files in: {}", projects_dir.display());
    }

    let jsonl_files = match parser.find_jsonl_files() {
        Ok(files) => files,
        Err(e) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Failed to find JSONL files: {}"}}"#,
                    e
                );
            } else {
                eprintln!("Error: Failed to find JSONL files: {}", e);
                eprintln!(
                    "Make sure you have Claude conversations in: {}",
                    projects_dir.display()
                );
            }
            std::process::exit(1);
        }
    };

    if jsonl_files.is_empty() {
        if json_output {
            println!(r#"{{"status": "warning", "message": "No JSONL files found", "data": []}}"#);
        } else {
            println!("No Claude usage data found in {}", projects_dir.display());
            println!("Make sure you have conversations saved in Claude Desktop or CLI.");
        }
        return Ok(());
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
        match parser.parse_file_with_verbose(&file_path, verbose) {
            Ok(parsed_conversation) => {
                // Use unified project name extraction for consistency
                let project_name =
                    parser.get_unified_project_name(&file_path, &parsed_conversation.messages);
                total_messages += parsed_conversation.messages.len();

                // Apply deduplication
                match dedup_engine.filter_duplicates(parsed_conversation.messages, &project_name) {
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
                            if json_output {
                                eprintln!(
                                    r#"{{"status": "warning", "message": "Failed to deduplicate file {}: {}"}}"#,
                                    file_path.display(),
                                    e
                                );
                            } else {
                                eprintln!(
                                    "Warning: Failed to deduplicate file {}: {}",
                                    file_path.display(),
                                    e
                                );
                            }
                        }
                        continue;
                    }
                }

                files_processed += 1;
            }
            Err(e) => {
                if verbose {
                    if json_output {
                        eprintln!(
                            r#"{{"status": "warning", "message": "Failed to parse file {}: {}"}}"#,
                            file_path.display(),
                            e
                        );
                    } else {
                        eprintln!(
                            "Warning: Failed to parse file {}: {}",
                            file_path.display(),
                            e
                        );
                    }
                }
            }
        }
    }

    if verbose && !json_output {
        println!(
            "Processed {} files, {} total messages, {} unique messages",
            files_processed, total_messages, unique_messages
        );
    }

    if all_usage_data.is_empty() {
        if json_output {
            println!(r#"{{"status": "success", "message": "No usage data found", "data": []}}"#);
        } else {
            println!("No usage data found in your Claude projects.");
        }
        return Ok(());
    }

    // Convert enhanced data to tuple format
    let usage_tuples: Vec<(parser::jsonl::UsageData, String)> = all_usage_data
        .into_iter()
        .map(|enhanced| (enhanced.usage_data, enhanced.project_name))
        .collect();

    // Calculate usage with the tracker
    let project_usage =
        match usage_tracker.calculate_usage_with_projects(usage_tuples, &pricing_manager) {
            Ok(usage) => usage,
            Err(e) => {
                if json_output {
                    println!(
                        r#"{{"status": "error", "message": "Failed to calculate usage: {}"}}"#,
                        e
                    );
                } else {
                    eprintln!("Error: Failed to calculate usage: {}", e);
                }
                std::process::exit(1);
            }
        };

    if project_usage.is_empty() {
        if json_output {
            println!(r#"{{"status": "success", "message": "No project usage found", "data": []}}"#);
        } else {
            println!("No project usage data found.");
        }
        return Ok(());
    }

    // Determine sort method
    let sort_method = match sort_by {
        Some(ProjectSort::Cost) => ProjectSortBy::Cost,
        Some(ProjectSort::Tokens) => ProjectSortBy::Tokens,
        Some(ProjectSort::Name) => ProjectSortBy::Name,
        None => ProjectSortBy::Name,
    };

    // Analyze and sort projects
    let mut project_summaries = project_analyzer.analyze_projects(project_usage, sort_method);

    // Convert currencies if needed
    if target_currency != "USD" {
        let currency_converter = CurrencyConverter::new();

        // Convert all USD amounts to target currency
        for summary in &mut project_summaries {
                match currency_converter
                    .convert_from_usd(summary.total_cost_usd, target_currency)
                    .await
                {
                    Ok(converted_cost) => {
                        summary.total_cost_usd = converted_cost; // Reusing the USD field for converted amount
                    }
                    Err(e) => {
                        if verbose {
                            if json_output {
                                eprintln!(
                                    r#"{{"status": "warning", "message": "Failed to convert currency for {}: {}"}}"#,
                                    summary.project_name, e
                                );
                            } else {
                                eprintln!(
                                    "Warning: Failed to convert currency for {}: {}",
                                    summary.project_name, e
                                );
                            }
                        }
                        // Keep USD amounts if conversion fails
                    }
                }
            }
    }

    // Get statistics
    let stats = project_analyzer.get_project_statistics(&project_summaries);

    // Display results
    if json_output {
        let json_output = serde_json::json!({
            "projects": project_summaries,
            "statistics": stats
        });
        match serde_json::to_string_pretty(&json_output) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                println!(
                    r#"{{"status": "error", "message": "Failed to serialize results: {}"}}"#,
                    e
                );
                std::process::exit(1);
            }
        }
    } else {
        println!(
            "{}",
            project_summaries.to_table_with_currency_and_color(
                target_currency,
                decimal_places,
                colored
            )
        );

        // Show summary stats
        println!();
        println!("Summary:");
        println!("  Total Projects: {}", stats.total_projects);
        println!(
            "  Total Input Tokens: {}",
            format_number(stats.total_input_tokens)
        );
        println!(
            "  Total Output Tokens: {}",
            format_number(stats.total_output_tokens)
        );
        println!("  Total Messages: {}", format_number(stats.total_messages));
        println!(
            "  Total Cost: {}",
            models::currency::format_currency(stats.total_cost, target_currency, decimal_places)
        );

        if let Some(ref highest_cost) = stats.highest_cost_project {
            println!("  Highest Cost Project: {}", highest_cost);
        }

        if let Some(ref most_active) = stats.most_active_project {
            println!("  Most Active Project: {}", most_active);
        }
    }
    Ok(())
}


async fn handle_conversations_command(
    sort_by: Option<ConversationSort>,
    project: Option<String>,
    since: Option<String>,
    until: Option<String>,
    model: Option<String>,
    min_cost: Option<f64>,
    max_cost: Option<f64>,
    outliers_only: bool,
    min_efficiency: Option<f32>,
    max_efficiency: Option<f32>,
    export: Option<String>,
    target_currency: &str,
    decimal_places: u8,
    json_output: bool,
    verbose: bool,
    colored: bool,
) -> anyhow::Result<()> {

    // Find and parse JSONL files - use config setting
    let config_for_projects = match Config::load() {
        Ok(config) => config,
        Err(_) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Failed to load config for projects path"}}"#
                );
            } else {
                eprintln!("Error: Failed to load config for projects path");
            }
            std::process::exit(1);
        }
    };

    let projects_dir = if config_for_projects
        .general
        .claude_projects_path
        .starts_with("~/")
    {
        // Expand tilde to home directory
        if let Some(home_dir) = dirs::home_dir() {
            home_dir.join(&config_for_projects.general.claude_projects_path[2..])
        } else {
            PathBuf::from(&config_for_projects.general.claude_projects_path)
        }
    } else {
        PathBuf::from(&config_for_projects.general.claude_projects_path)
    };

    let _pricing_manager = PricingManager::new();
    let _usage_tracker = UsageTracker::new(CostCalculationMode::Auto);
    let conversation_analyzer = ConversationAnalyzer::new();
    let parser = JsonlParser::new(projects_dir.clone());
    let mut dedup_engine = DeduplicationEngine::new();

    if verbose && !json_output {
        println!("Searching for JSONL files in: {}", projects_dir.display());
    }

    let jsonl_files = match parser.find_jsonl_files() {
        Ok(files) => files,
        Err(e) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Failed to find JSONL files: {}"}}"#,
                    e
                );
            } else {
                eprintln!("Error: Failed to find JSONL files: {}", e);
                eprintln!(
                    "Make sure you have Claude conversations in: {}",
                    projects_dir.display()
                );
            }
            std::process::exit(1);
        }
    };

    if jsonl_files.is_empty() {
        if json_output {
            println!(r#"{{"status": "warning", "message": "No JSONL files found", "data": []}}"#);
        } else {
            println!("No Claude usage data found in {}", projects_dir.display());
            println!("Make sure you have conversations saved in Claude Desktop or CLI.");
        }
        return Ok(());
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
        match parser.parse_file_with_verbose(&file_path, verbose) {
            Ok(parsed_conversation) => {
                // Use unified project name extraction for consistency
                let project_name =
                    parser.get_unified_project_name(&file_path, &parsed_conversation.messages);
                total_messages += parsed_conversation.messages.len();

                match dedup_engine.filter_duplicates(parsed_conversation.messages, &project_name) {
                    Ok(unique_data) => {
                        unique_messages += unique_data.len();

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
                            if json_output {
                                eprintln!(
                                    r#"{{"status": "warning", "message": "Failed to deduplicate file {}: {}"}}"#,
                                    file_path.display(),
                                    e
                                );
                            } else {
                                eprintln!(
                                    "Warning: Failed to deduplicate file {}: {}",
                                    file_path.display(),
                                    e
                                );
                            }
                        }
                        continue;
                    }
                }

                files_processed += 1;
            }
            Err(e) => {
                if verbose {
                    if json_output {
                        eprintln!(
                            r#"{{"status": "warning", "message": "Failed to parse file {}: {}"}}"#,
                            file_path.display(),
                            e
                        );
                    } else {
                        eprintln!(
                            "Warning: Failed to parse file {}: {}",
                            file_path.display(),
                            e
                        );
                    }
                }
            }
        }
    }

    if verbose && !json_output {
        println!(
            "Processed {} files, {} total messages, {} unique messages",
            files_processed, total_messages, unique_messages
        );
    }

    if all_usage_data.is_empty() {
        if json_output {
            println!(r#"{{"status": "success", "message": "No usage data found", "data": []}}"#);
        } else {
            println!("No usage data found in your Claude projects.");
        }
        return Ok(());
    }

    // Convert enhanced data to tuple format
    let usage_tuples: Vec<(parser::jsonl::UsageData, String)> = all_usage_data
        .into_iter()
        .map(|enhanced| (enhanced.usage_data, enhanced.project_name))
        .collect();

    // Group into conversations
    let conversations = match conversation_analyzer.group_into_conversations(usage_tuples) {
        Ok(convs) => convs,
        Err(e) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Failed to group conversations: {}"}}"#,
                    e
                );
            } else {
                eprintln!("Error: Failed to group conversations: {}", e);
            }
            std::process::exit(1);
        }
    };

    if conversations.is_empty() {
        if json_output {
            println!(r#"{{"status": "success", "message": "No conversations found", "data": []}}"#);
        } else {
            println!("No conversations found in your usage data.");
        }
        return Ok(());
    }

    // Analyze conversations
    let mut insights = match conversation_analyzer.analyze_conversations(conversations) {
        Ok(insights) => insights,
        Err(e) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Failed to analyze conversations: {}"}}"#,
                    e
                );
            } else {
                eprintln!("Error: Failed to analyze conversations: {}", e);
            }
            std::process::exit(1);
        }
    };

    // Create filter
    let mut filter = ConversationFilter::default();
    filter.project_name = project;
    filter.model_name = model;
    filter.min_cost = min_cost;
    filter.max_cost = max_cost;
    filter.outliers_only = outliers_only;
    filter.min_efficiency = min_efficiency;
    filter.max_efficiency = max_efficiency;

    // Parse date filters
    if let Some(since_str) = since {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(&since_str, "%Y-%m-%d") {
            if let Some(naive_dt) = date.and_hms_opt(0, 0, 0) {
                filter.since = Some(Utc.from_utc_datetime(&naive_dt));
            } else {
                eprintln!("Warning: Invalid since date format: {}", since_str);
            }
        }
    }

    if let Some(until_str) = until {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(&until_str, "%Y-%m-%d") {
            if let Some(naive_dt) = date.and_hms_opt(23, 59, 59) {
                filter.until = Some(Utc.from_utc_datetime(&naive_dt));
            } else {
                eprintln!("Warning: Invalid until date format: {}", until_str);
            }
        }
    }

    // Apply filters
    insights = conversation_analyzer.filter_conversations(insights, &filter);

    if insights.is_empty() {
        if json_output {
            println!(
                r#"{{"status": "success", "message": "No conversations found matching filters", "data": []}}"#
            );
        } else {
            println!("No conversations found matching your filters.");
        }
        return Ok(());
    }

    // Sort conversations
    let sort_method = match sort_by {
        Some(ConversationSort::Cost) => ConversationSortBy::Cost,
        Some(ConversationSort::Tokens) => ConversationSortBy::Tokens,
        Some(ConversationSort::Efficiency) => ConversationSortBy::Efficiency,
        Some(ConversationSort::Messages) => ConversationSortBy::Messages,
        Some(ConversationSort::Duration) => ConversationSortBy::Duration,
        Some(ConversationSort::StartTime) => ConversationSortBy::StartTime,
        None => ConversationSortBy::Cost, // Default to cost
    };

    insights = conversation_analyzer.sort_conversations(insights, sort_method);

    // Convert currencies if needed
    if target_currency != "USD" {
        let currency_converter = CurrencyConverter::new();

        for insight in &mut insights {
                match currency_converter
                    .convert_from_usd(insight.total_cost, target_currency)
                    .await
                {
                    Ok(converted_cost) => {
                        insight.total_cost = converted_cost;
                    }
                    Err(e) => {
                        if verbose {
                            if json_output {
                                eprintln!(
                                    r#"{{"status": "warning", "message": "Failed to convert currency for {}: {}"}}"#,
                                    insight.conversation_id, e
                                );
                            } else {
                                eprintln!(
                                    "Warning: Failed to convert currency for {}: {}",
                                    insight.conversation_id, e
                                );
                            }
                        }
                    }
                }

                // Convert model-level costs too
                for model_usage in insight.model_usage.values_mut() {
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
    }

    // Handle export if specified
    if let Some(export_format) = export {
        handle_conversation_export(export_format, &insights, json_output);
        return Ok(());
    }

    // Wrap in display wrapper
    let insights_list = ConversationInsightList(insights);

    // Display results
    if json_output {
        match insights_list.to_json() {
            Ok(json) => {
                println!("{}", json);
                return Ok(());
            },
            Err(e) => {
                println!(
                    r#"{{"status": "error", "message": "Failed to serialize results: {}"}}"#,
                    e
                );
                std::process::exit(1);
            }
        }
    } else {
        println!(
            "{}",
            insights_list.to_table_with_currency_and_color(
                target_currency,
                decimal_places,
                colored
            )
        );
        return Ok(());
    }
}

fn handle_conversation_export(
    export_format: String,
    insights: &[ConversationInsight],
    json_output: bool,
) {
    use std::fs::File;
    use std::io::Write;

    match export_format.to_lowercase().as_str() {
        "json" => {
            let output = format!(
                "conversations_{}.json",
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            );
            match serde_json::to_string_pretty(insights) {
                Ok(json) => match File::create(&output) {
                    Ok(mut file) => {
                        if let Err(e) = file.write_all(json.as_bytes()) {
                            if json_output {
                                println!(
                                    r#"{{"status": "error", "message": "Failed to write JSON file: {}"}}"#,
                                    e
                                );
                            } else {
                                eprintln!("Error: Failed to write JSON file: {}", e);
                            }
                            std::process::exit(1);
                        }
                        if json_output {
                            println!(
                                r#"{{"status": "success", "message": "Exported {} conversations to {}", "count": {}}}"#,
                                insights.len(),
                                output,
                                insights.len()
                            );
                        } else {
                            println!("Exported {} conversations to {}", insights.len(), output);
                        }
                    }
                    Err(e) => {
                        if json_output {
                            println!(
                                r#"{{"status": "error", "message": "Failed to create JSON file: {}"}}"#,
                                e
                            );
                        } else {
                            eprintln!("Error: Failed to create JSON file: {}", e);
                        }
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    if json_output {
                        println!(
                            r#"{{"status": "error", "message": "Failed to serialize conversations: {}"}}"#,
                            e
                        );
                    } else {
                        eprintln!("Error: Failed to serialize conversations: {}", e);
                    }
                    std::process::exit(1);
                }
            }
        }
        "csv" => {
            let output = format!(
                "conversations_{}.csv",
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            );
            match File::create(&output) {
                Ok(mut file) => {
                    // Write CSV header
                    let header = "conversation_id,project_name,total_cost,message_count,total_input_tokens,total_output_tokens,efficiency_score,cache_hit_rate,duration_minutes,outlier_count\n";
                    if let Err(e) = file.write_all(header.as_bytes()) {
                        if json_output {
                            println!(
                                r#"{{"status": "error", "message": "Failed to write CSV header: {}"}}"#,
                                e
                            );
                        } else {
                            eprintln!("Error: Failed to write CSV header: {}", e);
                        }
                        std::process::exit(1);
                    }

                    // Write CSV rows
                    for insight in insights {
                        let row = format!(
                            "{},{},{},{},{},{},{:.1},{:.3},{:.1},{}\n",
                            insight.conversation_id,
                            insight.project_name,
                            insight.total_cost,
                            insight.message_count,
                            insight.total_input_tokens,
                            insight.total_output_tokens,
                            insight.efficiency_score,
                            insight.cache_hit_rate,
                            insight.duration_minutes,
                            insight.outlier_flags.len()
                        );
                        if let Err(e) = file.write_all(row.as_bytes()) {
                            if json_output {
                                println!(
                                    r#"{{"status": "error", "message": "Failed to write CSV row: {}"}}"#,
                                    e
                                );
                            } else {
                                eprintln!("Error: Failed to write CSV row: {}", e);
                            }
                            std::process::exit(1);
                        }
                    }

                    if json_output {
                        println!(
                            r#"{{"status": "success", "message": "Exported {} conversations to {}", "count": {}}}"#,
                            insights.len(),
                            output,
                            insights.len()
                        );
                    } else {
                        println!("Exported {} conversations to {}", insights.len(), output);
                    }
                }
                Err(e) => {
                    if json_output {
                        println!(
                            r#"{{"status": "error", "message": "Failed to create CSV file: {}"}}"#,
                            e
                        );
                    } else {
                        eprintln!("Error: Failed to create CSV file: {}", e);
                    }
                    std::process::exit(1);
                }
            }
        }
        _ => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Unsupported export format. Use 'json' or 'csv'"}}"#
                );
            } else {
                eprintln!("Error: Unsupported export format. Use 'json' or 'csv'");
            }
            std::process::exit(1);
        }
    }
}

async fn handle_optimize_command(
    project: Option<String>,
    model: Option<String>,
    since: Option<String>,
    until: Option<String>,
    potential_savings: bool,
    export: Option<String>,
    confidence_threshold: Option<f32>,
    model_from: Option<String>,
    model_to: Option<String>,
    target_currency: &str,
    decimal_places: u8,
    json_output: bool,
    verbose: bool,
    _colored: bool,
) -> anyhow::Result<()> {

    // Find and parse JSONL files - use config setting
    let config_for_projects = match Config::load() {
        Ok(config) => config,
        Err(_) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Failed to load config for projects path"}}"#
                );
            } else {
                eprintln!("Error: Failed to load config for projects path");
            }
            std::process::exit(1);
        }
    };

    let projects_dir = if config_for_projects
        .general
        .claude_projects_path
        .starts_with("~/")
    {
        // Expand tilde to home directory
        if let Some(home_dir) = dirs::home_dir() {
            home_dir.join(&config_for_projects.general.claude_projects_path[2..])
        } else {
            PathBuf::from(&config_for_projects.general.claude_projects_path)
        }
    } else {
        PathBuf::from(&config_for_projects.general.claude_projects_path)
    };

    let pricing_manager = PricingManager::new();
    let usage_tracker = UsageTracker::new(CostCalculationMode::Auto);
    let parser = JsonlParser::new(projects_dir.clone());
    let mut dedup_engine = DeduplicationEngine::new();
    let optimization_engine = OptimizationEngine::new(pricing_manager);

    if verbose && !json_output {
        println!("Searching for JSONL files in: {}", projects_dir.display());
    }

    let jsonl_files = match parser.find_jsonl_files() {
        Ok(files) => files,
        Err(e) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Failed to find JSONL files: {}"}}"#,
                    e
                );
            } else {
                eprintln!("Error: Failed to find JSONL files: {}", e);
                eprintln!(
                    "Make sure you have Claude conversations in: {}",
                    projects_dir.display()
                );
            }
            std::process::exit(1);
        }
    };

    if jsonl_files.is_empty() {
        if json_output {
            println!(r#"{{"status": "warning", "message": "No JSONL files found", "data": []}}"#);
        } else {
            println!("No Claude usage data found in {}", projects_dir.display());
            println!("Make sure you have conversations saved in Claude Desktop or CLI.");
        }
        return Ok(());
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
        match parser.parse_file_with_verbose(&file_path, verbose) {
            Ok(parsed_conversation) => {
                // Use unified project name extraction for consistency
                let project_name =
                    parser.get_unified_project_name(&file_path, &parsed_conversation.messages);

                // Apply project filter if specified
                if let Some(ref filter_project) = project {
                    if project_name != *filter_project {
                        continue;
                    }
                }
                total_messages += parsed_conversation.messages.len();

                // Apply deduplication
                match dedup_engine.filter_duplicates(parsed_conversation.messages, &project_name) {
                    Ok(unique_data) => {
                        unique_messages += unique_data.len();

                        // Create enhanced usage data with project name
                        for data in unique_data {
                            // Apply date filters if specified
                            if let Some(timestamp_str) = &data.timestamp {
                                if let Ok(message_time) =
                                    usage_tracker.parse_timestamp(timestamp_str)
                                {
                                    // Check since filter
                                    if let Some(ref since_str) = since {
                                        if let Ok(since_date) =
                                            chrono::NaiveDate::parse_from_str(since_str, "%Y-%m-%d")
                                        {
                                            let since_datetime = match since_date.and_hms_opt(0, 0, 0) {
                                                Some(naive_dt) => chrono::Utc.from_utc_datetime(&naive_dt),
                                                None => continue, // Skip malformed date
                                            };
                                            if message_time < since_datetime {
                                                continue;
                                            }
                                        }
                                    }

                                    // Check until filter
                                    if let Some(ref until_str) = until {
                                        if let Ok(until_date) =
                                            chrono::NaiveDate::parse_from_str(until_str, "%Y-%m-%d")
                                        {
                                            let until_datetime = match until_date.and_hms_opt(23, 59, 59) {
                                                Some(naive_dt) => chrono::Utc.from_utc_datetime(&naive_dt),
                                                None => continue, // Skip malformed date
                                            };
                                            if message_time > until_datetime {
                                                continue;
                                            }
                                        }
                                    }
                                }
                            }

                            all_usage_data.push((data, project_name.clone()));
                        }
                    }
                    Err(e) => {
                        if verbose {
                            if json_output {
                                eprintln!(
                                    r#"{{"status": "warning", "message": "Failed to deduplicate file {}: {}"}}"#,
                                    file_path.display(),
                                    e
                                );
                            } else {
                                eprintln!(
                                    "Warning: Failed to deduplicate file {}: {}",
                                    file_path.display(),
                                    e
                                );
                            }
                        }
                        continue;
                    }
                }

                files_processed += 1;
            }
            Err(e) => {
                if verbose {
                    if json_output {
                        eprintln!(
                            r#"{{"status": "warning", "message": "Failed to parse file {}: {}"}}"#,
                            file_path.display(),
                            e
                        );
                    } else {
                        eprintln!(
                            "Warning: Failed to parse file {}: {}",
                            file_path.display(),
                            e
                        );
                    }
                }
            }
        }
    }

    if verbose && !json_output {
        println!(
            "Processed {} files, {} total messages, {} unique messages",
            files_processed, total_messages, unique_messages
        );
    }

    if all_usage_data.is_empty() {
        if json_output {
            println!(
                r#"{{"status": "success", "message": "No usage data found matching filters", "data": []}}"#
            );
        } else {
            println!("No usage data found matching your filters.");
        }
        return Ok(());
    }

    // Analyze optimization opportunities
    let mut optimization_summary = match optimization_engine
        .analyze_optimization_opportunities(all_usage_data)
    {
        Ok(summary) => summary,
        Err(e) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Failed to analyze optimization opportunities: {}"}}"#,
                    e
                );
            } else {
                eprintln!("Error: Failed to analyze optimization opportunities: {}", e);
            }
            std::process::exit(1);
        }
    };

    // Apply confidence threshold filter if specified
    if let Some(min_confidence) = confidence_threshold {
        optimization_summary =
            optimization_engine.filter_by_confidence(optimization_summary, min_confidence);
    }

    // Apply model transition filter if specified
    if model_from.is_some() || model_to.is_some() {
        optimization_summary = optimization_engine.filter_by_model_transition(
            optimization_summary,
            model_from,
            model_to,
        );
    }

    // Convert currencies if needed
    if target_currency != "USD" {
        let currency_converter = CurrencyConverter::new();

        // Convert currency values in the summary
            if let Ok(converted_current) = currency_converter
                .convert_from_usd(optimization_summary.total_current_cost, target_currency)
                .await
            {
                optimization_summary.total_current_cost = converted_current;
            }
            if let Ok(converted_potential) = currency_converter
                .convert_from_usd(optimization_summary.total_potential_cost, target_currency)
                .await
            {
                optimization_summary.total_potential_cost = converted_potential;
            }
            if let Ok(converted_savings) = currency_converter
                .convert_from_usd(
                    optimization_summary.total_potential_savings,
                    target_currency,
                )
                .await
            {
                optimization_summary.total_potential_savings = converted_savings;
            }

            // Convert recommendation values
            for recommendation in &mut optimization_summary.recommendations {
                if let Ok(converted) = currency_converter
                    .convert_from_usd(recommendation.potential_savings, target_currency)
                    .await
                {
                    recommendation.potential_savings = converted;
                }
                if let Ok(converted) = currency_converter
                    .convert_from_usd(recommendation.total_current_cost, target_currency)
                    .await
                {
                    recommendation.total_current_cost = converted;
                }
                if let Ok(converted) = currency_converter
                    .convert_from_usd(recommendation.total_potential_cost, target_currency)
                    .await
                {
                    recommendation.total_potential_cost = converted;
                }
            }
    }

    // Handle export if specified
    if let Some(export_format) = export {
        match export_format.to_lowercase().as_str() {
            "json" => match serde_json::to_string_pretty(&optimization_summary) {
                Ok(json_str) => {
                    let filename = format!(
                        "optimization_recommendations_{}.json",
                        chrono::Utc::now().format("%Y%m%d_%H%M%S")
                    );
                    if let Err(e) = std::fs::write(&filename, json_str) {
                        if json_output {
                            println!(
                                r#"{{"status": "error", "message": "Failed to write export file: {}"}}"#,
                                e
                            );
                        } else {
                            eprintln!("Error: Failed to write export file: {}", e);
                        }
                        std::process::exit(1);
                    } else {
                        if json_output {
                            println!(
                                r#"{{"status": "success", "message": "Exported to {}", "file": "{}"}}"#,
                                filename, filename
                            );
                        } else {
                            println!("Optimization recommendations exported to: {}", filename);
                        }
                        return Ok(());
                    }
                }
                Err(e) => {
                    if json_output {
                        println!(
                            r#"{{"status": "error", "message": "Failed to serialize recommendations: {}"}}"#,
                            e
                        );
                    } else {
                        eprintln!("Error: Failed to serialize recommendations: {}", e);
                    }
                    std::process::exit(1);
                }
            },
            "csv" => {
                let mut csv_content = String::new();
                csv_content.push_str("Current Model,Suggested Model,Confidence,Conversations,Current Cost,Potential Cost,Savings,Savings %,Reasoning\n");

                for rec in &optimization_summary.recommendations {
                    csv_content.push_str(&format!(
                        "{},{},{:.2},{},{:.2},{:.2},{:.2},{:.1}%,\"{}\"\n",
                        rec.current_model,
                        rec.suggested_model,
                        rec.confidence_score,
                        rec.conversation_count,
                        rec.total_current_cost,
                        rec.total_potential_cost,
                        rec.potential_savings,
                        rec.potential_savings_percentage,
                        rec.reasoning.replace('"', "'")
                    ));
                }

                let filename = format!(
                    "optimization_recommendations_{}.csv",
                    chrono::Utc::now().format("%Y%m%d_%H%M%S")
                );
                if let Err(e) = std::fs::write(&filename, csv_content) {
                    if json_output {
                        println!(
                            r#"{{"status": "error", "message": "Failed to write CSV file: {}"}}"#,
                            e
                        );
                    } else {
                        eprintln!("Error: Failed to write CSV file: {}", e);
                    }
                    std::process::exit(1);
                } else {
                    if json_output {
                        println!(
                            r#"{{"status": "success", "message": "Exported to {}", "file": "{}"}}"#,
                            filename, filename
                        );
                    } else {
                        println!("Optimization recommendations exported to: {}", filename);
                    }
                    return Ok(());
                }
            }
            _ => {
                if json_output {
                    println!(
                        r#"{{"status": "error", "message": "Unsupported export format. Use 'json' or 'csv'"}}"#
                    );
                } else {
                    eprintln!("Error: Unsupported export format. Use 'json' or 'csv'");
                }
                std::process::exit(1);
            }
        }
    }

    // Display results
    if json_output {
        match serde_json::to_string_pretty(&optimization_summary) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                println!(
                    r#"{{"status": "error", "message": "Failed to serialize results: {}"}}"#,
                    e
                );
                std::process::exit(1);
            }
        }
    } else {
        // Show potential savings summary if requested
        if potential_savings {
            println!("Optimization Savings Summary:");
            println!(
                "  Total Conversations Analyzed: {}",
                optimization_summary.total_conversations_analyzed
            );
            println!(
                "  Current Total Cost: {}",
                models::currency::format_currency(
                    optimization_summary.total_current_cost,
                    target_currency,
                    decimal_places
                )
            );
            println!(
                "  Potential Total Cost: {}",
                models::currency::format_currency(
                    optimization_summary.total_potential_cost,
                    target_currency,
                    decimal_places
                )
            );
            println!(
                "  Total Potential Savings: {}",
                models::currency::format_currency(
                    optimization_summary.total_potential_savings,
                    target_currency,
                    decimal_places
                )
            );
            println!(
                "  Savings Percentage: {:.1}%",
                optimization_summary.savings_percentage
            );

            if !optimization_summary.optimization_opportunities.is_empty() {
                println!("\nSavings by Current Model:");
                let mut opportunities: Vec<_> = optimization_summary
                    .optimization_opportunities
                    .iter()
                    .collect();
                opportunities
                    .sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

                for (model, savings) in opportunities {
                    println!(
                        "  {}: {}",
                        model,
                        models::currency::format_currency(
                            *savings,
                            target_currency,
                            decimal_places
                        )
                    );
                }
            }
        } else {
            // Show detailed recommendations
            println!("Model Usage Optimization Analysis");
            println!("================================");
            println!();

            println!("Summary:");
            println!(
                "  Conversations Analyzed: {}",
                optimization_summary.total_conversations_analyzed
            );
            println!(
                "  Current Total Cost: {}",
                models::currency::format_currency(
                    optimization_summary.total_current_cost,
                    target_currency,
                    decimal_places
                )
            );
            println!(
                "  Potential Total Cost: {}",
                models::currency::format_currency(
                    optimization_summary.total_potential_cost,
                    target_currency,
                    decimal_places
                )
            );
            println!(
                "  Total Potential Savings: {}",
                models::currency::format_currency(
                    optimization_summary.total_potential_savings,
                    target_currency,
                    decimal_places
                )
            );
            println!(
                "  Savings Percentage: {:.1}%",
                optimization_summary.savings_percentage
            );
            println!();

            if optimization_summary.recommendations.is_empty() {
                println!("No optimization opportunities found with the current filters.");
                println!("Your model usage appears to be well-optimized!");
            } else {
                println!("Optimization Recommendations:");
                println!("============================");

                for (i, rec) in optimization_summary.recommendations.iter().enumerate() {
                    println!();
                    println!("{}. {} → {}", i + 1, rec.current_model, rec.suggested_model);
                    println!("   Conversations: {}", rec.conversation_count);
                    println!(
                        "   Confidence: {:.0}% ({:?})",
                        rec.confidence_score * 100.0,
                        rec.confidence_level
                    );
                    println!(
                        "   Current Cost: {}",
                        models::currency::format_currency(
                            rec.total_current_cost,
                            target_currency,
                            decimal_places
                        )
                    );
                    println!(
                        "   Potential Cost: {}",
                        models::currency::format_currency(
                            rec.total_potential_cost,
                            target_currency,
                            decimal_places
                        )
                    );
                    println!(
                        "   Savings: {} ({:.1}%)",
                        models::currency::format_currency(
                            rec.potential_savings,
                            target_currency,
                            decimal_places
                        ),
                        rec.potential_savings_percentage
                    );
                    println!("   Reasoning: {}", rec.reasoning);
                }

                println!();
                println!("Implementation Tips:");
                println!("- Start with high-confidence recommendations");
                println!("- Test suggested models on similar conversations");
                println!("- Monitor quality vs. cost trade-offs");
                println!("- Use 'ccost optimize --export csv' to save recommendations");
            }
        }

        // Show model distribution
        if !optimization_summary.model_distribution.is_empty() && verbose {
            println!();
            println!("Current Model Distribution:");
            let mut distribution: Vec<_> = optimization_summary.model_distribution.iter().collect();
            distribution.sort_by(|a, b| b.1.cmp(a.1));

            for (model, count) in distribution {
                println!("  {}: {} conversations", model, count);
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load configuration
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error: Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Determine final currency (CLI override takes precedence)
    let target_currency = cli
        .currency
        .as_ref()
        .unwrap_or(&config.currency.default_currency);

    // Determine final colored setting (CLI override takes precedence)
    let colored = cli.colored || config.output.colored;

    match cli.command {
        Commands::Usage {
            timeframe,
            project,
            since,
            until,
            model,
        } => {
            handle_usage_command(
                timeframe,
                project,
                since,
                until,
                model,
                target_currency,
                config.output.decimal_places,
                cli.json,
                cli.verbose,
                colored,
                &config.timezone.timezone,
                config.timezone.daily_cutoff_hour,
            )
            .await?;
        }
        Commands::Projects { sort_by } => {
            handle_projects_command(
                sort_by,
                target_currency,
                config.output.decimal_places,
                cli.json,
                cli.verbose,
                colored,
            )
            .await?;
        }
        Commands::Conversations {
            sort_by,
            project,
            since,
            until,
            model,
            min_cost,
            max_cost,
            outliers_only,
            min_efficiency,
            max_efficiency,
            export,
        } => {
            handle_conversations_command(
                sort_by,
                project,
                since,
                until,
                model,
                min_cost,
                max_cost,
                outliers_only,
                min_efficiency,
                max_efficiency,
                export,
                target_currency,
                config.output.decimal_places,
                cli.json,
                cli.verbose,
                colored,
            )
            .await?;
        }
        Commands::Optimize {
            project,
            model,
            since,
            until,
            potential_savings,
            export,
            confidence_threshold,
            model_from,
            model_to,
        } => {
            handle_optimize_command(
                project,
                model,
                since,
                until,
                potential_savings,
                export,
                confidence_threshold,
                model_from,
                model_to,
                target_currency,
                config.output.decimal_places,
                cli.json,
                cli.verbose,
                colored,
            )
            .await?;
        }
        Commands::Config { action } => {
            handle_config_action(action, cli.json);
        }
        Commands::Pricing { action } => {
            // TODO: implement pricing command handler
            eprintln!("Pricing command not yet implemented");
        }
        Commands::Watch {
            ref project,
            threshold,
            no_charts,
            refresh_rate,
        } => {
            if let Err(e) =
                handle_watch_command(project.clone(), threshold, no_charts, refresh_rate, &cli)
                    .await
            {
                eprintln!("Error starting watch mode: {}", e);
                std::process::exit(1);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli_structure() {
        // Verify the CLI can be built without panicking
        Cli::command().debug_assert();
    }

    #[test]
    fn test_global_options() {
        let cli = Cli::try_parse_from([
            "ccost",
            "--config",
            "/custom/config.toml",
            "--currency",
            "EUR",
            "--timezone",
            "America/New_York",
            "--verbose",
            "--json",
            "usage",
        ])
        .expect("Failed to parse CLI arguments");

        assert_eq!(cli.config, Some("/custom/config.toml".to_string()));
        assert_eq!(cli.currency, Some("EUR".to_string()));
        assert_eq!(cli.timezone, Some("America/New_York".to_string()));
        assert!(cli.verbose);
        assert!(cli.json);
    }

    #[test]
    fn test_usage_command_options() {
        let cli = Cli::try_parse_from([
            "ccost",
            "usage",
            "--project",
            "transcribr",
            "--since",
            "2025-06-01",
            "--until",
            "2025-06-09",
            "--model",
            "claude-sonnet-4",
        ])
        .expect("Failed to parse CLI arguments");

        match cli.command {
            Commands::Usage {
                project,
                since,
                until,
                model,
                timeframe,
            } => {
                assert_eq!(project, Some("transcribr".to_string()));
                assert_eq!(since, Some("2025-06-01".to_string()));
                assert_eq!(until, Some("2025-06-09".to_string()));
                assert_eq!(model, Some("claude-sonnet-4".to_string()));
                assert!(timeframe.is_none());
            }
            _ => assert!(false, "Expected Usage command but got different command"),
        }
    }

    #[test]
    fn test_usage_timeframe_subcommands() {
        let cli = Cli::try_parse_from(["ccost", "usage", "today"]).unwrap();
        match cli.command {
            Commands::Usage { timeframe, .. } => {
                assert!(matches!(
                    timeframe,
                    Some(UsageTimeframe::Today {
                        project: None,
                        model: None
                    })
                ));
            }
            _ => assert!(false, "Expected Usage command but got different command"),
        }

        let cli =
            Cli::try_parse_from(["ccost", "usage", "yesterday", "--project", "test"]).unwrap();
        match cli.command {
            Commands::Usage { timeframe, .. } => match timeframe {
                Some(UsageTimeframe::Yesterday { project, model }) => {
                    assert_eq!(project, Some("test".to_string()));
                    assert_eq!(model, None);
                }
                _ => assert!(false, "Expected Yesterday timeframe but got different timeframe"),
            },
            _ => assert!(false, "Expected Usage command but got different command"),
        }

        let cli =
            Cli::try_parse_from(["ccost", "usage", "this-week", "--model", "claude-sonnet-4"])
                .expect("Failed to parse CLI arguments");
        match cli.command {
            Commands::Usage { timeframe, .. } => match timeframe {
                Some(UsageTimeframe::ThisWeek { project, model }) => {
                    assert_eq!(project, None);
                    assert_eq!(model, Some("claude-sonnet-4".to_string()));
                }
                _ => assert!(false, "Expected ThisWeek timeframe but got different timeframe"),
            },
            _ => assert!(false, "Expected Usage command but got different command"),
        }

        let cli = Cli::try_parse_from(["ccost", "usage", "this-month"]).unwrap();
        match cli.command {
            Commands::Usage { timeframe, .. } => {
                assert!(matches!(
                    timeframe,
                    Some(UsageTimeframe::ThisMonth {
                        project: None,
                        model: None
                    })
                ));
            }
            _ => assert!(false, "Expected Usage command but got different command"),
        }
    }

    #[test]
    fn test_projects_command_options() {
        let cli = Cli::try_parse_from(["ccost", "projects", "cost"]).unwrap();

        match cli.command {
            Commands::Projects { sort_by } => {
                assert!(matches!(sort_by, Some(ProjectSort::Cost)));
            }
            _ => assert!(false, "Expected Projects command but got different command"),
        }

        let cli = Cli::try_parse_from(["ccost", "projects", "tokens"]).unwrap();

        match cli.command {
            Commands::Projects { sort_by } => {
                assert!(matches!(sort_by, Some(ProjectSort::Tokens)));
            }
            _ => assert!(false, "Expected Projects command but got different command"),
        }

        // Test default (no subcommand)
        let cli = Cli::try_parse_from(["ccost", "projects"]).unwrap();

        match cli.command {
            Commands::Projects { sort_by } => {
                assert!(matches!(sort_by, None));
            }
            _ => assert!(false, "Expected Projects command but got different command"),
        }
    }

    #[test]
    fn test_config_command_options() {
        let cli = Cli::try_parse_from(["ccost", "config", "show"]).unwrap();

        match cli.command {
            Commands::Config { action } => {
                assert!(matches!(action, ConfigAction::Show));
            }
            _ => assert!(false, "Expected Config command but got different command"),
        }

        let cli = Cli::try_parse_from(["ccost", "config", "init"]).unwrap();

        match cli.command {
            Commands::Config { action } => {
                assert!(matches!(action, ConfigAction::Init));
            }
            _ => assert!(false, "Expected Config command but got different command"),
        }

        let cli =
            Cli::try_parse_from(["ccost", "config", "set", "currency.default_currency", "EUR"])
                .expect("Failed to parse CLI arguments");

        match cli.command {
            Commands::Config { action } => match action {
                ConfigAction::Set { key, value } => {
                    assert_eq!(key, "currency.default_currency");
                    assert_eq!(value, "EUR");
                }
                _ => assert!(false, "Expected Set action but got different action"),
            },
            _ => assert!(false, "Expected Config command but got different command"),
        }
    }

    #[test]
    fn test_invalid_command_fails() {
        let result = Cli::try_parse_from(["ccost", "invalid"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_optimize_command_options() {
        let cli = Cli::try_parse_from([
            "ccost",
            "optimize",
            "--project",
            "transcribr",
            "--since",
            "2025-06-01",
            "--until",
            "2025-06-09",
            "--confidence-threshold",
            "0.8",
            "--model-from",
            "Opus",
            "--model-to",
            "Sonnet",
            "--potential-savings",
            "--export",
            "csv",
        ])
        .expect("Failed to parse CLI arguments");

        match cli.command {
            Commands::Optimize {
                project,
                since,
                until,
                potential_savings,
                export,
                confidence_threshold,
                model_from,
                model_to,
            } => {
                assert_eq!(project, Some("transcribr".to_string()));
                assert_eq!(since, Some("2025-06-01".to_string()));
                assert_eq!(until, Some("2025-06-09".to_string()));
                assert!(potential_savings);
                assert_eq!(export, Some("csv".to_string()));
                assert_eq!(confidence_threshold, Some(0.8));
                assert_eq!(model_from, Some("Opus".to_string()));
                assert_eq!(model_to, Some("Sonnet".to_string()));
            }
            _ => assert!(false, "Expected Optimize command but got different command"),
        }
    }

    #[test]
    fn test_help_commands() {
        // Test that help commands don't panic
        let result = Cli::try_parse_from(["ccost", "--help"]);
        assert!(result.is_err()); // Help exits with error code but shouldn't panic

        let result = Cli::try_parse_from(["ccost", "usage", "--help"]);
        assert!(result.is_err()); // Help exits with error code but shouldn't panic
    }
}

