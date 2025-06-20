// Projects command handler
use crate::analysis::{CostCalculationMode, UsageFilter, UsageTracker};
use crate::config::Config;
use crate::models::PricingManager;
use crate::models::currency::CurrencyConverter;
use crate::output::OutputFormat;
use crate::parser::deduplication::DeduplicationEngine;
use crate::parser::jsonl::JsonlParser;
use crate::utils::{DateFormatter, EnhancedUsageData, apply_usage_filters, maybe_hide_project_name};
use std::collections::HashSet;
use std::path::PathBuf;

pub async fn handle_projects_command(
    projects: Option<String>,
    target_currency: &str,
    decimal_places: u8,
    json_output: bool,
    verbose: bool,
    colored: bool,
    hidden: bool,
) -> anyhow::Result<()> {
    // Load config for timezone and date format settings
    let config = Config::load().unwrap_or_default();
    
    // Parse comma-separated project names
    let project_filters: Option<HashSet<String>> = projects.map(|p| {
        p.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    });

    // Initialize date formatter
    let _date_formatter = match DateFormatter::new(&config.output.date_format) {
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

    let projects_dir = if config.general.claude_projects_path.starts_with("~/") {
        // Expand tilde to home directory
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
    let mut dedup_engine = DeduplicationEngine::new();

    // Create usage filter for other filtering (no project filter here since we handle it manually)
    let usage_filter = UsageFilter {
        project_name: None, // We'll filter projects manually
        model_name: None,
        since: None,
        until: None,
    };

    if verbose && !json_output {
        if let Some(ref filters) = project_filters {
            println!("Filtering projects: {}", filters.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
        } else {
            println!("Showing all projects");
        }
        println!("Searching for JSONL files in: {}", projects_dir.display());
    }

    let jsonl_files = match parser.find_jsonl_files() {
        Ok(files) => files,
        Err(e) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Failed to find JSONL files: {e}"}}"#
                );
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
                if let Some(ref filter_projects) = project_filters {
                    if !filter_projects.contains(&raw_project_name) {
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
    let (project_usage, pricing_source) = match usage_tracker.calculate_usage_with_projects_filtered_enhanced(
        usage_tuples,
        &mut pricing_manager,
        &usage_filter,
    ).await {
        Ok((usage, source)) => (usage, source),
        Err(e) => {
            if json_output {
                println!(
                    r#"{{"status": "error", "message": "Failed to calculate usage: {e}"}}"#
                );
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
                println!(
                    r#"{{"status": "error", "message": "Failed to serialize results: {e}"}}"#
                );
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
