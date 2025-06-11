// Core watch mode implementation
use anyhow::{Result, Context};
use chrono::Utc;
use std::path::PathBuf;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

use crate::config::Config;
use crate::parser::jsonl::JsonlParser;
use crate::parser::deduplication::DeduplicationEngine;
use crate::models::pricing::PricingManager;
use crate::models::currency::CurrencyConverter;
use crate::watch::{
    Dashboard, DashboardState, FileWatcher, FileEvent, WatchEvent, SessionTracker
};

pub struct WatchMode {
    config: Config,
    pricing_manager: PricingManager,
    currency_converter: CurrencyConverter,
    parser: JsonlParser,
    dashboard: Dashboard,
    file_watcher: Option<FileWatcher>,
    event_receiver: mpsc::UnboundedReceiver<FileEvent>,
    event_sender: mpsc::UnboundedSender<FileEvent>,
    projects_dir: PathBuf,
    project_filter: Option<String>,
    expensive_threshold: f64,
    refresh_rate_ms: u64,
    // Track last processed message count per file to only process new messages
    file_message_counts: HashMap<PathBuf, usize>,
    // Deduplication engine to ensure consistent costs with usage command
    dedup_engine: DeduplicationEngine,
    // Session tracking for current session cost tracking
    session_tracker: SessionTracker,
}

impl WatchMode {
    pub fn new(
        config: Config,
        project_filter: Option<String>,
        expensive_threshold: f64,
        refresh_rate_ms: u64,
    ) -> Result<Self> {
        // Create database
        let db_path = dirs::config_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap())
            .join("ccost")
            .join("cache.db");
        let database1 = crate::storage::sqlite::Database::new(&db_path)?;
        let database2 = crate::storage::sqlite::Database::new(&db_path)?;
        
        let pricing_manager = PricingManager::with_database(database1);
        let currency_converter = CurrencyConverter::new(database2, config.currency.cache_ttl_hours);
        
        // Get projects directory from config or default
        let projects_dir = if config.general.claude_projects_path.starts_with("~/") {
            // Expand tilde to home directory
            if let Some(home_dir) = dirs::home_dir() {
                home_dir.join(&config.general.claude_projects_path[2..])
            } else {
                PathBuf::from(&config.general.claude_projects_path)
            }
        } else {
            PathBuf::from(&config.general.claude_projects_path)
        };
        
        let parser = JsonlParser::new(projects_dir.clone());
        let dashboard = Dashboard::new(expensive_threshold, refresh_rate_ms)?;
        
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        let dedup_engine = DeduplicationEngine::new();
        let session_tracker = SessionTracker::new(30, expensive_threshold); // 30 minute idle timeout
        
