use crate::alerts::{
    thresholds::{AlertThresholds, AlertType, AlertRule},
    notifications::NotificationHandler,
    patterns::{UsagePatternAnalyzer, WeeklySummary},
};
use crate::analysis::usage::ProjectUsage;
use crate::storage::Database;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;

pub struct AlertSystem {
    pub thresholds: AlertThresholds,
    pub notification_handler: NotificationHandler,
    pub pattern_analyzer: UsagePatternAnalyzer,
    pub alert_rules: HashMap<String, AlertRule>,
    database: Option<Database>,
}

impl AlertSystem {
    pub fn new(thresholds: AlertThresholds, notifications_enabled: bool) -> Self {
        let notification_handler = NotificationHandler::new(notifications_enabled);
        let pattern_analyzer = UsagePatternAnalyzer::new();
        
        // Initialize alert rules based on enabled alerts
        let mut alert_rules = HashMap::new();
        for alert_type in &thresholds.enabled_alerts {
            alert_rules.insert(
                alert_type.clone(),
                AlertRule::new(alert_type.clone(), true)
            );
        }

        Self {
            thresholds,
            notification_handler,
            pattern_analyzer,
            alert_rules,
            database: None,
        }
    }

    pub fn with_database(mut self, database: Database) -> Self {
        self.database = Some(database);
        self
    }

    /// Check all configured thresholds against current usage data
    pub fn check_thresholds(
        &mut self,
        current_usage: &[ProjectUsage],
        daily_spending: f64,
        weekly_spending: f64,
        monthly_spending: f64,
        daily_tokens: u64,
        currency: &str,
        daily_history: &[f64],
    ) -> Result<Vec<AlertType>> {
        let mut triggered_alerts = Vec::new();

        // Check daily spending limit
        if let Some(limit) = self.thresholds.daily_spending_limit {
            if daily_spending > limit && self.can_trigger_alert("daily_spending") {
                let alert = AlertType::DailySpendingLimit {
                    limit,
                    current: daily_spending,
                    currency: currency.to_string(),
                };
                triggered_alerts.push(alert);
                self.mark_alert_triggered("daily_spending");
            }
        }

        // Check weekly spending limit
        if let Some(limit) = self.thresholds.weekly_spending_limit {
            if weekly_spending > limit && self.can_trigger_alert("weekly_spending") {
                let alert = AlertType::WeeklySpendingLimit {
                    limit,
                    current: weekly_spending,
                    currency: currency.to_string(),
                };
                triggered_alerts.push(alert);
                self.mark_alert_triggered("weekly_spending");
            }
        }

        // Check monthly spending limit
        if let Some(limit) = self.thresholds.monthly_spending_limit {
            if monthly_spending > limit && self.can_trigger_alert("monthly_spending") {
                let alert = AlertType::MonthlySpendingLimit {
                    limit,
                    current: monthly_spending,
                    currency: currency.to_string(),
                };
                triggered_alerts.push(alert);
                self.mark_alert_triggered("monthly_spending");
            }
        }

        // Check daily token limit
        if let Some(limit) = self.thresholds.daily_token_limit {
            if daily_tokens > limit && self.can_trigger_alert("daily_tokens") {
                let alert = AlertType::DailyTokenLimit {
                    limit,
                    current: daily_tokens,
                };
                triggered_alerts.push(alert);
                self.mark_alert_triggered("daily_tokens");
            }
        }

        // Check for spending spikes
        if self.can_trigger_alert("spending_spike") {
            if let Some(spike_alert) = self.pattern_analyzer.check_spending_spike(
                daily_spending, 
                daily_history, 
                currency
            ) {
                triggered_alerts.push(spike_alert);
                self.mark_alert_triggered("spending_spike");
            }
        }

        // Check usage patterns
        let pattern_alerts = self.pattern_analyzer.analyze_patterns(current_usage, currency);
        for alert in pattern_alerts {
            let alert_id = alert.alert_id();
            if self.can_trigger_alert(alert_id) {
                triggered_alerts.push(alert);
                self.mark_alert_triggered(alert_id);
            }
        }

        Ok(triggered_alerts)
    }

    /// Send notifications for triggered alerts
    pub fn send_notifications(&self, alerts: &[AlertType]) -> Result<()> {
        for alert in alerts {
            self.notification_handler.send_alert(alert)
                .with_context(|| format!("Failed to send notification for alert: {}", alert.alert_id()))?;
        }
        Ok(())
    }

