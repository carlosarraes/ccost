// ccost: Claude Cost Tracking Tool
use clap::{Parser, Subcommand};
use chrono::{DateTime, Utc, NaiveDate, TimeZone, Datelike};
use std::path::PathBuf;
use config::Config;
use models::{PricingManager, ModelPricing};
use models::currency::CurrencyConverter;
use storage::Database;
use parser::jsonl::JsonlParser;
use parser::deduplication::DeduplicationEngine;
use analysis::{UsageTracker, UsageFilter, CostCalculationMode, ProjectAnalyzer, ProjectSortBy};
use output::OutputFormat;

// Module declarations
mod config;
mod parser;
mod storage;
mod models;
mod analysis;
mod output;
mod sync;

// Helper structure to associate usage data with project name
#[derive(Debug, Clone)]
struct EnhancedUsageData {
    usage_data: parser::jsonl::UsageData,
    project_name: String,
}

#[derive(Parser)]
#[command(name = "ccost")]
#[command(about = "Claude Cost Tracking Tool")]
#[command(version)]
struct Cli {
    /// Custom config file path
    #[arg(long, global = true)]
    config: Option<String>,
    
    /// Override currency (EUR, GBP, JPY, etc.)
    #[arg(long, global = true)]
    currency: Option<String>,
    
    /// Override timezone
    #[arg(long, global = true)]
    timezone: Option<String>,
    
    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
    