        Ok(WatchMode {
            config,
            pricing_manager,
            currency_converter,
            parser,
            dashboard,
            file_watcher: None,
            event_receiver,
            event_sender,
            projects_dir,
            project_filter,
            expensive_threshold,
            refresh_rate_ms,
            file_message_counts: HashMap::new(),
            dedup_engine,
            session_tracker,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        use tokio::time::{interval, Duration};
        
        // Start file watcher
        self.start_file_watcher().await?;
        
        // Create a channel for sending watch events to dashboard
        let (watch_event_sender, watch_event_receiver) = tokio::sync::mpsc::unbounded_channel();
        
        // Create a channel for receiving reset events from dashboard
        let (reset_sender, mut reset_receiver) = tokio::sync::mpsc::unbounded_channel();
        
        // We'll handle both file events and dashboard updates in this main loop
        let mut file_event_receiver = std::mem::replace(&mut self.event_receiver, tokio::sync::mpsc::unbounded_channel().1);
        let mut refresh_timer = interval(Duration::from_millis(self.refresh_rate_ms));
        
        // Start the dashboard in a separate task
        let mut dashboard = std::mem::replace(&mut self.dashboard, Dashboard::new(0.1, 200)?);
        let mut dashboard_task = tokio::spawn(async move {
            dashboard.run_with_events_and_reset(watch_event_receiver, reset_sender).await
        });
        
        // Main event loop
        loop {
            tokio::select! {
                // Handle file events
                Some(file_event) = file_event_receiver.recv() => {
                    match file_event {
                        FileEvent::FileModified(path) | FileEvent::FileCreated(path) => {
                            // Process the file and generate watch events
                            if let Ok(watch_events) = self.process_file_change(path).await {
                                for event in watch_events {
                                    let _ = watch_event_sender.send(event);
                                }
                            }
                        }
                        FileEvent::Error(err) => {
                            eprintln!("File watcher error: {}", err);
                        }
                    }
                }
                
                // Handle reset events from dashboard
                Some(_) = reset_receiver.recv() => {
                    // Reset file tracking when user presses 'r' in dashboard
                    self.reset_file_tracking();
                }
                
                // Refresh timer (for any periodic tasks)
                _ = refresh_timer.tick() => {
                    // Check for idle sessions and mark them as ended
                    let ended_sessions = self.session_tracker.check_idle_sessions();
                    for session in ended_sessions {
                        let project = session.project.clone();
                        let session_end_event = WatchEvent::SessionEnd {
                            project,
                            duration: session.duration(),
                            total_cost: session.total_cost,
                            timestamp: Utc::now(),
                        };
                        let _ = watch_event_sender.send(session_end_event);
                    }
                }
                
                // Check if dashboard task completed (user quit)
                result = &mut dashboard_task => {
                    match result {
                        Ok(Ok(())) => break, // Clean exit
                        Ok(Err(e)) => return Err(e), // Dashboard error
                        Err(e) => return Err(anyhow::anyhow!("Dashboard task panicked: {}", e)), // Task panic
                    }
                }
            }
        }

        Ok(())
    }

    async fn start_file_watcher(&mut self) -> Result<()> {
        if !self.projects_dir.exists() {
            return Err(anyhow::anyhow!(
                "Projects directory does not exist: {}. Make sure Claude is installed and has created some conversation files.",
                self.projects_dir.display()
            ));
        }

        let (file_watcher, file_receiver) = FileWatcher::new(
            self.projects_dir.clone(),
            self.event_sender.clone(),
        )?;

        // Spawn the file watcher task
        let event_sender = self.event_sender.clone();
        tokio::spawn(async move {
            if let Err(e) = file_watcher.run_with_receiver(file_receiver).await {
                let _ = event_sender.send(FileEvent::Error(format!("File watcher error: {}", e)));
            }
        });

        Ok(())
    }

    async fn handle_file_event(&mut self, event: FileEvent) -> Result<()> {
        match event {
            FileEvent::FileModified(path) | FileEvent::FileCreated(path) => {
                self.process_file_change(path).await?;
            }
            FileEvent::Error(error) => {
                eprintln!("File watcher error: {}", error);
            }
        }
        Ok(())
    }

    async fn process_file_change(&mut self, file_path: PathBuf) -> Result<Vec<WatchEvent>> {
        // Parse the file and look for new messages
        let parsed_conversation = self.parser.parse_file_with_verbose(&file_path, false)?;
        
        // Extract smart project name from cwd/originalCwd, fallback to directory extraction
        let project = parsed_conversation.smart_project_name
            .unwrap_or_else(|| self.extract_project_name(&file_path).unwrap_or_else(|_| "unknown".to_string()));
        
        // Apply project filter if specified
        if let Some(ref filter) = self.project_filter {
            if project != *filter {
                return Ok(vec![]);
            }
        }
        
        // CRITICAL FIX: Apply deduplication engine to match usage command behavior
        let usage_data = self.dedup_engine.filter_duplicates(parsed_conversation.messages)?;
        
        let mut watch_events = Vec::new();
        
        for data in usage_data {
            // Process each message and generate watch events
            let tokens = data.usage.as_ref()
                .map(|u| u.input_tokens.unwrap_or(0) + u.output_tokens.unwrap_or(0) + u.cache_creation_input_tokens.unwrap_or(0) + u.cache_read_input_tokens.unwrap_or(0))
                .unwrap_or(0);

            let cost = if let Some(cost_usd) = data.cost_usd {
                // Convert to user's preferred currency
                let currency = &self.config.currency.default_currency;
                if currency == "USD" {
                    cost_usd
                } else {
                    self.currency_converter.convert_from_usd(cost_usd, currency).await
                        .unwrap_or(cost_usd)
                }
            } else {
                // Calculate cost using pricing manager
                if let Some(usage) = &data.usage {
                    let model = data.message.as_ref()
                        .and_then(|m| m.model.as_deref())
                        .unwrap_or("claude-3-sonnet-20240229");
                    let pricing = self.pricing_manager.get_pricing_with_fallback(model);
                    
                    let total_cost_usd = pricing.calculate_cost(
                        usage.input_tokens.unwrap_or(0),
                        usage.output_tokens.unwrap_or(0),
                        usage.cache_creation_input_tokens.unwrap_or(0),
                        usage.cache_read_input_tokens.unwrap_or(0),
                    );
                    
                    // Convert to user's preferred currency
                    let currency = &self.config.currency.default_currency;
                    if currency == "USD" {
                        total_cost_usd
                    } else {
                        self.currency_converter.convert_from_usd(total_cost_usd, currency).await
                            .unwrap_or(total_cost_usd)
                    }
                } else {
                    0.0
                }
            };

            let model = data.message.as_ref()
                .and_then(|m| m.model.clone())
                .unwrap_or_else(|| "unknown".to_string());
            let timestamp = Utc::now(); // Use current time since we're processing in real-time

            // Create and collect watch event
            let watch_event = WatchEvent::NewMessage {
                tokens: tokens as u32,
                cost,
                model: model.clone(),
                project: project.clone(),
                timestamp,
            };

            watch_events.push(watch_event);

            // Update session tracking
            let is_new_session = self.session_tracker.update_activity(&project, tokens as u32, cost, &model);
            if is_new_session {
                let session_start_event = WatchEvent::SessionStart {
                    project: project.clone(),
                    timestamp,
                };
                watch_events.push(session_start_event);
            }

            // Check for expensive conversations
            if cost > self.expensive_threshold {
                let expensive_event = WatchEvent::ExpensiveConversation {
                    cost,
                    threshold: self.expensive_threshold,
                    project: project.clone(),
                    timestamp,
                };
                watch_events.push(expensive_event);
            }

            // Check for cache hits
            if let Some(usage) = &data.usage {
                if usage.cache_read_input_tokens.unwrap_or(0) > 0 {
                    let saved_tokens = usage.cache_read_input_tokens.unwrap_or(0);
                    let pricing = self.pricing_manager.get_pricing_with_fallback(&model);
                    // Calculate how much we saved by using cache instead of full input processing
                    let saved_cost = (saved_tokens as f64 / 1_000_000.0) * (pricing.input_cost_per_mtok - pricing.cache_cost_per_mtok);

                    let cache_event = WatchEvent::CacheHit {
                        saved_tokens: saved_tokens as u32,
                        saved_cost,
                        model: model.clone(),
                        project: project.clone(),
                        timestamp,
                    };
                    watch_events.push(cache_event);
                }
            }
        }

        Ok(watch_events)
    }

    fn extract_project_name(&self, file_path: &PathBuf) -> Result<String> {
        // Extract project name from path like ~/.claude/projects/PROJECT_NAME/file.jsonl
        let path_str = file_path.to_string_lossy();
        
        if let Some(projects_pos) = path_str.find("/projects/") {
            let after_projects = &path_str[projects_pos + 10..]; // "/projects/".len() = 10
            if let Some(slash_pos) = after_projects.find('/') {
                let project_name = &after_projects[..slash_pos];
                Ok(project_name.to_string())
            } else {
                // File is directly in projects directory
                Ok(after_projects.to_string())
            }
        } else {
            // Fallback: use parent directory name
            file_path.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow::anyhow!("Could not extract project name from path: {}", path_str))
        }
    }