    /// Send a weekly usage summary notification
    pub fn send_weekly_summary(&self, usage_data: &[ProjectUsage], currency: &str) -> Result<()> {
        let summary = self.pattern_analyzer.generate_weekly_summary(usage_data, currency);
        let title = "Weekly Claude Usage Summary";
        let message = summary.to_notification_message();
        
        self.notification_handler.send_summary_notification(title, &message)
            .context("Failed to send weekly summary notification")
    }

    /// Test the notification system
    pub fn test_notifications(&self) -> Result<()> {
        self.notification_handler.send_test_notification()
            .context("Failed to send test notification")
    }

    /// Enable or disable a specific alert type
    pub fn set_alert_enabled(&mut self, alert_type: &str, enabled: bool) -> Result<()> {
        if let Some(rule) = self.alert_rules.get_mut(alert_type) {
            rule.enabled = enabled;
            
            // Update enabled_alerts list in thresholds
            if enabled {
                if !self.thresholds.enabled_alerts.contains(&alert_type.to_string()) {
                    self.thresholds.enabled_alerts.push(alert_type.to_string());
                }
            } else {
                self.thresholds.enabled_alerts.retain(|x| x != alert_type);
            }
            
            Ok(())
        } else {
            // Create new rule if it doesn't exist
            self.alert_rules.insert(
                alert_type.to_string(),
                AlertRule::new(alert_type.to_string(), enabled)
            );
            
            if enabled {
                self.thresholds.enabled_alerts.push(alert_type.to_string());
            }
            
            Ok(())
        }
    }

    /// Set a threshold value
    pub fn set_threshold(&mut self, threshold_type: &str, value: f64) -> Result<()> {
        match threshold_type {
            "daily_spending" => self.thresholds.daily_spending_limit = Some(value),
            "weekly_spending" => self.thresholds.weekly_spending_limit = Some(value),
            "monthly_spending" => self.thresholds.monthly_spending_limit = Some(value),
            "daily_tokens" => self.thresholds.daily_token_limit = Some(value as u64),
            "cache_hit_rate" => self.thresholds.cache_hit_rate_threshold = Some(value as f32),
            "opus_usage" => self.thresholds.opus_usage_threshold = Some(value as u32),
            "spending_spike_factor" => self.thresholds.spending_spike_factor = Some(value as f32),
            _ => return Err(anyhow::anyhow!("Unknown threshold type: {}", threshold_type)),
        }
        Ok(())
    }

    /// Get current alert status
    pub fn get_alert_status(&self) -> Vec<AlertStatus> {
        let mut status = Vec::new();
        
        for (alert_type, rule) in &self.alert_rules {
            status.push(AlertStatus {
                alert_type: alert_type.clone(),
                enabled: rule.enabled,
                last_triggered: rule.last_triggered,
                can_trigger: rule.can_trigger(),
            });
        }
        
        status.sort_by(|a, b| a.alert_type.cmp(&b.alert_type));
        status
    }

    /// Enable/disable desktop notifications
    pub fn set_notifications_enabled(&mut self, enabled: bool) {
        self.notification_handler.set_enabled(enabled);
    }

    /// Check if notifications are available on this system
    pub fn notifications_available() -> bool {
        NotificationHandler::is_available()
    }

    fn can_trigger_alert(&self, alert_type: &str) -> bool {
        self.alert_rules
            .get(alert_type)
            .map(|rule| rule.can_trigger())
            .unwrap_or(false)
    }

    fn mark_alert_triggered(&mut self, alert_type: &str) {
        if let Some(rule) = self.alert_rules.get_mut(alert_type) {
            rule.mark_triggered();
        }
    }

    /// Save alert rules to database (if available)
    pub fn save_state(&self) -> Result<()> {
        if let Some(_db) = &self.database {
            // TODO: Implement database persistence for alert rules and thresholds
            // This would store the alert rules state and thresholds for persistence
            // between application runs
        }
        Ok(())
    }

