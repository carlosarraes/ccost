use crate::analysis::{
    CostCalculationMode, DailyUsage, DailyUsageList, TimezoneCalculator, UsageFilter, UsageTracker,
};
use crate::config::Config;
use crate::models::PricingManager;
use crate::models::currency::CurrencyConverter;
use crate::output::OutputFormat;
use crate::parser::deduplication::DeduplicationEngine;
use crate::parser::jsonl::JsonlParser;
use crate::utils::{
    DateFormatter, EnhancedUsageData, apply_usage_filters, maybe_hide_project_name,
    print_filter_info, resolve_filters,
};
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum UsageTimeframe {
    Today,
    Yesterday,
    ThisWeek,
    ThisMonth,
    Daily { days: u32 },
}
pub async fn handle_usage_command(
    timeframe: Option<UsageTimeframe>,
    project: Option<String>,
    since: Option<String>,
    until: Option<String>,
    model: Option<String>,
    target_currency: &str,
    decimal_places: u8,
    json_output: bool,
    verbose: bool,
    colored: bool,
    hidden: bool,
    timezone_name: &str,
    daily_cutoff_hour: u8,
    date_format: &str,
) -> anyhow::Result<()> {
    // Initialize timezone calculator
    let timezone_calc = match TimezoneCalculator::new(timezone_name, daily_cutoff_hour) {
        Ok(calc) => calc,
        Err(e) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Invalid timezone configuration: {e}"}}"#
                );
            } else {
                eprintln!("Error: Invalid timezone configuration: {e}");
            }
            std::process::exit(1);
        }
    };

    // Initialize date formatter
    let date_formatter = match DateFormatter::new(date_format) {
        Ok(formatter) => formatter,
        Err(e) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Invalid date format configuration: {e}"}}"#
                );
            } else {
                eprintln!("Error: Invalid date format configuration: {e}");
            }
            std::process::exit(1);
        }
    };

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

    // Initialize pricing manager based on configuration
    // Default to static pricing for speed, only use live when explicitly requested
    let mut pricing_manager = match config_for_projects.pricing.source.as_str() {
        "live" => PricingManager::with_live_pricing(),
        _ => PricingManager::new(), // "auto", "static" or unknown - all use static by default
    };

    // Only enable live pricing when explicitly set to "live"
    pricing_manager.set_live_pricing(config_for_projects.pricing.source == "live");

    // Pre-fetch pricing data if live pricing is enabled
    if let Err(_) = pricing_manager.initialize_live_pricing().await {
        // If live pricing fails, it will fall back to static during calculations
    }

    let usage_tracker = UsageTracker::new(CostCalculationMode::Auto);
    let parser = JsonlParser::new(projects_dir.clone());
    let mut dedup_engine = DeduplicationEngine::new();

    // Check if this is a daily command - requires special handling
    if let Some(UsageTimeframe::Daily { days }) = &timeframe {
        handle_daily_usage_command(
            *days,
            project,
            model,
            target_currency,
            decimal_places,
            json_output,
            verbose,
            colored,
            hidden,
            timezone_name,
            daily_cutoff_hour,
            date_format,
        )
        .await?;
        return Ok(());
    }

    // Parse timeframe into date filters
    let (final_project, final_since, final_until, final_model) =
        resolve_filters(timeframe, project, since, until, model, &timezone_calc);

    // Create usage filter
    let usage_filter = UsageFilter {
        project_name: final_project.clone(),
        model_name: final_model.clone(),
        since: final_since,
        until: final_until,
    };

    if verbose {
        print_filter_info(&usage_filter, json_output, &date_formatter);
    }

    if verbose && !json_output {
        println!("Searching for JSONL files in: {}", projects_dir.display());
    }

    let jsonl_files = match parser.find_jsonl_files() {
        Ok(files) => files,
        Err(e) => {
            if json_output {
                println!(r#"{{"status": "error", "message": "Failed to find JSONL files: {e}"}}"#);
            } else {
                eprintln!("Error: Failed to find JSONL files: {e}");
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
                let raw_project_name =
                    parser.get_unified_project_name(&file_path, &parsed_conversation.messages);
                let project_name = maybe_hide_project_name(&raw_project_name, hidden);

                // Apply project filter if specified
                if let Some(ref filter_project) = final_project
                    && raw_project_name != *filter_project
                {
                    continue;
                }
                total_messages += parsed_conversation.messages.len();

                // Apply deduplication
                match dedup_engine.filter_duplicates(parsed_conversation.messages, &project_name) {
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
            "Processed {files_processed} files, {total_messages} total messages, {unique_messages} unique messages"
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

    // Convert enhanced data to tuple format
    let usage_tuples: Vec<(crate::parser::jsonl::UsageData, String)> = all_usage_data
        .into_iter()
        .map(|enhanced| (enhanced.usage_data, enhanced.project_name))
        .collect();

    // Calculate usage with enhanced pricing (supports live pricing)
    let (project_usage, pricing_source) = match usage_tracker
        .calculate_usage_with_projects_filtered_enhanced(
            usage_tuples,
            &mut pricing_manager,
            &usage_filter,
        )
        .await
    {
        Ok((usage, source)) => (usage, source),
        Err(e) => {
            if json_output {
                println!(r#"{{"status": "error", "message": "Failed to calculate usage: {e}"}}"#);
            } else {
                eprintln!("Error: Failed to calculate usage: {e}");
            }
            std::process::exit(1);
        }
    };

    // Display pricing source in verbose mode
    if verbose && !json_output {
        if let Some(source) = &pricing_source {
            println!("Pricing source: {}", source);
        }
    }

    // Apply remaining filters to the calculated usage
    let mut filtered_usage = apply_usage_filters(project_usage, &usage_filter);

    // Convert currencies if needed
    if target_currency != "USD" {
        let currency_converter = CurrencyConverter::new();

        // Convert all USD amounts to target currency
        for project in &mut filtered_usage {
            match currency_converter
                .convert_from_usd(project.total_cost_usd, target_currency)
                .await
            {
                Ok(converted_cost) => {
                    project.total_cost_usd = converted_cost; // Reusing the USD field for converted amount
                }
                Err(e) => {
                    if verbose {
                        if json_output {
                            eprintln!(
                                r#"{{"status": "warning", "message": "Failed to convert currency for {}: {}"}}"#,
                                project.project_name, e
                            );
                        } else {
                            eprintln!(
                                "Warning: Failed to convert currency for {}: {}",
                                project.project_name, e
                            );
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
    }

    if filtered_usage.is_empty() {
        if json_output {
            println!(
                r#"{{"status": "success", "message": "No usage data found matching filters", "data": []}}"#
            );
        } else {
            println!("No usage data found matching your filters.");
        }
        return Ok(());
    }

    // Display results
    if json_output {
        match filtered_usage.to_json() {
            Ok(json) => println!("{json}"),
            Err(e) => {
                println!(r#"{{"status": "error", "message": "Failed to serialize results: {e}"}}"#);
                std::process::exit(1);
            }
        }
    } else {
        println!(
            "{}",
            filtered_usage.to_table_with_currency_and_color(
                target_currency,
                decimal_places,
                colored
            )
        );
    }
    Ok(())
}

pub async fn handle_daily_usage_command(
    days: u32,
    project_filter: Option<String>,
    model_filter: Option<String>,
    target_currency: &str,
    decimal_places: u8,
    json_output: bool,
    verbose: bool,
    colored: bool,
    hidden: bool,
    _timezone_name: &str,
    _daily_cutoff_hour: u8,
    date_format: &str,
) -> anyhow::Result<()> {
    // Initialize date formatter
    let date_formatter = match DateFormatter::new(date_format) {
        Ok(formatter) => formatter,
        Err(e) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Invalid date format configuration: {e}"}}"#
                );
            } else {
                eprintln!("Error: Invalid date format configuration: {e}");
            }
            std::process::exit(1);
        }
    };

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

    // Initialize pricing manager based on configuration
    // Default to static pricing for speed, only use live when explicitly requested
    let mut pricing_manager = match config_for_projects.pricing.source.as_str() {
        "live" => PricingManager::with_live_pricing(),
        _ => PricingManager::new(), // "auto", "static" or unknown - all use static by default
    };

    // Only enable live pricing when explicitly set to "live"
    pricing_manager.set_live_pricing(config_for_projects.pricing.source == "live");

    // Pre-fetch pricing data if live pricing is enabled
    if let Err(_) = pricing_manager.initialize_live_pricing().await {
        // If live pricing fails, it will fall back to static during calculations
    }

    let usage_tracker = UsageTracker::new(CostCalculationMode::Auto);
    let parser = JsonlParser::new(projects_dir.clone());
    let mut dedup_engine = DeduplicationEngine::new();

    if verbose && !json_output {
        println!("Searching for JSONL files in: {}", projects_dir.display());
    }

    let jsonl_files = match parser.find_jsonl_files() {
        Ok(files) => files,
        Err(e) => {
            if json_output {
                println!(r#"{{"status": "error", "message": "Failed to find JSONL files: {e}"}}"#);
            } else {
                eprintln!("Error: Failed to find JSONL files: {e}");
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
                let raw_project_name =
                    parser.get_unified_project_name(&file_path, &parsed_conversation.messages);
                let project_name = maybe_hide_project_name(&raw_project_name, hidden);

                // Apply project filter if specified
                if let Some(ref filter_project) = project_filter
                    && raw_project_name != *filter_project
                {
                    continue;
                }
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
            "Processed {files_processed} files, {total_messages} total messages, {unique_messages} unique messages"
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

    // Group usage by day
    let mut daily_usage_map: HashMap<String, DailyUsage> = HashMap::new();

    for enhanced in &all_usage_data {
        let message = &enhanced.usage_data;

        // Skip messages without usage data
        let usage = match &message.usage {
            Some(usage) => usage,
            None => continue,
        };

        // Extract model name and apply model filter
        let model_name = message
            .message
            .as_ref()
            .and_then(|m| m.model.clone())
            .unwrap_or_else(|| "unknown".to_string());

        if let Some(ref filter_model) = model_filter
            && model_name != *filter_model
        {
            continue;
        }

        // Parse timestamp and extract date
        let date_key = if let Some(timestamp_str) = &message.timestamp {
            if let Ok(message_time) = usage_tracker.parse_timestamp(timestamp_str) {
                // Check if message is within the requested days range
                let today = Utc::now().date_naive();
                let cutoff_date = today - chrono::Duration::days(days as i64 - 1);
                let message_date = message_time.date_naive();

                if message_date < cutoff_date {
                    continue;
                }

                if json_output {
                    date_formatter.format_naive_date_for_json(&message_date)
                } else {
                    date_formatter.format_naive_date_for_table(&message_date)
                }
            } else {
                continue; // Skip messages with unparseable timestamps
            }
        } else {
            continue; // Skip messages without timestamps
        };

        // Get or create daily usage entry
        let daily_usage = daily_usage_map
            .entry(date_key.clone())
            .or_insert_with(|| DailyUsage {
                date: date_key.clone(),
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_cache_creation_tokens: 0,
                total_cache_read_tokens: 0,
                total_cost_usd: 0.0,
                message_count: 0,
                projects_count: 0,
            });

        // Aggregate token counts
        let input_tokens = usage.input_tokens.unwrap_or(0);
        let output_tokens = usage.output_tokens.unwrap_or(0);
        let cache_creation_tokens = usage.cache_creation_input_tokens.unwrap_or(0);
        let cache_read_tokens = usage.cache_read_input_tokens.unwrap_or(0);

        daily_usage.total_input_tokens += input_tokens;
        daily_usage.total_output_tokens += output_tokens;
        daily_usage.total_cache_creation_tokens += cache_creation_tokens;
        daily_usage.total_cache_read_tokens += cache_read_tokens;
        daily_usage.message_count += 1;

        // Calculate cost
        let cost = if let Some(embedded_cost) = message.cost_usd {
            embedded_cost
        } else {
            // Calculate from pricing
            if let Some(pricing) = pricing_manager.get_pricing(&model_name) {
                let input_cost = (input_tokens as f64 / 1_000_000.0) * pricing.input_cost_per_mtok;
                let output_cost =
                    (output_tokens as f64 / 1_000_000.0) * pricing.output_cost_per_mtok;
                let cache_creation_cost =
                    (cache_creation_tokens as f64 / 1_000_000.0) * pricing.cache_cost_per_mtok;
                let cache_read_cost =
                    (cache_read_tokens as f64 / 1_000_000.0) * pricing.cache_cost_per_mtok;
                input_cost + output_cost + cache_creation_cost + cache_read_cost
            } else {
                0.0
            }
        };

        daily_usage.total_cost_usd += cost;
    }

    // Count projects per day
    let mut project_sets_by_day: HashMap<String, std::collections::HashSet<String>> =
        HashMap::new();
    for enhanced in all_usage_data.iter() {
        if let Some(timestamp_str) = &enhanced.usage_data.timestamp
            && let Ok(message_time) = usage_tracker.parse_timestamp(timestamp_str)
        {
            let date_key = if json_output {
                date_formatter.format_naive_date_for_json(&message_time.date_naive())
            } else {
                date_formatter.format_naive_date_for_table(&message_time.date_naive())
            };
            if daily_usage_map.contains_key(&date_key) {
                project_sets_by_day
                    .entry(date_key)
                    .or_default()
                    .insert(enhanced.project_name.clone());
            }
        }
    }

    // Update projects count
    for (date, daily_usage) in daily_usage_map.iter_mut() {
        if let Some(project_set) = project_sets_by_day.get(date) {
            daily_usage.projects_count = project_set.len();
        }
    }

    // Convert to sorted vector
    let mut daily_usage_vec: Vec<DailyUsage> = daily_usage_map.into_values().collect();
    daily_usage_vec.sort_by(|a, b| a.date.cmp(&b.date));

    if daily_usage_vec.is_empty() {
        if json_output {
            println!(
                r#"{{"status": "success", "message": "No daily usage data found matching filters", "data": []}}"#
            );
        } else {
            println!("No daily usage data found matching your filters.");
        }
        return Ok(());
    }

    // Convert currencies if needed
    if target_currency != "USD" {
        let currency_converter = CurrencyConverter::new();

        // Convert all USD amounts to target currency
        for daily in &mut daily_usage_vec {
            match currency_converter
                .convert_from_usd(daily.total_cost_usd, target_currency)
                .await
            {
                Ok(converted_cost) => {
                    daily.total_cost_usd = converted_cost;
                }
                Err(e) => {
                    if verbose {
                        if json_output {
                            eprintln!(
                                r#"{{"status": "warning", "message": "Failed to convert currency for {}: {}"}}"#,
                                daily.date, e
                            );
                        } else {
                            eprintln!(
                                "Warning: Failed to convert currency for {}: {}",
                                daily.date, e
                            );
                        }
                    }
                    // Keep USD amounts if conversion fails
                }
            }
        }
    }

    // Wrap in our display wrapper after currency conversion
    let daily_usage_list = DailyUsageList(daily_usage_vec);

    // Display results
    if json_output {
        match daily_usage_list.to_json() {
            Ok(json) => println!("{json}"),
            Err(e) => {
                println!(r#"{{"status": "error", "message": "Failed to serialize results: {e}"}}"#);
                std::process::exit(1);
            }
        }
    } else {
        println!(
            "{}",
            daily_usage_list.to_table_with_currency_and_color(
                target_currency,
                decimal_places,
                colored
            )
        );
    }
    Ok(())
}
