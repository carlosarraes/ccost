// Command handlers module
pub mod config;
pub mod projects;
pub mod usage;

// Re-export command handlers for easy access (some temporarily unused)
#[allow(unused)]
pub use config::handle_config_action;
#[allow(unused)]
#[allow(unused)]
pub use projects::handle_projects_command;
#[allow(unused)]
pub use usage::handle_usage_command;
