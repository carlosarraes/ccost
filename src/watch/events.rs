#![allow(dead_code)] // Watch mode events - experimental feature
// Event system for watch mode
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum WatchEvent {
    NewMessage {
        tokens: u32,
        cost: f64,
        model: String,
        project: String,
        timestamp: DateTime<Utc>,
    },
    ModelSwitch {
        from: String,
        to: String,
        project: String,
        timestamp: DateTime<Utc>,
    },
    CacheHit {
        saved_tokens: u32,
        saved_cost: f64,
        model: String,
        project: String,
        timestamp: DateTime<Utc>,
    },
    ExpensiveConversation {
        cost: f64,
        threshold: f64,
        project: String,
        timestamp: DateTime<Utc>,
    },
    SessionStart {
        project: String,
        timestamp: DateTime<Utc>,
    },
    SessionEnd {
        project: String,
        duration: Duration,
        total_cost: f64,
        timestamp: DateTime<Utc>,
    },
    ProjectActivity {
        project: String,
        message_count: u32,
        cost: f64,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone)]
pub enum FileEvent {
    FileModified(PathBuf),
    FileCreated(PathBuf),
    Error(String),
}

impl WatchEvent {
    pub fn get_cost(&self) -> f64 {
        match self {
            WatchEvent::NewMessage { cost, .. } => *cost,
            WatchEvent::CacheHit { saved_cost, .. } => -*saved_cost, // Negative because it's saved
            WatchEvent::ExpensiveConversation { cost, .. } => *cost,
            WatchEvent::SessionEnd { total_cost, .. } => *total_cost,
            WatchEvent::ProjectActivity { cost, .. } => *cost,
            _ => 0.0,
        }
    }

    pub fn get_project(&self) -> &str {
        match self {
            WatchEvent::NewMessage { project, .. } => project,
            WatchEvent::ModelSwitch { project, .. } => project,
            WatchEvent::CacheHit { project, .. } => project,
            WatchEvent::ExpensiveConversation { project, .. } => project,
            WatchEvent::SessionStart { project, .. } => project,
            WatchEvent::SessionEnd { project, .. } => project,
            WatchEvent::ProjectActivity { project, .. } => project,
        }
    }

    pub fn get_timestamp(&self) -> DateTime<Utc> {
        match self {
            WatchEvent::NewMessage { timestamp, .. } => *timestamp,
            WatchEvent::ModelSwitch { timestamp, .. } => *timestamp,
            WatchEvent::CacheHit { timestamp, .. } => *timestamp,
            WatchEvent::ExpensiveConversation { timestamp, .. } => *timestamp,
            WatchEvent::SessionStart { timestamp, .. } => *timestamp,
            WatchEvent::SessionEnd { timestamp, .. } => *timestamp,
            WatchEvent::ProjectActivity { timestamp, .. } => *timestamp,
        }
    }

    pub fn is_expensive(&self, threshold: f64) -> bool {
        match self {
            WatchEvent::NewMessage { cost, .. } => *cost > threshold,
            WatchEvent::ExpensiveConversation { .. } => true,
            WatchEvent::ProjectActivity { cost, .. } => *cost > threshold,
            _ => false,
        }
    }

    pub fn get_efficiency_level(&self) -> EfficiencyLevel {
        match self {
            WatchEvent::CacheHit { .. } => EfficiencyLevel::Excellent,
            WatchEvent::NewMessage { cost, .. } => {
                if *cost < 0.01 { EfficiencyLevel::Good }
                else if *cost < 0.05 { EfficiencyLevel::Warning }
                else { EfficiencyLevel::Expensive }
            },
            WatchEvent::ExpensiveConversation { .. } => EfficiencyLevel::Expensive,
            _ => EfficiencyLevel::Neutral,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EfficiencyLevel {
    Excellent,  // Green - cache hits, very low cost
    Good,       // Green - low cost operations
    Warning,    // Yellow - moderate cost
    Expensive,  // Red - high cost operations
    Neutral,    // Default color
}

impl EfficiencyLevel {
    pub fn to_color(&self) -> ratatui::style::Color {
        match self {
            EfficiencyLevel::Excellent | EfficiencyLevel::Good => ratatui::style::Color::Green,
            EfficiencyLevel::Warning => ratatui::style::Color::Yellow,
            EfficiencyLevel::Expensive => ratatui::style::Color::Red,
            EfficiencyLevel::Neutral => ratatui::style::Color::Gray,
        }
    }

    pub fn to_symbol(&self) -> &'static str {
        match self {
            EfficiencyLevel::Excellent => "★",
            EfficiencyLevel::Good => "✓",
            EfficiencyLevel::Warning => "⚠",
            EfficiencyLevel::Expensive => "⚡",
            EfficiencyLevel::Neutral => "·",
        }
    }
}