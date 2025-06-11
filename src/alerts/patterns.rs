use crate::analysis::usage::ProjectUsage;
use crate::alerts::thresholds::AlertType;
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;

pub struct UsagePatternAnalyzer {
    // Historical averages for spike detection
    historical_window_days: i64,
}

impl UsagePatternAnalyzer {
    pub fn new() -> Self {
        Self {
            historical_window_days: 7, // Use 7-day rolling average
        }
    }

    /// Analyze usage patterns and generate efficiency recommendations
    pub fn analyze_patterns(&self, usage_data: &[ProjectUsage], currency: &str) -> Vec<AlertType> {
        let mut alerts = Vec::new();

        // Analyze model efficiency (Opus vs Sonnet usage)
        if let Some(efficiency_alert) = self.analyze_model_efficiency(usage_data, currency) {
            alerts.push(efficiency_alert);
        }

        // Analyze cache utilization
        if let Some(cache_alert) = self.analyze_cache_efficiency(usage_data) {
            alerts.push(cache_alert);
        }

        alerts
    }

    /// Check for spending spikes compared to historical average
    pub fn check_spending_spike(&self, current_spending: f64, daily_history: &[f64], currency: &str) -> Option<AlertType> {
        if daily_history.len() < 3 {
            return None; // Need at least 3 days of history
        }

        let average_spending: f64 = daily_history.iter().sum::<f64>() / daily_history.len() as f64;
        
        if average_spending <= 0.0 {
            return None; // No meaningful average to compare against
        }

        let spike_factor = current_spending / average_spending;
        
        // Alert if current spending is 3x or more than average and above $1
        if spike_factor >= 3.0 && current_spending > 1.0 {
            return Some(AlertType::UnusualSpike {
                current: current_spending,
                average: average_spending,
                factor: spike_factor as f32,
                currency: currency.to_string(),
            });
        }

        None
    }

    /// Analyze model usage patterns to identify inefficient Opus usage
    fn analyze_model_efficiency(&self, usage_data: &[ProjectUsage], currency: &str) -> Option<AlertType> {
        let mut opus_usage_count = 0;
        let mut total_opus_cost = 0.0;
        let mut sonnet_avg_cost = 0.0;
        let mut sonnet_message_count = 0;

        for project in usage_data {
            for (model_name, model_usage) in &project.model_usage {
                if model_name.contains("opus") {
                    opus_usage_count += model_usage.message_count;
                    total_opus_cost += model_usage.cost_usd;
                } else if model_name.contains("sonnet") {
                    sonnet_message_count += model_usage.message_count;
                    sonnet_avg_cost += model_usage.cost_usd;
                }
            }
        }

        // Calculate potential savings if user switched from Opus to Sonnet
        if opus_usage_count > 20 && sonnet_message_count > 0 {
            let avg_sonnet_cost_per_message = sonnet_avg_cost / sonnet_message_count as f64;
            let avg_opus_cost_per_message = total_opus_cost / opus_usage_count as f64;
            
            if avg_opus_cost_per_message > avg_sonnet_cost_per_message * 2.0 {
                let potential_savings = (avg_opus_cost_per_message - avg_sonnet_cost_per_message) 
                    * opus_usage_count as f64;
                
                return Some(AlertType::ModelInefficiency {
                    opus_uses: opus_usage_count as u32,
                    potential_savings,
                    currency: currency.to_string(),
                });
            }
        }

        None
    }

    /// Analyze cache utilization patterns
    fn analyze_cache_efficiency(&self, usage_data: &[ProjectUsage]) -> Option<AlertType> {
        let mut total_input_tokens = 0u64;
        let mut total_cache_read_tokens = 0u64;

        for project in usage_data {
            total_input_tokens += project.total_input_tokens;
            total_cache_read_tokens += project.total_cache_read_tokens;
        }

        if total_input_tokens > 100_000 { // Only analyze if significant usage
            let cache_hit_rate = if total_input_tokens > 0 {
                total_cache_read_tokens as f32 / total_input_tokens as f32
            } else {
                0.0
            };

            let expected_rate = 0.3; // 30% is a reasonable cache hit rate
            
            if cache_hit_rate < expected_rate {
                return Some(AlertType::CacheRateDropped {
                    current_rate: cache_hit_rate,
                    expected_rate,
                });
            }
        }

        None
    }

    /// Calculate daily spending averages for spike detection
    pub fn calculate_daily_averages(&self, daily_spending: &HashMap<String, f64>) -> Vec<f64> {
        let mut values: Vec<f64> = daily_spending.values().cloned().collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        values
    }

    /// Generate weekly summary statistics
    pub fn generate_weekly_summary(&self, usage_data: &[ProjectUsage], currency: &str) -> WeeklySummary {
        let mut total_cost = 0.0;
        let mut total_messages = 0u64;
        let mut total_input_tokens = 0u64;
        let mut total_output_tokens = 0u64;
        let mut model_usage: HashMap<String, u64> = HashMap::new();

        for project in usage_data {
            total_cost += project.total_cost_usd;
            total_messages += project.message_count;
            total_input_tokens += project.total_input_tokens;
            total_output_tokens += project.total_output_tokens;

            for (model_name, model_usage_data) in &project.model_usage {
                *model_usage.entry(model_name.clone()).or_insert(0) += model_usage_data.message_count;
            }
        }

        let most_used_model = model_usage
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(model, _)| model.clone())
            .unwrap_or_else(|| "None".to_string());

