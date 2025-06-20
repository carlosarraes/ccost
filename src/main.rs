// ccost: Claude Cost Tracking Tool
//
// No additional analysis imports needed in main.rs
use clap::Parser;
use config::Config;

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

// Import CLI types and commands
use cli::args::{Cli, Commands};
use commands::config::handle_config_action;
use commands::projects::handle_projects_command;
use commands::usage::{handle_usage_command, UsageTimeframe};
use commands::today::handle_today_command;
use commands::yesterday::handle_yesterday_command;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load configuration
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error: Failed to load configuration: {e}");
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
        Some(Commands::Today { project }) => {
            handle_today_command(
                project,
                cli.since.clone(),
                cli.until.clone(),
                cli.model.clone(),
                target_currency,
                config.output.decimal_places,
                cli.json,
                cli.verbose,
                colored,
                cli.hidden,
                &config.timezone.timezone,
                config.timezone.daily_cutoff_hour,
                &config.output.date_format,
            )
            .await?;
        }
        Some(Commands::Yesterday { project }) => {
            handle_yesterday_command(
                project,
                cli.since.clone(),
                cli.until.clone(),
                cli.model.clone(),
                target_currency,
                config.output.decimal_places,
                cli.json,
                cli.verbose,
                colored,
                cli.hidden,
                &config.timezone.timezone,
                config.timezone.daily_cutoff_hour,
                &config.output.date_format,
            )
            .await?;
        }
        Some(Commands::ThisWeek { project }) => {
            handle_usage_command(
                Some(UsageTimeframe::ThisWeek),
                project.or(cli.model.clone()),
                cli.since.clone(),
                cli.until.clone(),
                cli.model.clone(),
                target_currency,
                config.output.decimal_places,
                cli.json,
                cli.verbose,
                colored,
                cli.hidden,
                &config.timezone.timezone,
                config.timezone.daily_cutoff_hour,
                &config.output.date_format,
            )
            .await?;
        }
        Some(Commands::ThisMonth { project }) => {
            handle_usage_command(
                Some(UsageTimeframe::ThisMonth),
                project.or(cli.model.clone()),
                cli.since.clone(),
                cli.until.clone(),
                cli.model.clone(),
                target_currency,
                config.output.decimal_places,
                cli.json,
                cli.verbose,
                colored,
                cli.hidden,
                &config.timezone.timezone,
                config.timezone.daily_cutoff_hour,
                &config.output.date_format,
            )
            .await?;
        }
        Some(Commands::Daily { project, days }) => {
            handle_usage_command(
                Some(UsageTimeframe::Daily { days }),
                project.or(cli.model.clone()),
                cli.since.clone(),
                cli.until.clone(),
                cli.model.clone(),
                target_currency,
                config.output.decimal_places,
                cli.json,
                cli.verbose,
                colored,
                cli.hidden,
                &config.timezone.timezone,
                config.timezone.daily_cutoff_hour,
                &config.output.date_format,
            )
            .await?;
        }
        Some(Commands::Projects { projects }) => {
            handle_projects_command(
                projects,
                target_currency,
                config.output.decimal_places,
                cli.json,
                cli.verbose,
                colored,
                cli.hidden,
            )
            .await?;
        }
        Some(Commands::Config { action }) => {
            handle_config_action(action, cli.json);
        }
        None => {
            // Default behavior: show overall usage summary
            handle_usage_command(
                None,
                cli.model.clone(),
                cli.since.clone(),
                cli.until.clone(),
                cli.model.clone(),
                target_currency,
                config.output.decimal_places,
                cli.json,
                cli.verbose,
                colored,
                cli.hidden,
                &config.timezone.timezone,
                config.timezone.daily_cutoff_hour,
                &config.output.date_format,
            )
            .await?;
        }
    }
    Ok(())
}

