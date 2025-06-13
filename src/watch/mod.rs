// Real-time watch mode for monitoring Claude usage
pub mod dashboard;
pub mod events;
pub mod file_watcher;
pub mod session;
pub mod text_selection;
pub mod watch_mode;

pub use dashboard::{Dashboard, DashboardState};
pub use events::{FileEvent, WatchEvent};
pub use file_watcher::FileWatcher;
pub use session::SessionTracker;
pub use watch_mode::WatchMode;
