// Command handlers module
pub mod config;
pub mod conversations;
pub mod pricing;
pub mod projects;
pub mod usage;
pub mod watch;

// Re-export command handlers for easy access (some temporarily unused)
#[allow(unused)]
pub use config::handle_config_action;
#[allow(unused)]
pub use conversations::{handle_conversations_command, handle_optimize_command};
#[allow(unused)]
pub use pricing::handle_pricing_command;
#[allow(unused)]
pub use projects::handle_projects_command;
#[allow(unused)]
pub use usage::handle_usage_command;
#[allow(unused)]
pub use watch::handle_watch_command;
