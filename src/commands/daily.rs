// Daily usage breakdown command
use crate::commands::timeframe_utils::{TimeframeContext, handle_error};
use crate::analysis::DailyUsageList;
use crate::models::currency::CurrencyConverter;
use crate::output::OutputFormat;
use crate::utils::{DateFormatter, EnhancedUsageData};
use chrono::Utc;
use std::collections::HashMap;

pub async fn handle_daily_command(
    days: u32,
    project_filter: Option<String>,
    _since: Option<String>,
    _until: Option<String>,
    model_filter: Option<String>,
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
    // Initialize context
    let mut context = match TimeframeContext::new(timezone_name, daily_cutoff_hour, date_format).await {
        Ok(ctx) => ctx,
        Err(e) => {
            handle_error(&e, json_output);
            return Err(e);
        }
    };

    // Process JSONL files
    let all_usage_data = match context.process_jsonl_files(project_filter.clone(), verbose, json_output, hidden) {
        Ok(data) => data,
        Err(e) => {
            handle_error(&e, json_output);
            return Err(e);
        }
    };

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
    let daily_usage_list = group_usage_by_day(
        &all_usage_data,
        days,
        model_filter,
        &context.usage_tracker,
        &context.pricing_manager,
        &context.date_formatter,
        json_output,
    )?;

    if daily_usage_list.0.is_empty() {
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
    let mut converted_daily_usage = daily_usage_list;
    if target_currency != "USD" {
        convert_daily_currency(&mut converted_daily_usage, target_currency, verbose, json_output).await?;
    }

    // Display results
    if json_output {
        match converted_daily_usage.to_json() {
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
            converted_daily_usage.to_table_with_currency_and_color(
                target_currency,
                decimal_places,
                colored
            )
        );
    }

    Ok(())
}

fn group_usage_by_day(
    all_usage_data: &[EnhancedUsageData],
    days: u32,
    model_filter: Option<String>,
    usage_tracker: &crate::analysis::UsageTracker,
    pricing_manager: &crate::models::PricingManager,
    date_formatter: &DateFormatter,
    json_output: bool,
) -> anyhow::Result<DailyUsageList> {
    use crate::analysis::DailyUsage;

    let mut daily_usage_map: HashMap<String, DailyUsage> = HashMap::new();

    for enhanced in all_usage_data {
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

        if let Some(ref filter_model) = model_filter {
            if model_name != *filter_model {
                continue;
            }
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
                let output_cost = (output_tokens as f64 / 1_000_000.0) * pricing.output_cost_per_mtok;
                let cache_creation_cost = (cache_creation_tokens as f64 / 1_000_000.0) * pricing.cache_cost_per_mtok;
                let cache_read_cost = (cache_read_tokens as f64 / 1_000_000.0) * pricing.cache_cost_per_mtok;
                input_cost + output_cost + cache_creation_cost + cache_read_cost
            } else {
                0.0
            }
        };

        daily_usage.total_cost_usd += cost;
    }

    // Count projects per day
    let mut project_sets_by_day: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    for enhanced in all_usage_data.iter() {
        if let Some(timestamp_str) = &enhanced.usage_data.timestamp {
            if let Ok(message_time) = usage_tracker.parse_timestamp(timestamp_str) {
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

    Ok(DailyUsageList(daily_usage_vec))
}

async fn convert_daily_currency(
    daily_usage_list: &mut DailyUsageList,
    target_currency: &str,
    verbose: bool,
    json_output: bool,
) -> anyhow::Result<()> {
    let currency_converter = CurrencyConverter::new();

    // Convert all USD amounts to target currency
    for daily in &mut daily_usage_list.0 {
        match currency_converter
            .convert_from_usd(daily.total_cost_usd, target_currency)
            .await
        {
            Ok(converted_cost) => {
                daily.total_cost_usd = converted_cost;
            }
            Err(e) => {
                if verbose {
                    let error_msg = format!("Failed to convert currency for {}: {}", daily.date, e);
                    if json_output {
                        eprintln!(r#"{{"status": "warning", "message": "{}"}}"#, error_msg);
                    } else {
                        eprintln!("Warning: {}", error_msg);
                    }
                }
                // Keep USD amounts if conversion fails
            }
        }
    }

    Ok(())
}