    pub fn set_project_filter(&mut self, project: Option<String>) {
        self.project_filter = project;
    }

    pub fn set_expensive_threshold(&mut self, threshold: f64) {
        self.expensive_threshold = threshold;
    }
    
    pub fn reset_file_tracking(&mut self) {
        self.file_message_counts.clear();
        // Also reset deduplication engine to match user expectation
        let _ = self.dedup_engine.clear_history();
    }

    /// Get current active session costs
    pub fn get_active_sessions(&self) -> Vec<crate::watch::session::SessionState> {
        self.session_tracker.get_active_sessions().into_iter().cloned().collect()
    }

    /// Get total cost of all active sessions
    pub fn get_total_session_cost(&self) -> f64 {
        self.session_tracker.get_active_sessions()
            .iter()
            .map(|s| s.total_cost)
            .sum()
    }

    pub fn get_dashboard_state(&self) -> &DashboardState {
        // This would need to be implemented if we want to expose dashboard state
        // For now, the dashboard manages its own state internally
        todo!("Dashboard state access not implemented")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[tokio::test]
    async fn test_watch_mode_creation() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.general.claude_projects_path = temp_dir.path().join("projects").to_string_lossy().to_string();
        
        let watch_mode = WatchMode::new(config, None, 0.10, 200);
        assert!(watch_mode.is_ok());
    }

