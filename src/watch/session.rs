// Session tracking for watch mode
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::time::Duration;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub project: String,
    pub start_time: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub message_count: u32,
    pub total_tokens: u32,
    pub total_cost: f64,
    pub models_used: HashMap<String, u32>,
    pub is_active: bool,
}

impl SessionState {
    pub fn new(project: String) -> Self {
        let now = Utc::now();
        SessionState {
            project,
            start_time: now,
            last_activity: now,
            message_count: 0,
            total_tokens: 0,
            total_cost: 0.0,
            models_used: HashMap::new(),
            is_active: true,
        }
    }

    pub fn update_activity(&mut self, tokens: u32, cost: f64, model: &str) {
        self.last_activity = Utc::now();
        self.message_count += 1;
        self.total_tokens += tokens;
        self.total_cost += cost;
        *self.models_used.entry(model.to_string()).or_insert(0) += 1;
        self.is_active = true;
    }

    pub fn duration(&self) -> Duration {
        let end_time = if self.is_active { Utc::now() } else { self.last_activity };
        end_time.signed_duration_since(self.start_time)
            .to_std()
            .unwrap_or(Duration::ZERO)
    }

    pub fn idle_duration(&self) -> Duration {
        Utc::now().signed_duration_since(self.last_activity)
            .to_std()
            .unwrap_or(Duration::ZERO)
    }

    pub fn average_cost_per_message(&self) -> f64 {
        if self.message_count == 0 {
            0.0
        } else {
            self.total_cost / self.message_count as f64
        }
    }

    pub fn tokens_per_minute(&self) -> f64 {
        let duration_minutes = self.duration().as_secs() as f64 / 60.0;
        if duration_minutes == 0.0 {
            0.0
        } else {
            self.total_tokens as f64 / duration_minutes
        }
    }

    pub fn primary_model(&self) -> Option<String> {
        self.models_used.iter()
            .max_by_key(|(_, count)| *count)
            .map(|(model, _)| model.clone())
    }
}

#[derive(Debug, Clone)]
pub struct SessionTracker {
    sessions: HashMap<String, SessionState>,
    idle_timeout: Duration,
    expensive_threshold: f64,
}

impl SessionTracker {
    pub fn new(idle_timeout_minutes: u32, expensive_threshold: f64) -> Self {
        SessionTracker {
            sessions: HashMap::new(),
            idle_timeout: Duration::from_secs(idle_timeout_minutes as u64 * 60),
            expensive_threshold,
        }
    }

    pub fn update_activity(&mut self, project: &str, tokens: u32, cost: f64, model: &str) -> bool {
        // Check if this creates a new session
        let is_new_session = !self.sessions.contains_key(project) || 
            self.sessions.get(project).map_or(true, |s| !s.is_active);

        let session = self.sessions.entry(project.to_string())
            .or_insert_with(|| SessionState::new(project.to_string()));

        session.update_activity(tokens, cost, model);
        is_new_session
    }

    pub fn check_idle_sessions(&mut self) -> Vec<SessionState> {
        let mut ended_sessions = Vec::new();
        
        for session in self.sessions.values_mut() {
            if session.is_active && session.idle_duration() > self.idle_timeout {
                session.is_active = false;
                ended_sessions.push(session.clone());
            }
        }
        
        ended_sessions
    }

    pub fn get_active_sessions(&self) -> Vec<&SessionState> {
        self.sessions.values()
            .filter(|s| s.is_active)
            .collect()
    }

    pub fn get_session(&self, project: &str) -> Option<&SessionState> {
        self.sessions.get(project)
    }

    pub fn get_all_sessions(&self) -> &HashMap<String, SessionState> {
        &self.sessions
    }

    pub fn total_active_cost(&self) -> f64 {
        self.sessions.values()
            .filter(|s| s.is_active)
            .map(|s| s.total_cost)
            .sum()
    }

    pub fn total_active_messages(&self) -> u32 {
        self.sessions.values()
            .filter(|s| s.is_active)
            .map(|s| s.message_count)
            .sum()
    }

    pub fn is_expensive_session(&self, project: &str) -> bool {
        self.sessions.get(project)
            .map_or(false, |s| s.total_cost > self.expensive_threshold)
    }

    pub fn reset_sessions(&mut self) {
        self.sessions.clear();
    }

    pub fn end_session(&mut self, project: &str) -> Option<SessionState> {
        if let Some(session) = self.sessions.get_mut(project) {
            if session.is_active {
                session.is_active = false;
                return Some(session.clone());
            }
        }
        None
    }

