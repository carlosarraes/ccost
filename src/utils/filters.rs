use crate::analysis::UsageFilter;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use crate::cli::args::UsageTimeframe;

/// Helper structure to associate usage data with project name
#[derive(Debug, Clone)]
pub struct EnhancedUsageData {
    pub usage_data: crate::parser::jsonl::UsageData,
    pub project_name: String,
}

/// Resolves timeframe and explicit filters into final filter values
pub fn resolve_filters(
    timeframe: Option<UsageTimeframe>,
    project: Option<String>,
    since: Option<String>,
    until: Option<String>,
    model: Option<String>,
    timezone_calc: &crate::analysis::TimezoneCalculator,
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

/// Prints filter information for verbose mode
pub fn print_filter_info(filter: &UsageFilter, json_output: bool) {
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

/// Applies usage filters to project usage data
pub fn apply_usage_filters(
    usage: Vec<crate::analysis::usage::ProjectUsage>,
    filter: &UsageFilter,
) -> Vec<crate::analysis::usage::ProjectUsage> {
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
                    crate::analysis::usage::ModelUsage,
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