    /// Load alert rules from database (if available)
    pub fn load_state(&mut self) -> Result<()> {
        if let Some(_db) = &self.database {
            // TODO: Implement database loading for alert rules and thresholds
            // This would restore the alert rules state and thresholds from
            // previous application runs
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AlertStatus {
    pub alert_type: String,
    pub enabled: bool,
    pub last_triggered: Option<DateTime<Utc>>,
    pub can_trigger: bool,
}

impl AlertStatus {
    pub fn status_text(&self) -> String {
        if !self.enabled {
            "Disabled".to_string()
        } else if !self.can_trigger {
            if let Some(last_triggered) = self.last_triggered {
                let duration = Utc::now() - last_triggered;
                if duration.num_hours() < 24 {
                    format!("Cooldown ({}h ago)", duration.num_hours())
                } else {
                    format!("Cooldown ({}d ago)", duration.num_days())
                }
            } else {
                "Ready".to_string()
            }
        } else {
            "Ready".to_string()
        }
    }
}

impl Default for AlertSystem {
    fn default() -> Self {
        Self::new(
            AlertThresholds::default(),
            NotificationHandler::is_available()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::usage::{ModelUsage, ProjectUsage};
    use std::collections::HashMap;

    fn create_test_usage() -> Vec<ProjectUsage> {
        let mut model_usage = HashMap::new();
        model_usage.insert("claude-3-sonnet-20240229".to_string(), ModelUsage {
            input_tokens: 50_000,
            output_tokens: 12_500,
            cache_creation_tokens: 0,
            cache_read_tokens: 5_000,
            cost_usd: 2.5,
            message_count: 10,
        });

        vec![ProjectUsage {
            project_name: "test_project".to_string(),
            total_input_tokens: 50_000,
            total_output_tokens: 12_500,
            total_cache_creation_tokens: 0,
            total_cache_read_tokens: 5_000,
            total_cost_usd: 2.5,
            message_count: 10,
            model_usage,
        }]
    }

    #[test]
    fn test_alert_system_creation() {
        let thresholds = AlertThresholds::default();
        let mut system = AlertSystem::new(thresholds, false);
        
        assert!(!system.notification_handler.is_enabled());
        assert!(!system.alert_rules.is_empty());
    }

    #[test]
    fn test_daily_spending_limit_alert() {
        let mut thresholds = AlertThresholds::default();
        thresholds.daily_spending_limit = Some(5.0);
        
        let mut system = AlertSystem::new(thresholds, false);
        let usage = create_test_usage();
        
        let alerts = system.check_thresholds(
            &usage,
            10.0, // daily spending (exceeds limit)
            10.0, // weekly spending
            10.0, // monthly spending
            50_000, // daily tokens
            "USD",
            &[1.0, 2.0, 3.0], // daily history
        ).unwrap();

        assert!(!alerts.is_empty());
        assert!(alerts.iter().any(|alert| matches!(alert, AlertType::DailySpendingLimit { .. })));
    }

    #[test]
    fn test_alert_enabling_disabling() {
        let mut system = AlertSystem::default();
        
        // Test enabling an alert
        system.set_alert_enabled("daily_spending", true).unwrap();
        assert!(system.alert_rules.get("daily_spending").unwrap().enabled);
        
        // Test disabling an alert
        system.set_alert_enabled("daily_spending", false).unwrap();
        assert!(!system.alert_rules.get("daily_spending").unwrap().enabled);
    }

    #[test]
    fn test_threshold_setting() {
        let mut system = AlertSystem::default();
        
        system.set_threshold("daily_spending", 15.0).unwrap();
        assert_eq!(system.thresholds.daily_spending_limit, Some(15.0));
        
        system.set_threshold("daily_tokens", 500_000.0).unwrap();
        assert_eq!(system.thresholds.daily_token_limit, Some(500_000));
    }

    #[test]
    fn test_alert_status() {
        let system = AlertSystem::default();
        let status = system.get_alert_status();
        
        assert!(!status.is_empty());
        assert!(status.iter().any(|s| s.alert_type == "daily_spending"));
    }

    #[test]
    fn test_alert_cooldown() {
        let mut system = AlertSystem::default();
        let usage = create_test_usage();
        
        // First check should trigger alert
        let alerts1 = system.check_thresholds(
            &usage,
            15.0, // exceeds default limit of 10.0
            15.0,
            15.0,
            50_000,
            "USD",
            &[1.0, 2.0, 3.0],
        ).unwrap();
        assert!(!alerts1.is_empty());
        
        // Second immediate check should not trigger due to cooldown
        let alerts2 = system.check_thresholds(
            &usage,
            20.0, // even higher spending
            20.0,
            20.0,
            50_000,
            "USD",
            &[1.0, 2.0, 3.0],
        ).unwrap();
        
        // Should not contain daily spending alert due to cooldown
        assert!(!alerts2.iter().any(|alert| matches!(alert, AlertType::DailySpendingLimit { .. })));
    }
}