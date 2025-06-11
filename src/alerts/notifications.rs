use crate::alerts::thresholds::{AlertType, AlertPriority};
use anyhow::{Context, Result};
use notify_rust::{Notification, Timeout};

pub struct NotificationHandler {
    enabled: bool,
}

impl NotificationHandler {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn send_alert(&self, alert: &AlertType) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let (timeout, urgency) = match alert.priority() {
            AlertPriority::Critical => (Timeout::Never, notify_rust::Urgency::Critical),
            AlertPriority::High => (Timeout::Milliseconds(10000), notify_rust::Urgency::Critical),
            AlertPriority::Medium => (Timeout::Milliseconds(7000), notify_rust::Urgency::Normal),
            AlertPriority::Low => (Timeout::Milliseconds(5000), notify_rust::Urgency::Low),
        };

        let mut notification = Notification::new();
        notification
            .summary(&alert.title())
            .body(&alert.message())
            .timeout(timeout)
            .urgency(urgency)
            .appname("ccost");

        // Set icon based on alert type
        let icon = match alert {
            AlertType::DailySpendingLimit { .. } |
            AlertType::WeeklySpendingLimit { .. } |
            AlertType::MonthlySpendingLimit { .. } => "dialog-warning",
            AlertType::DailyTokenLimit { .. } => "dialog-information",
            AlertType::ModelInefficiency { .. } => "dialog-information",
            AlertType::CacheRateDropped { .. } => "dialog-warning",
            AlertType::UnusualSpike { .. } => "dialog-warning",
        };
        notification.icon(icon);

        // Add action buttons for interactive notifications (if supported)
        if matches!(alert.priority(), AlertPriority::High | AlertPriority::Critical) {
            notification.action("view", "View Details");
            notification.action("dismiss", "Dismiss");
        }

        notification.show()
            .context("Failed to show desktop notification")?;

        Ok(())
    }

    pub fn send_test_notification(&self) -> Result<()> {
        if !self.enabled {
            return Err(anyhow::anyhow!("Desktop notifications are disabled"));
        }

        Notification::new()
            .summary("ccost Alert System Test")
            .body("Desktop notifications are working correctly! You'll receive alerts when thresholds are exceeded.")
            .timeout(Timeout::Milliseconds(5000))
            .urgency(notify_rust::Urgency::Normal)
            .appname("ccost")
            .icon("dialog-information")
            .show()
            .context("Failed to show test notification")?;

        Ok(())
    }

    pub fn send_summary_notification(&self, title: &str, message: &str) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        Notification::new()
            .summary(title)
            .body(message)
            .timeout(Timeout::Milliseconds(8000))
            .urgency(notify_rust::Urgency::Low)
            .appname("ccost")
            .icon("dialog-information")
            .show()
            .context("Failed to show summary notification")?;

        Ok(())
    }

    pub fn is_available() -> bool {
        // Check if the system supports desktop notifications
        #[cfg(target_os = "linux")]
        {
            // Check if we're in a desktop environment
            std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok()
        }
        
        #[cfg(target_os = "macos")]
        {
            true // macOS always supports notifications
        }
        
        #[cfg(target_os = "windows")]
        {
            true // Windows always supports notifications
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            false // Unknown platform
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for NotificationHandler {
    fn default() -> Self {
        Self::new(Self::is_available())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alerts::thresholds::AlertType;

    #[test]
    fn test_notification_handler_creation() {
        let handler = NotificationHandler::new(true);
        assert!(handler.is_enabled());

        let handler = NotificationHandler::new(false);
        assert!(!handler.is_enabled());
    }

    #[test]
    fn test_notification_handler_enable_disable() {
        let mut handler = NotificationHandler::new(false);
        assert!(!handler.is_enabled());

        handler.set_enabled(true);
        assert!(handler.is_enabled());

        handler.set_enabled(false);
        assert!(!handler.is_enabled());
    }

    #[test]
    fn test_disabled_handler_skips_notifications() {
        let handler = NotificationHandler::new(false);
        let alert = AlertType::DailySpendingLimit {
            limit: 10.0,
            current: 15.0,
            currency: "USD".to_string(),
        };

        // Should not return an error even though notifications are disabled
        let result = handler.send_alert(&alert);
        assert!(result.is_ok());
    }

    #[test]
    fn test_alert_priority_mapping() {
        let handler = NotificationHandler::new(true);
        
        let high_priority = AlertType::DailySpendingLimit {
            limit: 10.0,
            current: 15.0,
            currency: "USD".to_string(),
        };
        assert_eq!(high_priority.priority(), AlertPriority::High);

        let medium_priority = AlertType::CacheRateDropped {
            current_rate: 0.2,
            expected_rate: 0.5,
        };
        assert_eq!(medium_priority.priority(), AlertPriority::Medium);
    }
}