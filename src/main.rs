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
mod watch;

// Import CLI types and commands
use cli::args::{Cli, Commands};
use commands::config::handle_config_action;
use commands::conversations::{handle_conversations_command, handle_optimize_command};
use commands::projects::handle_projects_command;
use commands::usage::handle_usage_command;
use commands::watch::handle_watch_command;

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
                &config.output.date_format,
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
            max_cost: _,
            outliers_only: _,
            min_efficiency: _,
            max_efficiency: _,
            export,
        } => {
            handle_conversations_command(
                sort_by,
                project,
                model,
                since,
                until,
                export,
                min_cost,
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
        Commands::Optimize {
            project,
            model,
            since,
            until,
            potential_savings: _,
            export: _,
            confidence_threshold: _,
            model_from: _,
            model_to: _,
        } => {
            handle_optimize_command(
                project,
                model,
                since,
                until,
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
        Commands::Config { action } => {
            handle_config_action(action, cli.json);
        }
        Commands::Pricing { action: _ } => {
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
#[allow(unused)]
mod tests {
    use super::*;
    use crate::cli::{ConfigAction, ProjectSort, UsageTimeframe};
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
                _ => assert!(
                    false,
                    "Expected Yesterday timeframe but got different timeframe"
                ),
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
                _ => assert!(
                    false,
                    "Expected ThisWeek timeframe but got different timeframe"
                ),
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
                model,
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