    /// JSON output format
    #[arg(long, global = true)]
    json: bool,
    
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum UsageTimeframe {
    /// Show today's usage
    Today {
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,
        
        /// Filter by model
        #[arg(long)]
        model: Option<String>,
    },
    /// Show yesterday's usage
    Yesterday {
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,
        
        /// Filter by model
        #[arg(long)]
        model: Option<String>,
    },
    /// Show this week's usage
    #[command(name = "this-week")]
    ThisWeek {
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,
        
        /// Filter by model
        #[arg(long)]
        model: Option<String>,
    },
    /// Show this month's usage
    #[command(name = "this-month")]
    ThisMonth {
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,
        
        /// Filter by model
        #[arg(long)]
        model: Option<String>,
    },
}

#[derive(Subcommand)]
enum ProjectSort {
    /// Sort projects by cost (highest first)
    Cost,
    /// Sort projects by token usage (highest first)
    Tokens,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Initialize fresh configuration
    Init,
    /// Set configuration value
    Set {
        /// Configuration key (e.g., currency.default_currency)
        key: String,
        /// Configuration value
        value: String,
    },
}

#[derive(Subcommand)]
enum PricingAction {
    /// List current pricing for all models
    List,
    /// Update pricing from all available sources
    Update {
        #[command(subcommand)]
        source: Option<PricingSource>,
    },
    /// Set custom pricing for a model
    Set {
        /// Model name (e.g., claude-sonnet-4)
        model: String,
        /// Input token price per million tokens
        input_price: String,
        /// Output token price per million tokens
        output_price: String,
    },
}

#[derive(Subcommand)]
enum PricingSource {
    /// Update from GitHub repository
    Github,
    /// Update via web scraping
    Scrape,
}

#[derive(Subcommand)]
enum Commands {
    /// Show usage analysis
    Usage {
        #[command(subcommand)]
        timeframe: Option<UsageTimeframe>,
        
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,
        
        /// Start date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,
        
        /// End date (YYYY-MM-DD)
        #[arg(long)]
        until: Option<String>,
        
        /// Filter by model
        #[arg(long)]
        model: Option<String>,
    },
    /// List and analyze projects
    Projects {
        #[command(subcommand)]
        sort_by: Option<ProjectSort>,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Model pricing management
    Pricing {
        #[command(subcommand)]
        action: PricingAction,
    },
}


fn handle_config_action(action: ConfigAction, json_output: bool) {
    match action {
        ConfigAction::Init => {
            match Config::default().save() {
                Ok(()) => {
                    if json_output {
                        println!(r#"{{"status": "success", "message": "Configuration initialized successfully"}}"#);
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
                        println!(r#"{{"status": "error", "message": "Failed to initialize config: {}"}}"#, e);
                    } else {
                        eprintln!("Error: Failed to initialize config: {}", e);
                    }
                    std::process::exit(1);
                }
            }
        }
        ConfigAction::Show => {
            match Config::load() {
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
                        println!(r#"{{"status": "error", "message": "Failed to load config: {}"}}"#, e);
                    } else {
                        eprintln!("Error: Failed to load config: {}", e);
                    }
                    std::process::exit(1);
                }
            }
        }
        ConfigAction::Set { key, value } => {
            match Config::load() {
                Ok(mut config) => {
                    match config.set_value(&key, &value) {
                        Ok(()) => {
                            match config.save() {
                                Ok(()) => {
                                    if json_output {
                                        println!(r#"{{"status": "success", "message": "Configuration updated: {} = {}"}}"#, key, value);
                                    } else {
                                        println!("Configuration updated: {} = {}", key, value);
                                    }
                                }
                                Err(e) => {
                                    if json_output {
                                        println!(r#"{{"status": "error", "message": "Failed to save config: {}"}}"#, e);
                                    } else {
                                        eprintln!("Error: Failed to save config: {}", e);
                                    }
                                    std::process::exit(1);
                                }
                            }
                        }
                        Err(e) => {
                            if json_output {
                                println!(r#"{{"status": "error", "message": "Invalid configuration: {}"}}"#, e);
                            } else {
                                eprintln!("Error: Invalid configuration: {}", e);
                            }
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    if json_output {
                        println!(r#"{{"status": "error", "message": "Failed to load config: {}"}}"#, e);
                    } else {
                        eprintln!("Error: Failed to load config: {}", e);
                    }
                    std::process::exit(1);
                }
            }
        }
    }
}

fn handle_pricing_action(action: PricingAction, json_output: bool) {
    // Initialize database and pricing manager
    let database = match get_database() {
        Ok(db) => db,
        Err(e) => {
            if json_output {
                println!(r#"{{"status": "error", "message": "Failed to initialize database: {}"}}"#, e);
            } else {
                eprintln!("Error: Failed to initialize database: {}", e);
            }
            std::process::exit(1);
        }
    };

    let mut pricing_manager = PricingManager::with_database(database);

    match action {
        PricingAction::List => {
            match pricing_manager.list_models() {
                Ok(models) => {
                    if json_output {
                        let mut pricing_list = Vec::new();
                        for model_name in &models {
                            if let Some(pricing) = pricing_manager.get_pricing(model_name) {
                                pricing_list.push(serde_json::json!({
                                    "model": model_name,
                                    "input_cost_per_mtok": pricing.input_cost_per_mtok,
                                    "output_cost_per_mtok": pricing.output_cost_per_mtok,
                                    "cache_cost_per_mtok": pricing.cache_cost_per_mtok
                                }));
                            }
                        }
                        match serde_json::to_string_pretty(&pricing_list) {
                            Ok(json) => println!("{}", json),
                            Err(e) => {
                                println!(r#"{{"status": "error", "message": "Failed to serialize pricing data: {}"}}"#, e);
                                std::process::exit(1);
                            }
                        }
                    } else {
                        println!("Model Pricing (per Million Tokens):");
                        println!("{:<30} {:<12} {:<12} {:<12}", "Model", "Input ($)", "Output ($)", "Cache ($)");
                        println!("{}", "-".repeat(78));
                        
                        for model_name in models {
                            if let Some(pricing) = pricing_manager.get_pricing(&model_name) {
                                println!(
                                    "{:<30} {:<12.2} {:<12.2} {:<12.2}",
                                    model_name,
                                    pricing.input_cost_per_mtok,
                                    pricing.output_cost_per_mtok,
                                    pricing.cache_cost_per_mtok
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    if json_output {
                        println!(r#"{{"status": "error", "message": "Failed to list models: {}"}}"#, e);
                    } else {
                        eprintln!("Error: Failed to list models: {}", e);
                    }
                    std::process::exit(1);
                }
            }
        }
        PricingAction::Update { source } => {
            match source {
                Some(PricingSource::Github) => {
                    if json_output {
                        println!(r#"{{"status": "error", "message": "GitHub pricing updates not yet implemented (TASK-014)"}}"#);
                    } else {
                        eprintln!("Error: GitHub pricing updates not yet implemented (TASK-014)");
                    }
                }
                Some(PricingSource::Scrape) => {
                    if json_output {
                        println!(r#"{{"status": "error", "message": "Web scraping for pricing not yet implemented (TASK-018)"}}"#);
                    } else {
                        eprintln!("Error: Web scraping for pricing not yet implemented (TASK-018)");
                    }
                }
                None => {
                    if json_output {
                        println!(r#"{{"status": "error", "message": "Automatic pricing updates not yet implemented"}}"#);
                    } else {
                        eprintln!("Error: Automatic pricing updates not yet implemented");
                    }
                }
            }
            std::process::exit(1);
        }
        PricingAction::Set { model, input_price, output_price } => {
            // Parse pricing values
            let input_cost = match input_price.parse::<f64>() {
                Ok(price) => price,
                Err(_) => {
                    if json_output {
                        println!(r#"{{"status": "error", "message": "Invalid input price format. Expected a number."}}"#);
                    } else {
                        eprintln!("Error: Invalid input price format. Expected a number.");
                    }
                    std::process::exit(1);
                }
            };

            let output_cost = match output_price.parse::<f64>() {
                Ok(price) => price,
                Err(_) => {
                    if json_output {
                        println!(r#"{{"status": "error", "message": "Invalid output price format. Expected a number."}}"#);
                    } else {
                        eprintln!("Error: Invalid output price format. Expected a number.");
                    }
                    std::process::exit(1);
                }
            };

            // Use default cache cost (10% of input cost)
            let cache_cost = input_cost * 0.1;
            let pricing = ModelPricing::new(input_cost, output_cost, cache_cost);

            match pricing_manager.set_pricing(model.clone(), pricing) {
                Ok(()) => {
                    if json_output {
                        println!(r#"{{"status": "success", "message": "Pricing set for model: {}"}}"#, model);
                    } else {
                        println!("Successfully set pricing for model: {}", model);
                        println!("  Input:  ${:.2} per million tokens", input_cost);
                        println!("  Output: ${:.2} per million tokens", output_cost);
                        println!("  Cache:  ${:.2} per million tokens (auto-calculated)", cache_cost);
                    }
                }
                Err(e) => {
                    if json_output {
                        println!(r#"{{"status": "error", "message": "Failed to set pricing: {}"}}"#, e);
                    } else {
                        eprintln!("Error: Failed to set pricing: {}", e);
                    }
                    std::process::exit(1);
                }
            }
        }
    }
}

fn get_database() -> anyhow::Result<Database> {
    let db_path = dirs::config_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("ccost")
        .join("cache.db");
    Database::new(&db_path)
}

fn handle_usage_command(
    timeframe: Option<UsageTimeframe>,
    project: Option<String>,
    since: Option<String>,
    until: Option<String>,
    model: Option<String>,
    target_currency: &str,
    cache_ttl_hours: u32,
    decimal_places: u8,
    json_output: bool,
    verbose: bool,
) {
    // Initialize database and components
    let database = match get_database() {
        Ok(db) => db,
        Err(e) => {
            if json_output {
                println!(r#"{{"status": "error", "message": "Failed to initialize database: {}"}}"#, e);
            } else {
                eprintln!("Error: Failed to initialize database: {}", e);
            }
            std::process::exit(1);
        }
    };

    // Find and parse JSONL files - use config setting
    let config_for_projects = match Config::load() {
        Ok(config) => config,
        Err(_) => {
            if json_output {
                println!(r#"{{"status": "error", "message": "Failed to load config for projects path"}}"#);
            } else {
                eprintln!("Error: Failed to load config for projects path");
            }
            std::process::exit(1);
        }
    };
    
    let projects_dir = if config_for_projects.general.claude_projects_path.starts_with("~/") {
        // Expand tilde to home directory
        if let Some(home_dir) = dirs::home_dir() {
            home_dir.join(&config_for_projects.general.claude_projects_path[2..])
        } else {
            PathBuf::from(&config_for_projects.general.claude_projects_path)
        }
    } else {
        PathBuf::from(&config_for_projects.general.claude_projects_path)
    };

    let pricing_manager = PricingManager::with_database(database);
    let usage_tracker = UsageTracker::new(CostCalculationMode::Auto);
    let parser = JsonlParser::new(projects_dir.clone());
    let mut dedup_engine = DeduplicationEngine::new();

    // Parse timeframe into date filters
    let (final_project, final_since, final_until, final_model) = 
        resolve_filters(timeframe, project, since, until, model);

    // Create usage filter
    let usage_filter = UsageFilter {
        project_name: final_project.clone(),
        model_name: final_model.clone(),
        since: final_since,
        until: final_until,
    };

    if verbose {
        print_filter_info(&usage_filter, json_output);
    }

    if verbose && !json_output {
        println!("Searching for JSONL files in: {}", projects_dir.display());
    }

    let jsonl_files = match parser.find_jsonl_files() {
        Ok(files) => files,
        Err(e) => {
            if json_output {
                println!(r#"{{"status": "error", "message": "Failed to find JSONL files: {}"}}"#, e);
            } else {
                eprintln!("Error: Failed to find JSONL files: {}", e);
                eprintln!("Make sure you have Claude conversations in: {}", projects_dir.display());
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
        return;
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
        // Extract project name from file path
        let project_name = match parser.extract_project_path(&file_path) {
            Ok(project_path) => project_path.to_string_lossy().to_string(),
            Err(_) => "Unknown".to_string(),
        };

        // Apply project filter early if specified
        if let Some(ref filter_project) = final_project {
            if project_name != *filter_project {
                continue;
            }
        }

        match parser.parse_file_with_verbose(&file_path, verbose) {
            Ok(parsed_conversation) => {
                total_messages += parsed_conversation.messages.len();
                
                // Apply deduplication
                match dedup_engine.filter_duplicates(parsed_conversation.messages) {
                    Ok(unique_data) => {
                        unique_messages += unique_data.len();
                        
                        // Create enhanced usage data with project name
                        for data in unique_data {
                            // Create an enhanced usage data structure
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
                                eprintln!(r#"{{"status": "warning", "message": "Failed to deduplicate file {}: {}"}}"#, file_path.display(), e);
                            } else {
                                eprintln!("Warning: Failed to deduplicate file {}: {}", file_path.display(), e);
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
                        eprintln!(r#"{{"status": "warning", "message": "Failed to parse file {}: {}"}}"#, file_path.display(), e);
                    } else {
                        eprintln!("Warning: Failed to parse file {}: {}", file_path.display(), e);
                    }
                }
            }
        }
    }

    if verbose && !json_output {
        println!("Processed {} files, {} total messages, {} unique messages", 
                 files_processed, total_messages, unique_messages);
    }

    if all_usage_data.is_empty() {
        if json_output {
            println!(r#"{{"status": "success", "message": "No usage data found matching filters", "data": []}}"#);
        } else {
            println!("No usage data found matching your filters.");
        }
        return;
    }

    // Convert enhanced data to tuple format
    let usage_tuples: Vec<(parser::jsonl::UsageData, String)> = all_usage_data
        .into_iter()
        .map(|enhanced| (enhanced.usage_data, enhanced.project_name))
        .collect();

    // Calculate usage with the tracker
    let project_usage = match usage_tracker.calculate_usage_with_projects(usage_tuples, &pricing_manager) {
        Ok(usage) => usage,
        Err(e) => {
            if json_output {
                println!(r#"{{"status": "error", "message": "Failed to calculate usage: {}"}}"#, e);
            } else {
                eprintln!("Error: Failed to calculate usage: {}", e);
            }
            std::process::exit(1);
        }
    };

    // Apply remaining filters to the calculated usage
    let mut filtered_usage = apply_usage_filters(project_usage, &usage_filter);
    
    // Convert currencies if needed
    if target_currency != "USD" {
        if let Ok(db_clone) = get_database() {
            let currency_converter = CurrencyConverter::new(db_clone, cache_ttl_hours);
            
            // Create an async runtime for currency conversion
            let rt = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(e) => {
                    if json_output {
                        println!(r#"{{"status": "error", "message": "Failed to create async runtime: {}"}}"#, e);
                    } else {
                        eprintln!("Error: Failed to create async runtime: {}", e);
                    }
                    std::process::exit(1);
                }
            };
            
            // Convert all USD amounts to target currency
            for project in &mut filtered_usage {
                match rt.block_on(currency_converter.convert_from_usd(project.total_cost_usd, target_currency)) {
                    Ok(converted_cost) => {
                        project.total_cost_usd = converted_cost; // Reusing the USD field for converted amount
                    }
                    Err(e) => {
                        if verbose {
                            if json_output {
                                eprintln!(r#"{{"status": "warning", "message": "Failed to convert currency for {}: {}"}}"#, project.project_name, e);
                            } else {
                                eprintln!("Warning: Failed to convert currency for {}: {}", project.project_name, e);
                            }
                        }
                        // Keep USD amounts if conversion fails
                    }
                }
                
                // Convert model-level costs too
                for model_usage in project.model_usage.values_mut() {
                    match rt.block_on(currency_converter.convert_from_usd(model_usage.cost_usd, target_currency)) {
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
    }

    if filtered_usage.is_empty() {
        if json_output {
            println!(r#"{{"status": "success", "message": "No usage data found matching filters", "data": []}}"#);
        } else {
            println!("No usage data found matching your filters.");
        }
        return;
    }

    // Display results
    if json_output {
        match filtered_usage.to_json() {
            Ok(json) => println!("{}", json),
            Err(e) => {
                println!(r#"{{"status": "error", "message": "Failed to serialize results: {}"}}"#, e);
                std::process::exit(1);
            }
        }
    } else {
        println!("{}", filtered_usage.to_table_with_currency(target_currency, decimal_places));
    }
}

fn resolve_filters(
    timeframe: Option<UsageTimeframe>,
    project: Option<String>,
    since: Option<String>,
    until: Option<String>,
    model: Option<String>,
) -> (Option<String>, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<String>) {
    let (tf_project, tf_model, tf_since, tf_until) = match timeframe {
        Some(UsageTimeframe::Today { project: tf_project, model: tf_model }) => {
            let today = Utc::now().date_naive();
            let start = Utc.from_utc_datetime(&today.and_hms_opt(0, 0, 0).unwrap());
            let end = Utc.from_utc_datetime(&today.and_hms_opt(23, 59, 59).unwrap());
            (tf_project, tf_model, Some(start), Some(end))
        },
        Some(UsageTimeframe::Yesterday { project: tf_project, model: tf_model }) => {
            let yesterday = Utc::now().date_naive() - chrono::Duration::days(1);
            let start = Utc.from_utc_datetime(&yesterday.and_hms_opt(0, 0, 0).unwrap());
            let end = Utc.from_utc_datetime(&yesterday.and_hms_opt(23, 59, 59).unwrap());
            (tf_project, tf_model, Some(start), Some(end))
        },
        Some(UsageTimeframe::ThisWeek { project: tf_project, model: tf_model }) => {
            let today = Utc::now().date_naive();
            let days_since_monday = today.weekday().num_days_from_monday();
            let monday = today - chrono::Duration::days(days_since_monday as i64);
            let start = Utc.from_utc_datetime(&monday.and_hms_opt(0, 0, 0).unwrap());
            (tf_project, tf_model, Some(start), None)
        },
        Some(UsageTimeframe::ThisMonth { project: tf_project, model: tf_model }) => {
            let today = Utc::now().date_naive();
            let first_of_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
            let start = Utc.from_utc_datetime(&first_of_month.and_hms_opt(0, 0, 0).unwrap());
            (tf_project, tf_model, Some(start), None)
        },
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
                .map(|date| Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()))
        })
    });
    
    let final_until = tf_until.or_else(|| {
        until.and_then(|s| {
            NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                .ok()
                .map(|date| Utc.from_utc_datetime(&date.and_hms_opt(23, 59, 59).unwrap()))
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
    filter: &UsageFilter
) -> Vec<analysis::usage::ProjectUsage> {
    usage.into_iter()
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
                let filtered_model_usage: std::collections::HashMap<String, analysis::usage::ModelUsage> = 
                    project.model_usage.into_iter()
                        .filter(|(model_name, _)| model_name == model_filter)
                        .collect();
                
                // Recalculate project totals based on filtered models
                let total_input_tokens = filtered_model_usage.values().map(|m| m.input_tokens).sum();
                let total_output_tokens = filtered_model_usage.values().map(|m| m.output_tokens).sum();
                let total_cache_creation_tokens = filtered_model_usage.values().map(|m| m.cache_creation_tokens).sum();
                let total_cache_read_tokens = filtered_model_usage.values().map(|m| m.cache_read_tokens).sum();
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

fn handle_projects_command(
    sort_by: Option<ProjectSort>,
    target_currency: &str,
    cache_ttl_hours: u32,
    decimal_places: u8,
    json_output: bool,
    verbose: bool,
) {
    // Initialize database and components
    let database = match get_database() {
        Ok(db) => db,
        Err(e) => {
            if json_output {
                println!(r#"{{"status": "error", "message": "Failed to initialize database: {}"}}"#, e);
            } else {
                eprintln!("Error: Failed to initialize database: {}", e);
            }
            std::process::exit(1);
        }
    };

    // Find and parse JSONL files - use config setting
    let config_for_projects = match Config::load() {
        Ok(config) => config,
        Err(_) => {
            if json_output {
                println!(r#"{{"status": "error", "message": "Failed to load config for projects path"}}"#);
            } else {
                eprintln!("Error: Failed to load config for projects path");
            }
            std::process::exit(1);
        }
    };
    
    let projects_dir = if config_for_projects.general.claude_projects_path.starts_with("~/") {
        // Expand tilde to home directory
        if let Some(home_dir) = dirs::home_dir() {
            home_dir.join(&config_for_projects.general.claude_projects_path[2..])
        } else {
            PathBuf::from(&config_for_projects.general.claude_projects_path)
        }
    } else {
        PathBuf::from(&config_for_projects.general.claude_projects_path)
    };

    let pricing_manager = PricingManager::with_database(database);
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
                println!(r#"{{"status": "error", "message": "Failed to find JSONL files: {}"}}"#, e);
            } else {
                eprintln!("Error: Failed to find JSONL files: {}", e);
                eprintln!("Make sure you have Claude conversations in: {}", projects_dir.display());
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
        return;
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
        // Extract project name from file path
        let project_name = match parser.extract_project_path(&file_path) {
            Ok(project_path) => project_path.to_string_lossy().to_string(),
            Err(_) => "Unknown".to_string(),
        };

        match parser.parse_file_with_verbose(&file_path, verbose) {
            Ok(parsed_conversation) => {
                total_messages += parsed_conversation.messages.len();
                
                // Apply deduplication
                match dedup_engine.filter_duplicates(parsed_conversation.messages) {
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
                                eprintln!(r#"{{"status": "warning", "message": "Failed to deduplicate file {}: {}"}}"#, file_path.display(), e);
                            } else {
                                eprintln!("Warning: Failed to deduplicate file {}: {}", file_path.display(), e);
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
                        eprintln!(r#"{{"status": "warning", "message": "Failed to parse file {}: {}"}}"#, file_path.display(), e);
                    } else {
                        eprintln!("Warning: Failed to parse file {}: {}", file_path.display(), e);
                    }
                }
            }
        }
    }

    if verbose && !json_output {
        println!("Processed {} files, {} total messages, {} unique messages", 
                 files_processed, total_messages, unique_messages);
    }

    if all_usage_data.is_empty() {
        if json_output {
            println!(r#"{{"status": "success", "message": "No usage data found", "data": []}}"#);
        } else {
            println!("No usage data found in your Claude projects.");
        }
        return;
    }

    // Convert enhanced data to tuple format
    let usage_tuples: Vec<(parser::jsonl::UsageData, String)> = all_usage_data
        .into_iter()
        .map(|enhanced| (enhanced.usage_data, enhanced.project_name))
        .collect();

    // Calculate usage with the tracker
    let project_usage = match usage_tracker.calculate_usage_with_projects(usage_tuples, &pricing_manager) {
        Ok(usage) => usage,
        Err(e) => {
            if json_output {
                println!(r#"{{"status": "error", "message": "Failed to calculate usage: {}"}}"#, e);
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
        return;
    }

    // Determine sort method
    let sort_method = match sort_by {
        Some(ProjectSort::Cost) => ProjectSortBy::Cost,
        Some(ProjectSort::Tokens) => ProjectSortBy::Tokens,
        None => ProjectSortBy::Name,
    };

    // Analyze and sort projects
    let mut project_summaries = project_analyzer.analyze_projects(project_usage, sort_method);
    
    // Convert currencies if needed
    if target_currency != "USD" {
        if let Ok(db_clone) = get_database() {
            let currency_converter = CurrencyConverter::new(db_clone, cache_ttl_hours);
            
            // Create an async runtime for currency conversion
            let rt = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(e) => {
                    if json_output {
                        println!(r#"{{"status": "error", "message": "Failed to create async runtime: {}"}}"#, e);
                    } else {
                        eprintln!("Error: Failed to create async runtime: {}", e);
                    }
                    std::process::exit(1);
                }
            };
            
            // Convert all USD amounts to target currency
            for summary in &mut project_summaries {
                match rt.block_on(currency_converter.convert_from_usd(summary.total_cost_usd, target_currency)) {
                    Ok(converted_cost) => {
                        summary.total_cost_usd = converted_cost; // Reusing the USD field for converted amount
                    }
                    Err(e) => {
                        if verbose {
                            if json_output {
                                eprintln!(r#"{{"status": "warning", "message": "Failed to convert currency for {}: {}"}}"#, summary.project_name, e);
                            } else {
                                eprintln!("Warning: Failed to convert currency for {}: {}", summary.project_name, e);
                            }
                        }
                        // Keep USD amounts if conversion fails
                    }
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
                println!(r#"{{"status": "error", "message": "Failed to serialize results: {}"}}"#, e);
                std::process::exit(1);
            }
        }
    } else {
        println!("{}", project_summaries.to_table_with_currency(target_currency, decimal_places));
        
        // Show summary stats
        println!();
        println!("Summary:");
        println!("  Total Projects: {}", stats.total_projects);
        println!("  Total Input Tokens: {}", format_number(stats.total_input_tokens));
        println!("  Total Output Tokens: {}", format_number(stats.total_output_tokens));
        println!("  Total Messages: {}", format_number(stats.total_messages));
        println!("  Total Cost: {}", models::currency::format_currency(stats.total_cost, target_currency, decimal_places));
        
        if let Some(ref highest_cost) = stats.highest_cost_project {
            println!("  Highest Cost Project: {}", highest_cost);
        }
        
        if let Some(ref most_active) = stats.most_active_project {
            println!("  Most Active Project: {}", most_active);
        }
    }
}

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

fn main() {
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
    let target_currency = cli.currency.as_ref()
        .unwrap_or(&config.currency.default_currency);
    
    match cli.command {
        Commands::Usage { 
            timeframe,
            project, 
            since, 
            until, 
            model
        } => {
            handle_usage_command(
                timeframe, 
                project, 
                since, 
                until, 
                model, 
                target_currency,
                config.currency.cache_ttl_hours,
                config.output.decimal_places,
                cli.json,
                cli.verbose
            );
        }
        Commands::Projects { sort_by } => {
            handle_projects_command(
                sort_by,
                target_currency,
                config.currency.cache_ttl_hours,
                config.output.decimal_places,
                cli.json,
                cli.verbose
            );
        }
        Commands::Config { action } => {
            handle_config_action(action, cli.json);
        }
        Commands::Pricing { action } => {
            handle_pricing_action(action, cli.json);
        }
    }
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
            "--config", "/custom/config.toml",
            "--currency", "EUR", 
            "--timezone", "America/New_York",
            "--verbose",
            "--json",
            "usage"
        ]).unwrap();
        
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
            "--project", "transcribr",
            "--since", "2025-06-01",
            "--until", "2025-06-09",
            "--model", "claude-sonnet-4"
        ]).unwrap();
        
        match cli.command {
            Commands::Usage { project, since, until, model, timeframe } => {
                assert_eq!(project, Some("transcribr".to_string()));
                assert_eq!(since, Some("2025-06-01".to_string()));
                assert_eq!(until, Some("2025-06-09".to_string()));
                assert_eq!(model, Some("claude-sonnet-4".to_string()));
                assert!(timeframe.is_none());
            }
            _ => panic!("Expected Usage command"),
        }
    }

    #[test]
    fn test_usage_timeframe_subcommands() {
        let cli = Cli::try_parse_from(["ccost", "usage", "today"]).unwrap();
        match cli.command {
            Commands::Usage { timeframe, .. } => {
                assert!(matches!(timeframe, Some(UsageTimeframe::Today { project: None, model: None })));
            }
            _ => panic!("Expected Usage command"),
        }

        let cli = Cli::try_parse_from(["ccost", "usage", "yesterday", "--project", "test"]).unwrap();
        match cli.command {
            Commands::Usage { timeframe, .. } => {
                match timeframe {
                    Some(UsageTimeframe::Yesterday { project, model }) => {
                        assert_eq!(project, Some("test".to_string()));
                        assert_eq!(model, None);
                    }
                    _ => panic!("Expected Yesterday timeframe"),
                }
            }
            _ => panic!("Expected Usage command"),
        }

        let cli = Cli::try_parse_from(["ccost", "usage", "this-week", "--model", "claude-sonnet-4"]).unwrap();
        match cli.command {
            Commands::Usage { timeframe, .. } => {
                match timeframe {
                    Some(UsageTimeframe::ThisWeek { project, model }) => {
                        assert_eq!(project, None);
                        assert_eq!(model, Some("claude-sonnet-4".to_string()));
                    }
                    _ => panic!("Expected ThisWeek timeframe"),
                }
            }
            _ => panic!("Expected Usage command"),
        }

        let cli = Cli::try_parse_from(["ccost", "usage", "this-month"]).unwrap();
        match cli.command {
            Commands::Usage { timeframe, .. } => {
                assert!(matches!(timeframe, Some(UsageTimeframe::ThisMonth { project: None, model: None })));
            }
            _ => panic!("Expected Usage command"),
        }
    }

    #[test]
    fn test_projects_command_options() {
        let cli = Cli::try_parse_from([
            "ccost", 
            "projects",
            "cost"
        ]).unwrap();
        
        match cli.command {
            Commands::Projects { sort_by } => {
                assert!(matches!(sort_by, Some(ProjectSort::Cost)));
            }
            _ => panic!("Expected Projects command"),
        }

        let cli = Cli::try_parse_from([
            "ccost", 
            "projects",
            "tokens"
        ]).unwrap();
        
        match cli.command {
            Commands::Projects { sort_by } => {
                assert!(matches!(sort_by, Some(ProjectSort::Tokens)));
            }
            _ => panic!("Expected Projects command"),
        }

        // Test default (no subcommand)
        let cli = Cli::try_parse_from([
            "ccost", 
            "projects"
        ]).unwrap();
        
        match cli.command {
            Commands::Projects { sort_by } => {
                assert!(matches!(sort_by, None));
            }
            _ => panic!("Expected Projects command"),
        }
    }

    #[test]
    fn test_config_command_options() {
        let cli = Cli::try_parse_from([
            "ccost", 
            "config",
            "show"
        ]).unwrap();
        
        match cli.command {
            Commands::Config { action } => {
                assert!(matches!(action, ConfigAction::Show));
            }
            _ => panic!("Expected Config command"),
        }

        let cli = Cli::try_parse_from([
            "ccost", 
            "config",
            "init"
        ]).unwrap();
        
        match cli.command {
            Commands::Config { action } => {
                assert!(matches!(action, ConfigAction::Init));
            }
            _ => panic!("Expected Config command"),
        }

        let cli = Cli::try_parse_from([
            "ccost", 
            "config",
            "set",
            "currency.default_currency",
            "EUR"
        ]).unwrap();
        
        match cli.command {
            Commands::Config { action } => {
                match action {
                    ConfigAction::Set { key, value } => {
                        assert_eq!(key, "currency.default_currency");
                        assert_eq!(value, "EUR");
                    }
                    _ => panic!("Expected Set action"),
                }
            }
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_pricing_command_options() {
        let cli = Cli::try_parse_from([
            "ccost", 
            "pricing",
            "list"
        ]).unwrap();
        
        match cli.command {
            Commands::Pricing { action } => {
                assert!(matches!(action, PricingAction::List));
            }
            _ => panic!("Expected Pricing command"),
        }

        let cli = Cli::try_parse_from([
            "ccost", 
            "pricing",
            "update"
        ]).unwrap();
        
        match cli.command {
            Commands::Pricing { action } => {
                match action {
                    PricingAction::Update { source } => {
                        assert!(source.is_none());
                    }
                    _ => panic!("Expected Update action"),
                }
            }
            _ => panic!("Expected Pricing command"),
        }

        let cli = Cli::try_parse_from([
            "ccost", 
            "pricing",
            "update",
            "github"
        ]).unwrap();
        
        match cli.command {
            Commands::Pricing { action } => {
                match action {
                    PricingAction::Update { source } => {
                        assert!(matches!(source, Some(PricingSource::Github)));
                    }
                    _ => panic!("Expected Update action"),
                }
            }
            _ => panic!("Expected Pricing command"),
        }

        let cli = Cli::try_parse_from([
            "ccost", 
            "pricing",
            "set",
            "claude-sonnet-4",
            "3.0",
            "15.0"
        ]).unwrap();
        
        match cli.command {
            Commands::Pricing { action } => {
                match action {
                    PricingAction::Set { model, input_price, output_price } => {
                        assert_eq!(model, "claude-sonnet-4");
                        assert_eq!(input_price, "3.0");
                        assert_eq!(output_price, "15.0");
                    }
                    _ => panic!("Expected Set action"),
                }
            }
            _ => panic!("Expected Pricing command"),
        }
    }

    #[test]
    fn test_invalid_command_fails() {
        let result = Cli::try_parse_from(["ccost", "invalid"]);
        assert!(result.is_err());
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