    #[test]
    fn test_extract_project_name() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.general.claude_projects_path = temp_dir.path().join("projects").to_string_lossy().to_string();
        let watch_mode = WatchMode::new(config, None, 0.10, 200).unwrap();
        
        let path = PathBuf::from("/home/user/.claude/projects/my-project/conversation.jsonl");
        let project = watch_mode.extract_project_name(&path).unwrap();
        assert_eq!(project, "my-project");
        
        let path2 = PathBuf::from("/home/user/.claude/projects/another-project-name/subdir/file.jsonl");
        let project2 = watch_mode.extract_project_name(&path2).unwrap();
        assert_eq!(project2, "another-project-name");
    }

    #[tokio::test]
    async fn test_project_filter() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.general.claude_projects_path = temp_dir.path().join("projects").to_string_lossy().to_string();
        let mut watch_mode = WatchMode::new(config, Some("test-project".to_string()), 0.10, 200).unwrap();
        
        // Test that project filter is set correctly
        assert_eq!(watch_mode.project_filter, Some("test-project".to_string()));
        
        // Test changing the filter
        watch_mode.set_project_filter(Some("new-project".to_string()));
        assert_eq!(watch_mode.project_filter, Some("new-project".to_string()));
        
        // Test clearing the filter
        watch_mode.set_project_filter(None);
        assert_eq!(watch_mode.project_filter, None);
    }

    #[test]
    fn test_threshold_setting() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.general.claude_projects_path = temp_dir.path().join("projects").to_string_lossy().to_string();
        let mut watch_mode = WatchMode::new(config, None, 0.10, 200).unwrap();
        
        assert_eq!(watch_mode.expensive_threshold, 0.10);
        
        watch_mode.set_expensive_threshold(0.25);
        assert_eq!(watch_mode.expensive_threshold, 0.25);
    }
    
    #[tokio::test]
    async fn test_deduplication_engine_integration() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.general.claude_projects_path = temp_dir.path().join("projects").to_string_lossy().to_string();
        let mut watch_mode = WatchMode::new(config, None, 0.10, 200).unwrap();
        
        // Create a test JSONL file with duplicate messages
        let projects_dir = temp_dir.path().join("projects");
        let project_dir = projects_dir.join("test-project");
        fs::create_dir_all(&project_dir).unwrap();
        
        let test_file = project_dir.join("conversation.jsonl");
        let content = r#"{"uuid":"msg1","requestId":"req1","message":{"content":"Hello","model":"claude-sonnet-4"},"usage":{"inputTokens":10,"outputTokens":20},"costUSD":0.001}
{"uuid":"msg2","requestId":"req2","message":{"content":"World","model":"claude-sonnet-4"},"usage":{"inputTokens":15,"outputTokens":25},"costUSD":0.002}
{"uuid":"msg1","requestId":"req1","message":{"content":"Hello","model":"claude-sonnet-4"},"usage":{"inputTokens":10,"outputTokens":20},"costUSD":0.001}"#;
        
        fs::write(&test_file, content).unwrap();
        
        // Process the file
        let events = watch_mode.process_file_change(test_file).await.unwrap();
        
        // Should only process 2 unique messages (duplicate removed)
        assert_eq!(events.len(), 2);
        
        // Total cost should be 0.003 (0.001 + 0.002), not 0.004 (with duplicate)
        let total_cost: f64 = events.iter().map(|e| {
            match e {
                WatchEvent::NewMessage { cost, .. } => *cost,
                _ => 0.0,
            }
        }).sum();
        
        assert!((total_cost - 0.003).abs() < 0.0001, "Expected 0.003, got {}", total_cost);
    }
}