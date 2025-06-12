// Command handlers module
pub mod usage;
pub mod projects;
pub mod config;
pub mod pricing;
pub mod watch;
pub mod conversations;

// Re-export command handlers for easy access
pub use usage::handle_usage_command;
pub use projects::handle_projects_command;
pub use config::handle_config_action;
pub use pricing::handle_pricing_command;
pub use watch::handle_watch_command;
pub use conversations::{handle_conversations_command, handle_optimize_command};