    pub fn get_session_statistics(&self) -> SessionStatistics {
        let active_sessions: Vec<_> = self.sessions.values()
            .filter(|s| s.is_active)
            .collect();

        let total_cost: f64 = active_sessions.iter().map(|s| s.total_cost).sum();
        let total_messages: u32 = active_sessions.iter().map(|s| s.message_count).sum();
        let total_tokens: u32 = active_sessions.iter().map(|s| s.total_tokens).sum();

        let average_cost = if !active_sessions.is_empty() {
            total_cost / active_sessions.len() as f64
        } else {
            0.0
        };

        let most_active_project = active_sessions.iter()
            .max_by_key(|s| s.message_count)
            .map(|s| s.project.clone());

        let most_expensive_project = active_sessions.iter()
            .max_by(|a, b| a.total_cost.partial_cmp(&b.total_cost).unwrap())
            .map(|s| s.project.clone());

        SessionStatistics {
            active_sessions: active_sessions.len(),
            total_cost,
            total_messages,
            total_tokens,
            average_cost_per_session: average_cost,
            most_active_project,
            most_expensive_project,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionStatistics {
    pub active_sessions: usize,
    pub total_cost: f64,
    pub total_messages: u32,
    pub total_tokens: u32,
    pub average_cost_per_session: f64,
    pub most_active_project: Option<String>,
    pub most_expensive_project: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_creation() {
        let session = SessionState::new("test-project".to_string());
        assert_eq!(session.project, "test-project");
        assert_eq!(session.message_count, 0);
        assert_eq!(session.total_cost, 0.0);
        assert!(session.is_active);
    }

    #[test]
    fn test_session_activity_update() {
        let mut session = SessionState::new("test-project".to_string());
        session.update_activity(1000, 0.05, "claude-3-sonnet");
        
        assert_eq!(session.message_count, 1);
        assert_eq!(session.total_tokens, 1000);
        assert_eq!(session.total_cost, 0.05);
        assert_eq!(session.models_used.get("claude-3-sonnet"), Some(&1));
    }

    #[test]
    fn test_session_tracker_new_session() {
        let mut tracker = SessionTracker::new(30, 0.1);
        let is_new = tracker.update_activity("project1", 500, 0.02, "claude-3-haiku");
        
        assert!(is_new);
        assert_eq!(tracker.get_active_sessions().len(), 1);
    }

    #[test]
    fn test_session_tracker_existing_session() {
        let mut tracker = SessionTracker::new(30, 0.1);
        tracker.update_activity("project1", 500, 0.02, "claude-3-haiku");
        let is_new = tracker.update_activity("project1", 300, 0.01, "claude-3-haiku");
        
        assert!(!is_new);
        assert_eq!(tracker.get_active_sessions().len(), 1);
        
        let session = tracker.get_session("project1").unwrap();
        assert_eq!(session.message_count, 2);
        assert_eq!(session.total_tokens, 800);
    }

    #[test]
    fn test_session_statistics() {
        let mut tracker = SessionTracker::new(30, 0.1);
        tracker.update_activity("project1", 1000, 0.05, "claude-3-sonnet");
        tracker.update_activity("project2", 500, 0.02, "claude-3-haiku");
        tracker.update_activity("project1", 200, 0.01, "claude-3-sonnet");
        
        let stats = tracker.get_session_statistics();
        assert_eq!(stats.active_sessions, 2);
        assert_eq!(stats.total_cost, 0.08);
        assert_eq!(stats.total_messages, 3);
        assert_eq!(stats.total_tokens, 1700);
        assert_eq!(stats.most_active_project, Some("project1".to_string()));
        assert_eq!(stats.most_expensive_project, Some("project1".to_string()));
    }

    #[test]
    fn test_expensive_session_detection() {
        let mut tracker = SessionTracker::new(30, 0.1);
        tracker.update_activity("cheap-project", 100, 0.05, "claude-3-haiku");
        tracker.update_activity("expensive-project", 2000, 0.15, "claude-3-opus");
        
        assert!(!tracker.is_expensive_session("cheap-project"));
        assert!(tracker.is_expensive_session("expensive-project"));
    }

    #[test]
    fn test_primary_model_detection() {
        let mut session = SessionState::new("test-project".to_string());
        session.update_activity(500, 0.02, "claude-3-haiku");
        session.update_activity(1000, 0.05, "claude-3-sonnet");
        session.update_activity(300, 0.01, "claude-3-haiku");
        
        assert_eq!(session.primary_model(), Some("claude-3-haiku".to_string()));
    }

    #[test]
    fn test_session_metrics() {
        let mut session = SessionState::new("test-project".to_string());
        session.update_activity(1000, 0.04, "claude-3-sonnet");
        session.update_activity(500, 0.02, "claude-3-sonnet");
        
        assert_eq!(session.average_cost_per_message(), 0.03);
        assert!(session.tokens_per_minute() >= 0.0); // Should be positive
    }
}