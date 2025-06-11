// Core watch mode implementation
use anyhow::{Result, Context};
use chrono::Utc;
use std::path::PathBuf;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tokio::task::JoinHandle;

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
    // File watcher task handle for proper cleanup
    file_watcher_task: Option<JoinHandle<()>>,
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
            file_watcher_task: None,
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
                        Ok(Ok(())) => {
                            // Clean exit - perform cleanup
                            self.cleanup().await;
                            break;
                        }
                        Ok(Err(e)) => {
                            // Dashboard error - cleanup and return error
                            self.cleanup().await;
                            return Err(e);
                        }
                        Err(e) => {
                            // Task panic - cleanup and return error
                            self.cleanup().await;
                            return Err(anyhow::anyhow!("Dashboard task panicked: {}", e));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Cleanup background tasks when exiting watch mode
    async fn cleanup(&mut self) {
        // Cancel the file watcher task if it's running
        if let Some(task_handle) = self.file_watcher_task.take() {
            task_handle.abort();
            // Wait a short time for graceful shutdown
            let _ = tokio::time::timeout(
                std::time::Duration::from_millis(100), 
                task_handle
            ).await;
        }
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

        // Spawn the file watcher task and store handle for cleanup
        let event_sender = self.event_sender.clone();
        let task_handle = tokio::spawn(async move {
            if let Err(e) = file_watcher.run_with_receiver(file_receiver).await {
                let _ = event_sender.send(FileEvent::Error(format!("File watcher error: {}", e)));
            }
        });
        
        // Store the task handle for proper cleanup
        self.file_watcher_task = Some(task_handle);

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
        
        // Use unified project name extraction for consistency
        let project = self.parser.get_unified_project_name(&file_path, &parsed_conversation.messages);
        
        // Apply project filter if specified
        if let Some(ref filter) = self.project_filter {
            if project != *filter {
                return Ok(vec![]);
            }
        }
        
        // CRITICAL FIX: Apply deduplication engine to match usage command behavior
        let usage_data = self.dedup_engine.filter_duplicates(parsed_conversation.messages, &project)?;
        
        let mut watch_events = Vec::new();
        
        for data in usage_data {
            // Filter out messages with 0 tokens to reduce noise
            if !self.should_display_message(&data) {
                continue;
            }
            
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
        // CRITICAL FIX: Reset session tracking to prevent cost carryover
        self.session_tracker.reset_sessions();
    }

    /// Reset session tracking to start fresh cost tracking
    /// This ensures each new watch session starts from $0.00
    pub fn reset_sessions(&mut self) {
        self.session_tracker.reset_sessions();
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

    /// Determine if a message should be displayed in watch mode
    /// Filters out messages with 0 tokens to reduce noise
    pub fn should_display_message(&self, data: &crate::parser::jsonl::UsageData) -> bool {
        if let Some(usage) = &data.usage {
            let total_tokens = usage.input_tokens.unwrap_or(0) 
                + usage.output_tokens.unwrap_or(0)
                + usage.cache_creation_input_tokens.unwrap_or(0)
                + usage.cache_read_input_tokens.unwrap_or(0);
            total_tokens > 0
        } else {
            false
        }
    }

    pub fn get_dashboard_state(&self) -> &DashboardState {
        // This would need to be implemented if we want to expose dashboard state
        // For now, the dashboard manages its own state internally
        todo!("Dashboard state access not implemented")
    }
}

impl Drop for WatchMode {
    fn drop(&mut self) {
        // Abort any running file watcher task
        if let Some(task_handle) = self.file_watcher_task.take() {
            task_handle.abort();
        }
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
    fn test_unified_project_name_extraction() {
        use crate::parser::jsonl::JsonlParser;
        
        // Test the unified project name extraction directly through parser
        let parser = JsonlParser::new(PathBuf::from("/home/user/.claude/projects"));
        
        // Test directory-based extraction (fallback)
        let path = PathBuf::from("/home/user/.claude/projects/my-project/conversation.jsonl");
        let empty_messages = vec![];
        let project = parser.get_unified_project_name(&path, &empty_messages);
        assert_eq!(project, "my-project");
        
        // Test smart name extraction from cwd field
        let messages_with_cwd = vec![crate::parser::jsonl::UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("test-uuid".to_string()),
            request_id: Some("req-1".to_string()),
            message: None,
            usage: None,
            cost_usd: None,
            cwd: Some("/home/user/real-project".to_string()),
            original_cwd: None,
        }];
        
        let path2 = PathBuf::from("/home/user/.claude/projects/-home-user-dir-name/conversation.jsonl");
        let project2 = parser.get_unified_project_name(&path2, &messages_with_cwd);
        assert_eq!(project2, "real-project"); // Should use smart name from cwd, not directory
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
        let content = r#"{"uuid":"msg1","requestId":"req1","message":{"content":"Hello","model":"claude-sonnet-4"},"usage":{"inputTokens":10,"outputTokens":20},"costUSD":0.001,"cwd":"/home/user/test-project","originalCwd":"/home/user/test-project"}
{"uuid":"msg2","requestId":"req2","message":{"content":"World","model":"claude-sonnet-4"},"usage":{"inputTokens":15,"outputTokens":25},"costUSD":0.002,"cwd":"/home/user/test-project","originalCwd":"/home/user/test-project"}
{"uuid":"msg1","requestId":"req1","message":{"content":"Hello","model":"claude-sonnet-4"},"usage":{"inputTokens":10,"outputTokens":20},"costUSD":0.001,"cwd":"/home/user/test-project","originalCwd":"/home/user/test-project"}"#;
        
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

    #[tokio::test]
    async fn test_session_cost_reset_on_new_watch_mode() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.general.claude_projects_path = temp_dir.path().join("projects").to_string_lossy().to_string();
        
        // Create first watch mode instance and simulate activity
        let mut watch_mode1 = WatchMode::new(config.clone(), None, 0.10, 200).unwrap();
        
        // Create test JSONL file to simulate activity
        let projects_dir = temp_dir.path().join("projects");
        let project_dir = projects_dir.join("test-project");
        fs::create_dir_all(&project_dir).unwrap();
        
        let test_file = project_dir.join("conversation.jsonl");
        let content = r#"{"uuid":"msg1","requestId":"req1","message":{"content":"Hello","model":"claude-sonnet-4"},"usage":{"inputTokens":100,"outputTokens":200},"costUSD":0.1,"cwd":"/home/user/test-project","originalCwd":"/home/user/test-project"}"#;
        fs::write(&test_file, content).unwrap();
        
        // Process file to create session activity
        let _ = watch_mode1.process_file_change(test_file.clone()).await.unwrap();
        
        // Verify session has activity
        let session_cost1 = watch_mode1.get_total_session_cost();
        assert!(session_cost1 > 0.0, "First session should have non-zero cost, got {}", session_cost1);
        
        // Create new watch mode instance (simulating restart)
        let watch_mode2 = WatchMode::new(config, None, 0.10, 200).unwrap();
        
        // Verify new session starts at $0
        let session_cost2 = watch_mode2.get_total_session_cost();
        assert_eq!(session_cost2, 0.0, "New watch mode session should start at $0, got {}", session_cost2);
    }

    #[tokio::test]
    async fn test_reset_file_tracking_resets_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.general.claude_projects_path = temp_dir.path().join("projects").to_string_lossy().to_string();
        let mut watch_mode = WatchMode::new(config, None, 0.10, 200).unwrap();
        
        // Create test JSONL file
        let projects_dir = temp_dir.path().join("projects");
        let project_dir = projects_dir.join("test-project");
        fs::create_dir_all(&project_dir).unwrap();
        
        let test_file = project_dir.join("conversation.jsonl");
        let content = r#"{"uuid":"msg1","requestId":"req1","message":{"content":"Hello","model":"claude-sonnet-4"},"usage":{"inputTokens":100,"outputTokens":200},"costUSD":0.1,"cwd":"/home/user/test-project","originalCwd":"/home/user/test-project"}"#;
        fs::write(&test_file, content).unwrap();
        
        // Process file to create session activity
        let _ = watch_mode.process_file_change(test_file).await.unwrap();
        
        // Verify session has activity
        let session_cost_before = watch_mode.get_total_session_cost();
        assert!(session_cost_before > 0.0, "Session should have activity before reset");
        
        // Reset file tracking (user presses 'r' in dashboard)
        watch_mode.reset_file_tracking();
        
        // Verify sessions are reset
        let session_cost_after = watch_mode.get_total_session_cost();
        assert_eq!(session_cost_after, 0.0, "Sessions should be reset to $0 after reset_file_tracking");
    }

    #[tokio::test]
    async fn test_session_cost_no_carryover_between_restarts() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.general.claude_projects_path = temp_dir.path().join("projects").to_string_lossy().to_string();
        
        // Create test JSONL file
        let projects_dir = temp_dir.path().join("projects");
        let project_dir = projects_dir.join("test-project");
        fs::create_dir_all(&project_dir).unwrap();
        
        let test_file = project_dir.join("conversation.jsonl");
        let content = r#"{"uuid":"msg1","requestId":"req1","message":{"content":"Hello","model":"claude-sonnet-4"},"usage":{"inputTokens":100,"outputTokens":200},"costUSD":0.1,"cwd":"/home/user/test-project","originalCwd":"/home/user/test-project"}
{"uuid":"msg2","requestId":"req2","message":{"content":"World","model":"claude-sonnet-4"},"usage":{"inputTokens":150,"outputTokens":250},"costUSD":0.15,"cwd":"/home/user/test-project","originalCwd":"/home/user/test-project"}"#;
        fs::write(&test_file, content).unwrap();
        
        // First session
        {
            let mut watch_mode1 = WatchMode::new(config.clone(), None, 0.10, 200).unwrap();
            let _ = watch_mode1.process_file_change(test_file.clone()).await.unwrap();
            let session1_cost = watch_mode1.get_total_session_cost();
            assert!(session1_cost > 0.0, "Session 1 should have activity");
        }
        
        // Second session (restart)
        {
            let mut watch_mode2 = WatchMode::new(config.clone(), None, 0.10, 200).unwrap();
            // Session should start at $0 even though same file exists
            let initial_cost = watch_mode2.get_total_session_cost();
            assert_eq!(initial_cost, 0.0, "Session 2 should start at $0");
            
            // Process file again
            let _ = watch_mode2.process_file_change(test_file.clone()).await.unwrap();
            let session2_cost = watch_mode2.get_total_session_cost();
            assert!(session2_cost > 0.0, "Session 2 should accumulate costs after processing");
        }
        
        // Third session (restart)
        {
            let watch_mode3 = WatchMode::new(config, None, 0.10, 200).unwrap();
            let session3_cost = watch_mode3.get_total_session_cost();
            assert_eq!(session3_cost, 0.0, "Session 3 should start at $0");
        }
    }

    #[test]
    fn test_reset_sessions_method() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.general.claude_projects_path = temp_dir.path().join("projects").to_string_lossy().to_string();
        let mut watch_mode = WatchMode::new(config, None, 0.10, 200).unwrap();
        
        // Manually add session activity using session tracker
        watch_mode.session_tracker.update_activity("test-project", 1000, 0.5, "claude-3-sonnet");
        
        // Verify session exists
        assert_eq!(watch_mode.get_total_session_cost(), 0.5);
        assert_eq!(watch_mode.get_active_sessions().len(), 1);
        
        // Reset sessions
        watch_mode.reset_sessions();
        
        // Verify sessions are cleared
        assert_eq!(watch_mode.get_total_session_cost(), 0.0);
        assert_eq!(watch_mode.get_active_sessions().len(), 0);
    }

    #[tokio::test]
    async fn test_file_watcher_task_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        
        // Create projects directory
        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();
        config.general.claude_projects_path = projects_dir.to_string_lossy().to_string();
        
        let mut watch_mode = WatchMode::new(config, None, 0.10, 200).unwrap();
        
        // Start file watcher - this should store the task handle
        let result = watch_mode.start_file_watcher().await;
        assert!(result.is_ok(), "File watcher should start successfully");
        
        // Verify task handle is stored
        assert!(watch_mode.file_watcher_task.is_some(), "File watcher task handle should be stored");
        
        // Call cleanup
        watch_mode.cleanup().await;
        
        // Verify task handle is removed (taken by cleanup)
        assert!(watch_mode.file_watcher_task.is_none(), "File watcher task handle should be removed after cleanup");
    }

    #[tokio::test]
    async fn test_cleanup_handles_no_task() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.general.claude_projects_path = temp_dir.path().join("projects").to_string_lossy().to_string();
        
        let mut watch_mode = WatchMode::new(config, None, 0.10, 200).unwrap();
        
        // Call cleanup without starting file watcher - should not panic
        watch_mode.cleanup().await;
        
        // Should still be None
        assert!(watch_mode.file_watcher_task.is_none(), "No task handle should exist");
    }

    #[test]  
    fn test_drop_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.general.claude_projects_path = temp_dir.path().join("projects").to_string_lossy().to_string();
        
        let mut watch_mode = WatchMode::new(config, None, 0.10, 200).unwrap();
        
        // Simulate having a task handle (we can't actually start the task in sync test)
        let dummy_task = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        });
        watch_mode.file_watcher_task = Some(dummy_task);
        
        // Verify task exists
        assert!(watch_mode.file_watcher_task.is_some(), "Task should be present before drop");
        
        // Drop should automatically cleanup - this happens when watch_mode goes out of scope
        drop(watch_mode);
        
        // Test passes if no panic occurs during drop
    }

    #[test]
    fn test_zero_token_filtering_logic() {
        use crate::parser::jsonl::{UsageData, Usage};
        
        // Create test messages with different token patterns
        let zero_token_msg = UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("msg1".to_string()),
            request_id: Some("req1".to_string()),
            message: None,
            usage: Some(Usage {
                input_tokens: Some(0),
                output_tokens: Some(0),
                cache_creation_input_tokens: Some(0),
                cache_read_input_tokens: Some(0),
            }),
            cost_usd: Some(0.0),
            cwd: None,
            original_cwd: None,
        };
        
        let input_token_msg = UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("msg2".to_string()),
            request_id: Some("req2".to_string()),
            message: None,
            usage: Some(Usage {
                input_tokens: Some(15),
                output_tokens: Some(25),
                cache_creation_input_tokens: Some(0),
                cache_read_input_tokens: Some(0),
            }),
            cost_usd: Some(0.002),
            cwd: None,
            original_cwd: None,
        };
        
        let cache_token_msg = UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("msg4".to_string()),
            request_id: Some("req4".to_string()),
            message: None,
            usage: Some(Usage {
                input_tokens: Some(0),
                output_tokens: Some(0),
                cache_creation_input_tokens: Some(10),
                cache_read_input_tokens: Some(0),
            }),
            cost_usd: Some(0.001),
            cwd: None,
            original_cwd: None,
        };
        
        let messages = vec![zero_token_msg, input_token_msg, cache_token_msg];
        
        // Filter using the same logic as should_display_message
        let filtered_messages: Vec<_> = messages.iter().filter(|data| {
            if let Some(usage) = &data.usage {
                let total_tokens = usage.input_tokens.unwrap_or(0) 
                    + usage.output_tokens.unwrap_or(0)
                    + usage.cache_creation_input_tokens.unwrap_or(0)
                    + usage.cache_read_input_tokens.unwrap_or(0);
                total_tokens > 0
            } else {
                false
            }
        }).collect();
        
        // Should only have 2 messages (input_token_msg and cache_token_msg)
        assert_eq!(filtered_messages.len(), 2, "Should filter out zero-token messages");
        
        // Verify correct messages are kept
        let kept_ids: Vec<_> = filtered_messages.iter().map(|m| m.uuid.as_ref().unwrap().as_str()).collect();
        assert!(kept_ids.contains(&"msg2"), "Should keep message with input/output tokens");
        assert!(kept_ids.contains(&"msg4"), "Should keep message with cache creation tokens");
        assert!(!kept_ids.contains(&"msg1"), "Should filter out zero-token message");
    }

    #[test]
    fn test_should_display_message_logic() {
        use crate::parser::jsonl::{UsageData, Usage};
        
        // Test message with 0 tokens - should be filtered
        let zero_token_data = UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("test-uuid".to_string()),
            request_id: Some("req-1".to_string()),
            message: None,
            usage: Some(Usage {
                input_tokens: Some(0),
                output_tokens: Some(0),
                cache_creation_input_tokens: Some(0),
                cache_read_input_tokens: Some(0),
            }),
            cost_usd: Some(0.0),
            cwd: None,
            original_cwd: None,
        };
        
        // Test message with non-zero input tokens - should be displayed
        let input_token_data = UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("test-uuid".to_string()),
            request_id: Some("req-1".to_string()),
            message: None,
            usage: Some(Usage {
                input_tokens: Some(10),
                output_tokens: Some(0),
                cache_creation_input_tokens: Some(0),
                cache_read_input_tokens: Some(0),
            }),
            cost_usd: Some(0.001),
            cwd: None,
            original_cwd: None,
        };
        
        // Test message with non-zero cache tokens - should be displayed
        let cache_token_data = UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("test-uuid".to_string()),
            request_id: Some("req-1".to_string()),
            message: None,
            usage: Some(Usage {
                input_tokens: Some(0),
                output_tokens: Some(0),
                cache_creation_input_tokens: Some(5),
                cache_read_input_tokens: Some(0),
            }),
            cost_usd: Some(0.0005),
            cwd: None,
            original_cwd: None,
        };
        
        // Test message with no usage data - should be filtered
        let no_usage_data = UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("test-uuid".to_string()),
            request_id: Some("req-1".to_string()),
            message: None,
            usage: None,
            cost_usd: None,
            cwd: None,
            original_cwd: None,
        };
        
        // Test the filtering logic directly without creating full WatchMode
        fn should_display_message_standalone(data: &UsageData) -> bool {
            if let Some(usage) = &data.usage {
                let total_tokens = usage.input_tokens.unwrap_or(0) 
                    + usage.output_tokens.unwrap_or(0)
                    + usage.cache_creation_input_tokens.unwrap_or(0)
                    + usage.cache_read_input_tokens.unwrap_or(0);
                total_tokens > 0
            } else {
                false
            }
        }
        
        // Test the filtering logic
        assert!(!should_display_message_standalone(&zero_token_data), "Zero token message should be filtered");
        assert!(should_display_message_standalone(&input_token_data), "Message with input tokens should be displayed");
        assert!(should_display_message_standalone(&cache_token_data), "Message with cache tokens should be displayed");
        assert!(!should_display_message_standalone(&no_usage_data), "Message with no usage data should be filtered");
    }

    #[test]
    fn test_filtering_preserves_cache_read_tokens() {
        use crate::parser::jsonl::{UsageData, Usage};
        
        // Test message with cache read tokens but 0 input/output tokens
        let cache_read_msg = UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("msg1".to_string()),
            request_id: Some("req1".to_string()),
            message: None,
            usage: Some(Usage {
                input_tokens: Some(0),
                output_tokens: Some(0),
                cache_creation_input_tokens: Some(0),
                cache_read_input_tokens: Some(100),
            }),
            cost_usd: Some(0.001),
            cwd: None,
            original_cwd: None,
        };
        
        // Test the filtering logic directly without creating full WatchMode
        fn should_display_message_standalone(data: &UsageData) -> bool {
            if let Some(usage) = &data.usage {
                let total_tokens = usage.input_tokens.unwrap_or(0) 
                    + usage.output_tokens.unwrap_or(0)
                    + usage.cache_creation_input_tokens.unwrap_or(0)
                    + usage.cache_read_input_tokens.unwrap_or(0);
                total_tokens > 0
            } else {
                false
            }
        }
        
        // Should preserve messages with cache read tokens because they're important
        assert!(should_display_message_standalone(&cache_read_msg), "Should preserve messages with cache read tokens");
    }
}