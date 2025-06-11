use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    pub daily_spending_limit: Option<f64>,
    pub weekly_spending_limit: Option<f64>,
    pub monthly_spending_limit: Option<f64>,
    pub daily_token_limit: Option<u64>,
    pub cache_hit_rate_threshold: Option<f32>, // Minimum cache hit rate (0.0-1.0)
    pub opus_usage_threshold: Option<u32>, // Alert if using Opus more than X times per day
    pub spending_spike_factor: Option<f32>, // Alert if daily spending is X times higher than average
    pub enabled_alerts: Vec<String>, // List of enabled alert types
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            daily_spending_limit: Some(10.0), // $10 USD per day
            weekly_spending_limit: Some(50.0), // $50 USD per week
            monthly_spending_limit: Some(200.0), // $200 USD per month
            daily_token_limit: Some(1_000_000), // 1M tokens per day
            cache_hit_rate_threshold: Some(0.3), // 30% cache hit rate
            opus_usage_threshold: Some(20), // More than 20 Opus messages per day
            spending_spike_factor: Some(3.0), // 3x higher than average
            enabled_alerts: vec![
                "daily_spending".to_string(),
                "opus_efficiency".to_string(),
                "cache_rate".to_string(),
                "spending_spike".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlertType {
    DailySpendingLimit {
        limit: f64,
        current: f64,
        currency: String,
    },
    WeeklySpendingLimit {
        limit: f64,
        current: f64,
        currency: String,
    },
    MonthlySpendingLimit {
        limit: f64,
        current: f64,
        currency: String,
    },
    DailyTokenLimit {
        limit: u64,
        current: u64,
    },
    ModelInefficiency {
        opus_uses: u32,
        potential_savings: f64,
        currency: String,
    },
    CacheRateDropped {
        current_rate: f32,
        expected_rate: f32,
    },
    UnusualSpike {
        current: f64,
        average: f64,
        factor: f32,
        currency: String,
    },
}

impl AlertType {
    pub fn alert_id(&self) -> &'static str {
        match self {
            AlertType::DailySpendingLimit { .. } => "daily_spending",
            AlertType::WeeklySpendingLimit { .. } => "weekly_spending",
            AlertType::MonthlySpendingLimit { .. } => "monthly_spending",
            AlertType::DailyTokenLimit { .. } => "daily_tokens",
            AlertType::ModelInefficiency { .. } => "opus_efficiency",
            AlertType::CacheRateDropped { .. } => "cache_rate",
            AlertType::UnusualSpike { .. } => "spending_spike",
        }
    }

    pub fn title(&self) -> String {
        match self {
            AlertType::DailySpendingLimit { limit, current, currency } => {
                format!("Daily Spending Limit Exceeded: {:.2} {} / {:.2} {}", current, currency, limit, currency)
            },
            AlertType::WeeklySpendingLimit { limit, current, currency } => {
                format!("Weekly Spending Limit Exceeded: {:.2} {} / {:.2} {}", current, currency, limit, currency)
            },
            AlertType::MonthlySpendingLimit { limit, current, currency } => {
                format!("Monthly Spending Limit Exceeded: {:.2} {} / {:.2} {}", current, currency, limit, currency)
            },
            AlertType::DailyTokenLimit { limit, current } => {
                format!("Daily Token Limit Exceeded: {} / {}", format_tokens(*current), format_tokens(*limit))
            },
            AlertType::ModelInefficiency { opus_uses, potential_savings, currency } => {
                format!("High Opus Usage: {} messages (potential savings: {:.2} {})", opus_uses, potential_savings, currency)
            },
            AlertType::CacheRateDropped { current_rate, expected_rate } => {
                format!("Low Cache Hit Rate: {:.1}% (expected: {:.1}%)", current_rate * 100.0, expected_rate * 100.0)
            },
            AlertType::UnusualSpike { current, average, factor, currency } => {
                format!("Unusual Spending Spike: {:.2} {} ({:.1}x higher than {:.2} {} average)", 
                    current, currency, factor, average, currency)
            },
        }
    }

    pub fn message(&self) -> String {
        match self {
            AlertType::DailySpendingLimit { limit, current, currency } => {
                format!("Your daily Claude usage has reached {:.2} {}, exceeding your limit of {:.2} {}. Consider reviewing your usage patterns.", 
                    current, currency, limit, currency)
            },
            AlertType::WeeklySpendingLimit { limit, current, currency } => {
                format!("Your weekly Claude usage has reached {:.2} {}, exceeding your limit of {:.2} {}.", 
                    current, currency, limit, currency)
            },
            AlertType::MonthlySpendingLimit { limit, current, currency } => {
                format!("Your monthly Claude usage has reached {:.2} {}, exceeding your limit of {:.2} {}.", 
                    current, currency, limit, currency)
            },
            AlertType::DailyTokenLimit { limit, current } => {
                format!("Your daily token usage has reached {}, exceeding your limit of {}.", 
                    format_tokens(*current), format_tokens(*limit))
            },
            AlertType::ModelInefficiency { opus_uses, potential_savings, currency } => {
                format!("You've used Claude Opus {} times today. Consider using Sonnet for simpler tasks to save approximately {:.2} {}.", 
                    opus_uses, potential_savings, currency)
            },
            AlertType::CacheRateDropped { current_rate, expected_rate } => {
                format!("Your cache hit rate is {:.1}%, below the expected {:.1}%. Try reusing conversation contexts to improve efficiency.", 
                    current_rate * 100.0, expected_rate * 100.0)
            },
            AlertType::UnusualSpike { current, average, factor, currency } => {
                format!("Today's spending of {:.2} {} is {:.1}x higher than your recent average of {:.2} {}. Check for any unusual usage patterns.", 
                    current, currency, factor, average, currency)
            },
        }
    }

    pub fn priority(&self) -> AlertPriority {
        match self {
            AlertType::DailySpendingLimit { .. } => AlertPriority::High,
            AlertType::WeeklySpendingLimit { .. } => AlertPriority::High,
            AlertType::MonthlySpendingLimit { .. } => AlertPriority::Critical,
            AlertType::DailyTokenLimit { .. } => AlertPriority::Medium,
            AlertType::ModelInefficiency { .. } => AlertPriority::Low,
            AlertType::CacheRateDropped { .. } => AlertPriority::Medium,
            AlertType::UnusualSpike { .. } => AlertPriority::Medium,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlertPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub alert_type: String,
    pub enabled: bool,
    pub last_triggered: Option<DateTime<Utc>>,
    pub cooldown_hours: u32, // Minimum hours between alerts of same type
}

impl AlertRule {
    pub fn new(alert_type: String, enabled: bool) -> Self {
        let cooldown_hours = match alert_type.as_str() {
            "daily_spending" | "daily_tokens" => 1, // 1 hour cooldown
            "weekly_spending" => 6, // 6 hour cooldown
            "monthly_spending" => 24, // 24 hour cooldown
            "opus_efficiency" => 4, // 4 hour cooldown
            "cache_rate" => 2, // 2 hour cooldown
            "spending_spike" => 1, // 1 hour cooldown
            _ => 1,
        };

        Self {
            alert_type,
            enabled,
            last_triggered: None,
            cooldown_hours,
        }
    }

    pub fn can_trigger(&self) -> bool {
        if !self.enabled {
            return false;
        }

        if let Some(last_triggered) = self.last_triggered {
            let cooldown_duration = chrono::Duration::hours(self.cooldown_hours as i64);
            let now = Utc::now();
            now - last_triggered > cooldown_duration
        } else {
            true
        }
    }

    pub fn mark_triggered(&mut self) {
        self.last_triggered = Some(Utc::now());
    }
}

fn format_tokens(tokens: u64) -> String {
    if tokens == 0 {
        return "0".to_string();
    }
    
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_type_id() {
        let alert = AlertType::DailySpendingLimit {
            limit: 10.0,
            current: 15.0,
            currency: "USD".to_string(),
        };
        assert_eq!(alert.alert_id(), "daily_spending");
    }

    #[test]
    fn test_alert_rule_cooldown() {
        let mut rule = AlertRule::new("daily_spending".to_string(), true);
        assert!(rule.can_trigger());
        
        rule.mark_triggered();
        assert!(!rule.can_trigger()); // Should be in cooldown
        
        // Manually set last_triggered to past to test cooldown expiry
        rule.last_triggered = Some(Utc::now() - chrono::Duration::hours(2));
        assert!(rule.can_trigger()); // Should be able to trigger again
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1_500), "1.5K");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn test_alert_priority() {
        let daily_limit = AlertType::DailySpendingLimit {
            limit: 10.0,
            current: 15.0,
            currency: "USD".to_string(),
        };
        assert_eq!(daily_limit.priority(), AlertPriority::High);

        let cache_rate = AlertType::CacheRateDropped {
            current_rate: 0.2,
            expected_rate: 0.5,
        };
        assert_eq!(cache_rate.priority(), AlertPriority::Medium);
    }
}