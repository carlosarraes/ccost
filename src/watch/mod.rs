// Real-time watch mode for monitoring Claude usage
pub mod dashboard;
pub mod events;
pub mod file_watcher;
pub mod session;
pub mod watch_mode;
pub mod simple_watch;
pub mod text_selection;

pub use dashboard::{Dashboard, DashboardState};
pub use events::{WatchEvent, FileEvent, EfficiencyLevel};
pub use file_watcher::FileWatcher;
pub use session::{SessionTracker, SessionState, SessionStatistics};
pub use watch_mode::WatchMode;
pub use simple_watch::SimpleWatchMode;
pub use text_selection::{TextSelection, TextSelectionHandler};