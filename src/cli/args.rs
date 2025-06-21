use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ccost")]
#[command(about = "Claude Cost Tracking Tool")]
#[command(version)]
pub struct Cli {
    /// Custom config file path
    #[arg(long, global = true)]
    pub config: Option<String>,

    /// Override currency (EUR, GBP, JPY, etc.)
    #[arg(long, global = true)]
    pub currency: Option<String>,

    /// Override timezone
    #[arg(long, global = true)]
    pub timezone: Option<String>,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// JSON output format
    #[arg(long, global = true)]
    pub json: bool,

    /// Enable colorized table output
    #[arg(long, global = true)]
    pub colored: bool,

    /// Use dummy project names for privacy in screenshots
    #[arg(short = 'd', long, global = true)]
    pub hidden: bool,

    /// Filter by model
    #[arg(long, global = true)]
    pub model: Option<String>,

    /// Start date (YYYY-MM-DD)
    #[arg(long, global = true)]
    pub since: Option<String>,

    /// End date (YYYY-MM-DD)
    #[arg(long, global = true)]
    pub until: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum ConfigAction {
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
pub enum Commands {
    /// Show today's usage
    Today {
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,
    },

    /// Show yesterday's usage
    Yesterday {
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,
    },

    /// Show this week's usage
    #[command(name = "this-week")]
    ThisWeek {
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,
    },

    /// Show this month's usage
    #[command(name = "this-month")]
    ThisMonth {
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,
    },

    /// Show daily usage for specified number of days
    Daily {
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,

        /// Number of days to show
        #[arg(long, default_value = "7")]
        days: u32,
    },

    /// Show project usage (all projects or specific projects)
    Projects {
        /// Project names to analyze (comma-separated, optional)
        projects: Option<String>,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}
