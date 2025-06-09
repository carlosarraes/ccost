// ccost: Claude Cost Tracking Tool
use clap::{Parser, Subcommand};
use config::Config;
use models::{PricingManager, ModelPricing};
use storage::Database;

// Module declarations
mod config;
mod parser;
mod storage;
mod models;
mod analysis;
mod output;
mod sync;

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

fn main() {
    let cli = Cli::parse();
    
    // Handle global options
    if cli.verbose {
        println!("Verbose mode enabled");
    }
    
    if cli.json {
        println!("JSON output mode enabled");
    }
    
    if let Some(config_path) = &cli.config {
        println!("Using config file: {}", config_path);
    }
    
    if let Some(currency) = &cli.currency {
        println!("Currency override: {}", currency);
    }
    
    if let Some(timezone) = &cli.timezone {
        println!("Timezone override: {}", timezone);
    }
    
    match cli.command {
        Commands::Usage { 
            timeframe,
            project, 
            since, 
            until, 
            model
        } => {
            println!("Usage analysis");
            
            match timeframe {
                Some(UsageTimeframe::Today { project: tf_project, model: tf_model }) => {
                    println!("  Showing today's usage");
                    if let Some(proj) = tf_project {
                        println!("  Project filter: {}", proj);
                    }
                    if let Some(m) = tf_model {
                        println!("  Model filter: {}", m);
                    }
                },
                Some(UsageTimeframe::Yesterday { project: tf_project, model: tf_model }) => {
                    println!("  Showing yesterday's usage");
                    if let Some(proj) = tf_project {
                        println!("  Project filter: {}", proj);
                    }
                    if let Some(m) = tf_model {
                        println!("  Model filter: {}", m);
                    }
                },
                Some(UsageTimeframe::ThisWeek { project: tf_project, model: tf_model }) => {
                    println!("  Showing this week's usage");
                    if let Some(proj) = tf_project {
                        println!("  Project filter: {}", proj);
                    }
                    if let Some(m) = tf_model {
                        println!("  Model filter: {}", m);
                    }
                },
                Some(UsageTimeframe::ThisMonth { project: tf_project, model: tf_model }) => {
                    println!("  Showing this month's usage");
                    if let Some(proj) = tf_project {
                        println!("  Project filter: {}", proj);
                    }
                    if let Some(m) = tf_model {
                        println!("  Model filter: {}", m);
                    }
                },
                None => {
                    println!("  Showing all usage (use date filters or timeframe subcommand)");
                    
                    if let Some(proj) = project {
                        println!("  Project filter: {}", proj);
                    }
                    
                    if let Some(start) = since {
                        println!("  Since: {}", start);
                    }
                    
                    if let Some(end) = until {
                        println!("  Until: {}", end);
                    }
                    
                    if let Some(m) = model {
                        println!("  Model filter: {}", m);
                    }
                },
            }
            
            println!("  TODO: Implement in TASK-011");
        }
        Commands::Projects { sort_by } => {
            println!("Projects analysis");
            
            match sort_by {
                Some(ProjectSort::Cost) => println!("  Sorted by cost"),
                Some(ProjectSort::Tokens) => println!("  Sorted by tokens"),
                None => println!("  Default sorting (by name)"),
            }
            
            println!("  TODO: Implement in TASK-012");
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