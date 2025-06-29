// Yesterday's usage command
use crate::analysis::UsageFilter;
use crate::commands::timeframe_utils::{TimeframeContext, UsageTimeframe, handle_error};
use crate::utils::{apply_usage_filters, print_filter_info, resolve_filters};

pub async fn handle_yesterday_command(
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
    // Initialize context
    let mut context =
        match TimeframeContext::new(timezone_name, daily_cutoff_hour, date_format).await {
            Ok(ctx) => ctx,
            Err(e) => {
                handle_error(&e, json_output);
                return Err(e);
            }
        };

    // Parse timeframe into date filters
    let (final_project, final_since, final_until, final_model) = resolve_filters(
        Some(UsageTimeframe::Yesterday),
        project,
        since,
        until,
        model,
        &context.timezone_calc,
    );

    // Create usage filter
    let usage_filter = UsageFilter {
        project_name: final_project.clone(),
        model_name: final_model.clone(),
        since: final_since,
        until: final_until,
    };

    if verbose {
        print_filter_info(&usage_filter, json_output, &context.date_formatter);
    }

    // Process JSONL files
    let all_usage_data =
        match context.process_jsonl_files(final_project, verbose, json_output, hidden) {
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

    // Convert enhanced data to tuple format
    let usage_tuples: Vec<(crate::parser::jsonl::UsageData, String)> = all_usage_data
        .into_iter()
        .map(|enhanced| (enhanced.usage_data, enhanced.project_name))
        .collect();

    // Calculate usage with enhanced pricing (supports live pricing)
    let (project_usage, pricing_source) = match context
        .calculate_usage_enhanced(usage_tuples, &usage_filter)
        .await
    {
        Ok((usage, source)) => (usage, source),
        Err(e) => {
            handle_error(&e, json_output);
            return Err(e);
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
    if let Err(e) = context
        .convert_currency(&mut filtered_usage, target_currency, verbose, json_output)
        .await
    {
        handle_error(&e, json_output);
        return Err(e);
    }

    // Display results
    context.display_results(
        &filtered_usage,
        target_currency,
        decimal_places,
        json_output,
        colored,
    )
}
