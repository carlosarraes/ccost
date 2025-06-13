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

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum UsageTimeframe {
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
    /// Show daily usage for specified number of days
    Daily {
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,

        /// Filter by model
        #[arg(long)]
        model: Option<String>,

        /// Number of days to show
        #[arg(long, default_value = "7")]
        days: u32,
    },
}

#[derive(Subcommand)]
pub enum ProjectSort {
    /// Sort by project name
    Name,
    /// Sort by total cost
    Cost,
    /// Sort by total tokens
    Tokens,
}

#[derive(Subcommand)]
pub enum ConversationSort {
    /// Sort by cost (highest first)
    Cost,
    /// Sort by efficiency score (highest first)
    Efficiency,
    /// Sort by duration (longest first)
    Duration,
    /// Sort by token count (highest first)
    Tokens,
    /// Sort by message count (highest first)
    Messages,
    /// Sort by start time (newest first)
    StartTime,
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
    /// Show usage analysis
    Usage {
        #[command(subcommand)]
        timeframe: Option<UsageTimeframe>,

        /// Filter by project name
        #[arg(long)]
        project: Option<String>,

        /// Filter by model
        #[arg(long)]
        model: Option<String>,

        /// Start date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// End date (YYYY-MM-DD)
        #[arg(long)]
        until: Option<String>,
    },

    /// List and analyze projects
    Projects {
        /// Sort projects by criteria
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

    /// Real-time watch mode
    Watch {
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,

        /// Expensive conversation threshold in USD
        #[arg(long, default_value = "5.0")]
        threshold: f64,

        /// Disable charts and graphs
        #[arg(long)]
        no_charts: bool,

        /// Refresh rate in milliseconds
        #[arg(long, default_value = "1000")]
        refresh_rate: u64,
    },

    /// Conversation analysis
    Conversations {
        /// Sort conversations by criteria
        #[command(subcommand)]
        sort_by: Option<ConversationSort>,

        /// Filter by project name
        #[arg(long)]
        project: Option<String>,

        /// Filter by model
        #[arg(long)]
        model: Option<String>,

        /// Start date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// End date (YYYY-MM-DD)
        #[arg(long)]
        until: Option<String>,

        /// Export conversations to file
        #[arg(long)]
        export: Option<String>,

        /// Minimum cost threshold
        #[arg(long)]
        min_cost: Option<f64>,

        /// Maximum cost threshold
        #[arg(long)]
        max_cost: Option<f64>,

        /// Show only conversations with outliers/issues
        #[arg(long)]
        outliers_only: bool,

        /// Minimum efficiency score (0-100)
        #[arg(long)]
        min_efficiency: Option<f32>,

        /// Maximum efficiency score (0-100)
        #[arg(long)]
        max_efficiency: Option<f32>,
    },

    /// Usage optimization analysis
    Optimize {
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,

        /// Filter by model
        #[arg(long)]
        model: Option<String>,

        /// Start date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// End date (YYYY-MM-DD)
        #[arg(long)]
        until: Option<String>,

        /// Show only potential savings (no detailed recommendations)
        #[arg(long)]
        potential_savings: bool,

        /// Export format for recommendations
        #[arg(long)]
        export: Option<String>,

        /// Minimum confidence threshold (0.0-1.0)
        #[arg(long)]
        confidence_threshold: Option<f32>,

        /// Filter recommendations from this model
        #[arg(long)]
        model_from: Option<String>,

        /// Filter recommendations to this model
        #[arg(long)]
        model_to: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum PricingAction {
    /// List current model pricing
    List,
    /// Set model pricing
    Set {
        /// Model name
        model: String,
        /// Input tokens price per 1M tokens
        input_price: f64,
        /// Output tokens price per 1M tokens
        output_price: f64,
    },
    /// Update pricing from GitHub repository
    Update {
        /// Update from community GitHub repository
        #[arg(long)]
        github: bool,
    },
}