        WeeklySummary {
            total_cost,
            currency: currency.to_string(),
            total_messages,
            total_input_tokens,
            total_output_tokens,
            most_used_model,
            projects_count: usage_data.len(),
        }
    }
}

impl Default for UsagePatternAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct WeeklySummary {
    pub total_cost: f64,
    pub currency: String,
    pub total_messages: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub most_used_model: String,
    pub projects_count: usize,
}

impl WeeklySummary {
    pub fn to_notification_message(&self) -> String {
        format!(
            "Weekly Claude Usage Summary:\n\
            • Total Cost: {:.2} {}\n\
            • Messages: {}\n\
            • Input Tokens: {}\n\
            • Output Tokens: {}\n\
            • Most Used Model: {}\n\
            • Active Projects: {}",
            self.total_cost,
            self.currency,
            format_number(self.total_messages),
            format_tokens(self.total_input_tokens),
            format_tokens(self.total_output_tokens),
            self.most_used_model,
            self.projects_count
        )
    }
}

fn format_number(n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    
    let mut result = String::new();
    let s = n.to_string();
    let chars: Vec<char> = s.chars().collect();
    
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*ch);
    }
    
    result
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
    use crate::analysis::usage::{ModelUsage, ProjectUsage};
    use std::collections::HashMap;

    fn create_test_project_usage(model_name: &str, message_count: u64, cost: f64, input_tokens: u64) -> ProjectUsage {
        let mut model_usage = HashMap::new();
        model_usage.insert(model_name.to_string(), ModelUsage {
            input_tokens,
            output_tokens: input_tokens / 4, // Rough 4:1 ratio
            cache_creation_tokens: 0,
            cache_read_tokens: input_tokens / 10, // 10% cache hit
            cost_usd: cost,
            message_count,
        });

        ProjectUsage {
            project_name: "test_project".to_string(),
            total_input_tokens: input_tokens,
            total_output_tokens: input_tokens / 4,
            total_cache_creation_tokens: 0,
            total_cache_read_tokens: input_tokens / 10,
            total_cost_usd: cost,
            message_count,
            model_usage,
        }
    }

    #[test]
    fn test_spending_spike_detection() {
        let analyzer = UsagePatternAnalyzer::new();
        let history = vec![1.0, 1.5, 2.0, 1.2, 1.8]; // Average ~1.5
        
        // Should trigger spike alert (6.0 is 4x the average)
        let spike_alert = analyzer.check_spending_spike(6.0, &history, "USD");
        assert!(spike_alert.is_some());
        
        if let Some(AlertType::UnusualSpike { factor, .. }) = spike_alert {
            assert!(factor >= 3.0);
        }

        // Should not trigger for normal spending
        let normal_alert = analyzer.check_spending_spike(2.0, &history, "USD");
        assert!(normal_alert.is_none());
    }

    #[test]
    fn test_model_efficiency_analysis() {
        let analyzer = UsagePatternAnalyzer::new();
        
        let usage_data = vec![
            create_test_project_usage("claude-3-opus-20240229", 25, 10.0, 100_000),
            create_test_project_usage("claude-3-sonnet-20240229", 15, 2.0, 80_000),
        ];

        let alerts = analyzer.analyze_patterns(&usage_data, "USD");
        
        // Should detect inefficient Opus usage
        let efficiency_alert = alerts.iter().find(|alert| {
            matches!(alert, AlertType::ModelInefficiency { .. })
        });
        assert!(efficiency_alert.is_some());
    }

    #[test]
    fn test_cache_efficiency_analysis() {
        let analyzer = UsagePatternAnalyzer::new();
        
        // Create usage with poor cache hit rate
        let mut usage = create_test_project_usage("claude-3-sonnet-20240229", 50, 5.0, 200_000);
        usage.total_cache_read_tokens = 5_000; // Only 2.5% cache hit rate
        
        let usage_data = vec![usage];
        let alerts = analyzer.analyze_patterns(&usage_data, "USD");
        
        // Should detect poor cache performance
        let cache_alert = alerts.iter().find(|alert| {
            matches!(alert, AlertType::CacheRateDropped { .. })
        });
        assert!(cache_alert.is_some());
    }

    #[test]
    fn test_weekly_summary_generation() {
        let analyzer = UsagePatternAnalyzer::new();
        
        let usage_data = vec![
            create_test_project_usage("claude-3-opus-20240229", 10, 5.0, 50_000),
            create_test_project_usage("claude-3-sonnet-20240229", 20, 3.0, 100_000),
        ];

        let summary = analyzer.generate_weekly_summary(&usage_data, "USD");
        
        assert_eq!(summary.total_cost, 8.0);
        assert_eq!(summary.total_messages, 30);
        assert_eq!(summary.projects_count, 2);
        assert!(summary.most_used_model.contains("sonnet"));
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1_500), "1.5K");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(1234), "1,234");
        assert_eq!(format_number(1234567), "1,234,567");
    }
}