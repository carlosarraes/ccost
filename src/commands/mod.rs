// Command handlers module
pub mod config;
pub mod conversations;
pub mod pricing;
pub mod projects;
pub mod usage;
pub mod watch;

// Re-export command handlers for easy access
pub use config::handle_config_action;
pub use conversations::{handle_conversations_command, handle_optimize_command};
pub use pricing::handle_pricing_command;
pub use projects::handle_projects_command;
pub use usage::handle_usage_command;
pub use watch::handle_watch_command;
