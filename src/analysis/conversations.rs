use crate::models::PricingManager;
use crate::output::OutputFormat;
use crate::parser::jsonl::UsageData;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;

/// Represents a conversation with all its messages and metadata
#[derive(Debug, Clone, Serialize)]
pub struct Conversation {
    pub conversation_id: String,
    pub project_name: String,
    pub messages: Vec<UsageData>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_minutes: f64,
}

/// Analysis results for a single conversation
#[derive(Debug, Clone, Serialize)]
pub struct ConversationInsight {
    pub conversation_id: String,
    pub project_name: String,
    pub total_cost: f64,
    pub message_count: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub efficiency_score: f32,
    pub cost_per_message: f64,
    pub cost_per_token: f64,
    pub model_usage: HashMap<String, ConversationModelUsage>,
    pub optimization_opportunities: Vec<OptimizationTip>,
    pub outlier_flags: Vec<OutlierFlag>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_minutes: f64,
    pub cache_hit_rate: f32,
}

/// Model usage within a conversation
#[derive(Debug, Clone, Serialize)]
pub struct ConversationModelUsage {
    pub model_name: String,
    pub message_count: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost_usd: f64,
    pub cost_percentage: f32,
}

/// Optimization recommendations
#[derive(Debug, Clone, Serialize)]
pub struct OptimizationTip {
    pub tip_type: OptimizationType,
    pub description: String,
    pub potential_savings: Option<f64>,
    pub confidence: f32,
}

/// Types of optimization recommendations
#[derive(Debug, Clone, Serialize)]
pub enum OptimizationType {
    ModelDowngrade,
    CacheOptimization,
    MessageLength,
    ContextSwitching,
    TokenEfficiency,
}

/// Flags for outlier detection
#[derive(Debug, Clone, Serialize)]
pub struct OutlierFlag {
    pub flag_type: OutlierType,
    pub description: String,
    pub severity: OutlierSeverity,
    pub metric_value: f64,
    pub threshold: f64,
}

/// Types of outliers
#[derive(Debug, Clone, Serialize)]
pub enum OutlierType {
    HighCost,
    LongConversation,
    HighTokenUsage,
    LowEfficiency,
    ExpensiveModel,
    PoorCacheHit,
}

/// Severity levels for outliers
#[derive(Debug, Clone, Serialize)]
pub enum OutlierSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Sort options for conversations
#[derive(Debug, Clone)]
pub enum ConversationSortBy {
    Cost,
    Tokens,
    Efficiency,
    Messages,
    Duration,
    StartTime,
}

/// Filter options for conversations
#[derive(Debug, Clone)]
pub struct ConversationFilter {
    pub project_name: Option<String>,
    pub model_name: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub min_cost: Option<f64>,
    pub max_cost: Option<f64>,
    pub outliers_only: bool,
    pub min_efficiency: Option<f32>,
    pub max_efficiency: Option<f32>,
}

/// Main analyzer for conversation efficiency
pub struct ConversationAnalyzer {
    efficiency_calculator: EfficiencyCalculator,
    outlier_detector: OutlierDetector,
    recommendation_engine: RecommendationEngine,
}

/// Calculates efficiency scores for conversations
pub struct EfficiencyCalculator {
    // Weight factors for different metrics
    cost_weight: f32,
    token_weight: f32,
    cache_weight: f32,
    message_weight: f32,
}

/// Detects outlier conversations
pub struct OutlierDetector {
    // Thresholds for outlier detection
    high_cost_threshold: f64,
    high_token_threshold: u64,
    low_efficiency_threshold: f32,
    poor_cache_hit_threshold: f32,
}

/// Generates optimization recommendations
pub struct RecommendationEngine {
    pricing_manager: PricingManager,
}

impl ConversationAnalyzer {
    pub fn new() -> Self {
        Self {
            efficiency_calculator: EfficiencyCalculator::new(),
            outlier_detector: OutlierDetector::new(),
            recommendation_engine: RecommendationEngine::new(),
        }
    }

    pub fn with_pricing_manager(pricing_manager: PricingManager) -> Self {
        Self {
            efficiency_calculator: EfficiencyCalculator::new(),
            outlier_detector: OutlierDetector::new(),
            recommendation_engine: RecommendationEngine::with_pricing_manager(pricing_manager),
        }
    }

    /// Groups usage data into conversations by UUID
    pub fn group_into_conversations(
        &self,
        usage_data: Vec<(UsageData, String)>,
    ) -> Result<Vec<Conversation>> {
        let mut conversation_map: HashMap<String, Vec<(UsageData, String)>> = HashMap::new();

        // Group messages by conversation UUID
        for (data, project_name) in usage_data {
            if let Some(uuid) = &data.uuid {
                conversation_map
                    .entry(uuid.clone())
                    .or_insert_with(Vec::new)
                    .push((data, project_name));
            }
        }

        let mut conversations = Vec::new();

        for (conversation_id, mut messages) in conversation_map {
            if messages.is_empty() {
                continue;
            }

            // Sort messages by timestamp
            messages.sort_by(|a, b| {
                let time_a =
                    a.0.timestamp
                        .as_ref()
                        .and_then(|t| self.parse_timestamp(t))
                        .unwrap_or(Utc::now());
                let time_b =
                    b.0.timestamp
                        .as_ref()
                        .and_then(|t| self.parse_timestamp(t))
                        .unwrap_or(Utc::now());
                time_a.cmp(&time_b)
            });

            let project_name = messages[0].1.clone();
            let usage_messages: Vec<UsageData> =
                messages.into_iter().map(|(data, _)| data).collect();

            // Calculate time span
            let start_time = usage_messages
                .first()
                .and_then(|m| m.timestamp.as_ref())
                .and_then(|t| self.parse_timestamp(t))
                .unwrap_or(Utc::now());

            let end_time = usage_messages
                .last()
                .and_then(|m| m.timestamp.as_ref())
                .and_then(|t| self.parse_timestamp(t))
                .unwrap_or(start_time);

            let duration_minutes = (end_time - start_time).num_minutes() as f64;

            conversations.push(Conversation {
                conversation_id,
                project_name,
                messages: usage_messages,
                start_time,
                end_time,
                duration_minutes,
            });
        }

        Ok(conversations)
    }

    /// Analyzes conversations and returns insights
    pub fn analyze_conversations(
        &self,
        conversations: Vec<Conversation>,
    ) -> Result<Vec<ConversationInsight>> {
        let mut insights = Vec::new();

        for conversation in conversations {
            let insight = self.analyze_single_conversation(conversation)?;
            insights.push(insight);
        }

        Ok(insights)
    }

    /// Analyzes a single conversation
    pub fn analyze_single_conversation(
        &self,
        conversation: Conversation,
    ) -> Result<ConversationInsight> {
        // Calculate basic metrics
        let mut total_cost = 0.0;
        let mut total_input_tokens = 0;
        let mut total_output_tokens = 0;
        let mut total_cache_creation_tokens = 0;
        let mut total_cache_read_tokens = 0;
        let mut model_usage: HashMap<String, ConversationModelUsage> = HashMap::new();

        for message in &conversation.messages {
            // Extract cost
            if let Some(cost) = message.cost_usd {
                total_cost += cost;
            }

            // Extract tokens
            if let Some(usage) = &message.usage {
                total_input_tokens += usage.input_tokens.unwrap_or(0);
                total_output_tokens += usage.output_tokens.unwrap_or(0);
                total_cache_creation_tokens += usage.cache_creation_input_tokens.unwrap_or(0);
                total_cache_read_tokens += usage.cache_read_input_tokens.unwrap_or(0);
            }

            // Track per-model usage
            if let Some(msg) = &message.message {
                if let Some(model_name) = &msg.model {
                    let model_entry = model_usage.entry(model_name.clone()).or_insert_with(|| {
                        ConversationModelUsage {
                            model_name: model_name.clone(),
                            message_count: 0,
                            input_tokens: 0,
                            output_tokens: 0,
                            cache_creation_tokens: 0,
                            cache_read_tokens: 0,
                            cost_usd: 0.0,
                            cost_percentage: 0.0,
                        }
                    });

                    model_entry.message_count += 1;
                    if let Some(cost) = message.cost_usd {
                        model_entry.cost_usd += cost;
                    }
                    if let Some(usage) = &message.usage {
                        model_entry.input_tokens += usage.input_tokens.unwrap_or(0);
                        model_entry.output_tokens += usage.output_tokens.unwrap_or(0);
                        model_entry.cache_creation_tokens +=
                            usage.cache_creation_input_tokens.unwrap_or(0);
                        model_entry.cache_read_tokens += usage.cache_read_input_tokens.unwrap_or(0);
                    }
                }
            }
        }

        // Calculate cost percentages for models
        for model_usage_entry in model_usage.values_mut() {
            if total_cost > 0.0 {
                model_usage_entry.cost_percentage =
                    (model_usage_entry.cost_usd / total_cost * 100.0) as f32;
            }
        }

        let message_count = conversation.messages.len() as u64;
        let cost_per_message = if message_count > 0 {
            total_cost / message_count as f64
        } else {
            0.0
        };
        let total_tokens = total_input_tokens + total_output_tokens;
        let cost_per_token = if total_tokens > 0 {
            total_cost / total_tokens as f64
        } else {
            0.0
        };

        // Calculate cache hit rate
        let cache_total = total_cache_creation_tokens + total_cache_read_tokens;
        let cache_hit_rate = if cache_total > 0 {
            total_cache_read_tokens as f32 / cache_total as f32
        } else {
            0.0
        };

        // Calculate efficiency score
        let efficiency_score = self.efficiency_calculator.calculate_efficiency(
            total_cost,
            total_tokens,
            message_count,
            cache_hit_rate,
            conversation.duration_minutes,
        );

        // Detect outliers
        let outlier_flags = self.outlier_detector.detect_outliers(
            total_cost,
            total_tokens,
            efficiency_score,
            cache_hit_rate,
            conversation.duration_minutes,
            &model_usage,
        );

        // Generate recommendations
        let optimization_opportunities = self.recommendation_engine.generate_recommendations(
            &conversation,
            total_cost,
            &model_usage,
            cache_hit_rate,
            efficiency_score,
        );

        Ok(ConversationInsight {
            conversation_id: conversation.conversation_id,
            project_name: conversation.project_name,
            total_cost,
            message_count,
            total_input_tokens,
            total_output_tokens,
            total_cache_creation_tokens,
            total_cache_read_tokens,
            efficiency_score,
            cost_per_message,
            cost_per_token,
            model_usage,
            optimization_opportunities,
            outlier_flags,
            start_time: conversation.start_time,
            end_time: conversation.end_time,
            duration_minutes: conversation.duration_minutes,
            cache_hit_rate,
        })
    }

    /// Sorts conversations by specified criteria
    pub fn sort_conversations(
        &self,
        mut insights: Vec<ConversationInsight>,
        sort_by: ConversationSortBy,
    ) -> Vec<ConversationInsight> {
        match sort_by {
            ConversationSortBy::Cost => {
                insights.sort_by(|a, b| {
                    b.total_cost
                        .partial_cmp(&a.total_cost)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            ConversationSortBy::Tokens => {
                insights.sort_by(|a, b| {
                    let total_a = a.total_input_tokens + a.total_output_tokens;
                    let total_b = b.total_input_tokens + b.total_output_tokens;
                    total_b.cmp(&total_a)
                });
            }
            ConversationSortBy::Efficiency => {
                insights.sort_by(|a, b| {
                    a.efficiency_score
                        .partial_cmp(&b.efficiency_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            ConversationSortBy::Messages => {
                insights.sort_by(|a, b| b.message_count.cmp(&a.message_count));
            }
            ConversationSortBy::Duration => {
                insights.sort_by(|a, b| {
                    b.duration_minutes
                        .partial_cmp(&a.duration_minutes)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            ConversationSortBy::StartTime => {
                insights.sort_by(|a, b| b.start_time.cmp(&a.start_time));
            }
        }
        insights
    }

    /// Filters conversations based on criteria
    pub fn filter_conversations(
        &self,
        insights: Vec<ConversationInsight>,
        filter: &ConversationFilter,
    ) -> Vec<ConversationInsight> {
        insights
            .into_iter()
            .filter(|insight| {
                // Project filter
                if let Some(ref project) = filter.project_name {
                    if insight.project_name != *project {
                        return false;
                    }
                }

                // Model filter
                if let Some(ref model) = filter.model_name {
                    if !insight.model_usage.contains_key(model) {
                        return false;
                    }
                }

                // Date filters
                if let Some(since) = filter.since {
                    if insight.start_time < since {
                        return false;
                    }
                }

                if let Some(until) = filter.until {
                    if insight.end_time > until {
                        return false;
                    }
                }

                // Cost filters
                if let Some(min_cost) = filter.min_cost {
                    if insight.total_cost < min_cost {
                        return false;
                    }
                }

                if let Some(max_cost) = filter.max_cost {
                    if insight.total_cost > max_cost {
                        return false;
                    }
                }

                // Efficiency filters
                if let Some(min_efficiency) = filter.min_efficiency {
                    if insight.efficiency_score < min_efficiency {
                        return false;
                    }
                }

                if let Some(max_efficiency) = filter.max_efficiency {
                    if insight.efficiency_score > max_efficiency {
                        return false;
                    }
                }

                // Outliers filter
                if filter.outliers_only && insight.outlier_flags.is_empty() {
                    return false;
                }

                true
            })
            .collect()
    }

    fn parse_timestamp(&self, timestamp_str: &str) -> Option<DateTime<Utc>> {
        // Try multiple timestamp formats
        if let Ok(dt) = DateTime::parse_from_rfc3339(timestamp_str) {
            return Some(dt.with_timezone(&Utc));
        }

        if let Ok(dt) = DateTime::parse_from_str(timestamp_str, "%Y-%m-%dT%H:%M:%S%.fZ") {
            return Some(dt.with_timezone(&Utc));
        }

        None
    }
}

impl EfficiencyCalculator {
    pub fn new() -> Self {
        Self {
            cost_weight: 0.4,
            token_weight: 0.3,
            cache_weight: 0.2,
            message_weight: 0.1,
        }
    }

    pub fn calculate_efficiency(
        &self,
        total_cost: f64,
        total_tokens: u64,
        message_count: u64,
        cache_hit_rate: f32,
        _duration_minutes: f64,
    ) -> f32 {
        let mut score = 100.0; // Start with perfect score

        // Cost efficiency (lower cost per token is better)
        let cost_per_token = if total_tokens > 0 {
            total_cost / total_tokens as f64
        } else {
            0.0
        };
        let cost_penalty = (cost_per_token * 1000000.0).min(50.0) as f32; // Cap at 50 point penalty
        score -= cost_penalty * self.cost_weight;

        // Token efficiency (more output tokens per input token is better)
        // This is a simplified metric - in practice you'd want more sophisticated analysis
        let token_efficiency = if total_tokens > 0 { 1.0 } else { 0.0 };
        score -= (1.0 - token_efficiency) * 20.0 * self.token_weight;

        // Cache efficiency (higher cache hit rate is better)
        let cache_penalty = (1.0 - cache_hit_rate) * 30.0;
        score -= cache_penalty * self.cache_weight;

        // Message efficiency (longer messages are generally more efficient)
        let avg_tokens_per_message = if message_count > 0 {
            total_tokens as f32 / message_count as f32
        } else {
            0.0
        };
        let message_efficiency = (avg_tokens_per_message / 1000.0).min(1.0); // Normalize to 0-1
        score -= (1.0 - message_efficiency) * 15.0 * self.message_weight;

        score.max(0.0).min(100.0)
    }
}

impl OutlierDetector {
    pub fn new() -> Self {
        Self {
            high_cost_threshold: 10.0,      // $10 per conversation
            high_token_threshold: 100_000,  // 100k tokens
            low_efficiency_threshold: 40.0, // Below 40% efficiency
            poor_cache_hit_threshold: 0.1,  // Below 10% cache hit rate
        }
    }

    pub fn detect_outliers(
        &self,
        total_cost: f64,
        total_tokens: u64,
        efficiency_score: f32,
        cache_hit_rate: f32,
        duration_minutes: f64,
        model_usage: &HashMap<String, ConversationModelUsage>,
    ) -> Vec<OutlierFlag> {
        let mut flags = Vec::new();

        // High cost detection
        if total_cost > self.high_cost_threshold {
            let severity = if total_cost > self.high_cost_threshold * 5.0 {
                OutlierSeverity::Critical
            } else if total_cost > self.high_cost_threshold * 2.0 {
                OutlierSeverity::High
            } else {
                OutlierSeverity::Medium
            };

            flags.push(OutlierFlag {
                flag_type: OutlierType::HighCost,
                description: format!(
                    "High cost conversation: ${:.2} (threshold: ${:.2})",
                    total_cost, self.high_cost_threshold
                ),
                severity,
                metric_value: total_cost,
                threshold: self.high_cost_threshold,
            });
        }

        // High token usage detection
        if total_tokens > self.high_token_threshold {
            flags.push(OutlierFlag {
                flag_type: OutlierType::HighTokenUsage,
                description: format!(
                    "High token usage: {} tokens (threshold: {} tokens)",
                    total_tokens, self.high_token_threshold
                ),
                severity: OutlierSeverity::Medium,
                metric_value: total_tokens as f64,
                threshold: self.high_token_threshold as f64,
            });
        }

        // Low efficiency detection
        if efficiency_score < self.low_efficiency_threshold {
            flags.push(OutlierFlag {
                flag_type: OutlierType::LowEfficiency,
                description: format!(
                    "Low efficiency: {:.1}% (threshold: {:.1}%)",
                    efficiency_score, self.low_efficiency_threshold
                ),
                severity: OutlierSeverity::High,
                metric_value: efficiency_score as f64,
                threshold: self.low_efficiency_threshold as f64,
            });
        }

        // Poor cache hit rate detection
        if cache_hit_rate < self.poor_cache_hit_threshold && cache_hit_rate > 0.0 {
            flags.push(OutlierFlag {
                flag_type: OutlierType::PoorCacheHit,
                description: format!(
                    "Poor cache hit rate: {:.1}% (threshold: {:.1}%)",
                    cache_hit_rate * 100.0,
                    self.poor_cache_hit_threshold * 100.0
                ),
                severity: OutlierSeverity::Medium,
                metric_value: cache_hit_rate as f64,
                threshold: self.poor_cache_hit_threshold as f64,
            });
        }

        // Long conversation detection (over 4 hours)
        if duration_minutes > 240.0 {
            flags.push(OutlierFlag {
                flag_type: OutlierType::LongConversation,
                description: format!(
                    "Long conversation: {:.1} minutes (over 4 hours)",
                    duration_minutes
                ),
                severity: OutlierSeverity::Low,
                metric_value: duration_minutes,
                threshold: 240.0,
            });
        }

        // Expensive model detection (high percentage of expensive models)
        let expensive_models = ["claude-opus-4", "claude-3-opus"];
        let expensive_cost: f64 = model_usage
            .iter()
            .filter(|(model_name, _)| expensive_models.contains(&model_name.as_str()))
            .map(|(_, usage)| usage.cost_usd)
            .sum();

        if expensive_cost > 0.0 && expensive_cost / total_cost > 0.8 {
            flags.push(OutlierFlag {
                flag_type: OutlierType::ExpensiveModel,
                description: format!(
                    "High expensive model usage: {:.1}% of total cost",
                    (expensive_cost / total_cost) * 100.0
                ),
                severity: OutlierSeverity::Medium,
                metric_value: expensive_cost / total_cost,
                threshold: 0.8,
            });
        }

        flags
    }
}

impl RecommendationEngine {
    pub fn new() -> Self {
        Self {
            pricing_manager: PricingManager::new(),
        }
    }

    pub fn with_pricing_manager(pricing_manager: PricingManager) -> Self {
        Self { pricing_manager }
    }

    pub fn generate_recommendations(
        &self,
        conversation: &Conversation,
        total_cost: f64,
        model_usage: &HashMap<String, ConversationModelUsage>,
        cache_hit_rate: f32,
        efficiency_score: f32,
    ) -> Vec<OptimizationTip> {
        let mut tips = Vec::new();

        // Model downgrade recommendations
        if let Some(opus_usage) = model_usage.get("claude-opus-4") {
            if opus_usage.cost_percentage > 70.0 {
                let potential_savings = opus_usage.cost_usd * 0.8; // Estimate 80% savings
                tips.push(OptimizationTip {
                    tip_type: OptimizationType::ModelDowngrade,
                    description: "Consider using Claude Sonnet for simpler tasks. Opus usage makes up over 70% of conversation cost.".to_string(),
                    potential_savings: Some(potential_savings),
                    confidence: 0.8,
                });
            }
        }

        // Cache optimization recommendations
        if cache_hit_rate < 0.2 && total_cost > 5.0 {
            tips.push(OptimizationTip {
                tip_type: OptimizationType::CacheOptimization,
                description: "Low cache hit rate detected. Consider structuring conversations to reuse context more effectively.".to_string(),
                potential_savings: Some(total_cost * 0.3),
                confidence: 0.6,
            });
        }

        // Token efficiency recommendations
        if efficiency_score < 50.0 {
            tips.push(OptimizationTip {
                tip_type: OptimizationType::TokenEfficiency,
                description:
                    "Low token efficiency. Consider shorter, more focused prompts and responses."
                        .to_string(),
                potential_savings: Some(total_cost * 0.2),
                confidence: 0.5,
            });
        }

        // Message length recommendations
        let avg_tokens_per_message = if conversation.messages.len() > 0 {
            let total_tokens: u64 = model_usage
                .values()
                .map(|u| u.input_tokens + u.output_tokens)
                .sum();
            total_tokens as f64 / conversation.messages.len() as f64
        } else {
            0.0
        };

        if avg_tokens_per_message < 100.0 && conversation.messages.len() > 10 {
            tips.push(OptimizationTip {
                tip_type: OptimizationType::MessageLength,
                description: "Many short messages detected. Consider consolidating related queries for better efficiency.".to_string(),
                potential_savings: Some(total_cost * 0.15),
                confidence: 0.4,
            });
        }

        tips
    }
}

/// Wrapper for conversation insights to implement OutputFormat
#[derive(Debug, Clone, Serialize)]
pub struct ConversationInsightList(pub Vec<ConversationInsight>);

impl OutputFormat for ConversationInsightList {
    fn to_table(&self) -> String {
        self.to_table_with_currency_and_color("USD", 2, false)
    }

    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.0)
    }

    fn to_table_with_currency(&self, currency: &str, decimal_places: u8) -> String {
        self.to_table_with_currency_and_color(currency, decimal_places, false)
    }

    fn to_table_with_currency_and_color(
        &self,
        currency: &str,
        decimal_places: u8,
        colored: bool,
    ) -> String {
        if self.0.is_empty() {
            return "No conversation insights found.".to_string();
        }

        use crate::output::table::{TableType, apply_table_style_with_color, format_number};
        use tabled::{Table, Tabled};

        #[derive(Tabled)]
        struct ConversationRow {
            #[tabled(rename = "Conversation ID")]
            conversation_id: String,
            #[tabled(rename = "Project")]
            project: String,
            #[tabled(rename = "Messages")]
            messages: String,
            #[tabled(rename = "Total Cost")]
            total_cost: String,
            #[tabled(rename = "Efficiency")]
            efficiency: String,
            #[tabled(rename = "Models")]
            models: String,
            #[tabled(rename = "Outliers")]
            outliers: String,
            #[tabled(rename = "Duration")]
            duration: String,
        }

        let rows: Vec<ConversationRow> = self
            .0
            .iter()
            .map(|insight| {
                let conversation_id = if insight.conversation_id.len() > 12 {
                    format!("{}...", &insight.conversation_id[..12])
                } else {
                    insight.conversation_id.clone()
                };

                let models: Vec<String> = insight.model_usage.keys().cloned().collect();
                let models_str = if models.len() > 2 {
                    format!("{}, {} (+{})", models[0], models[1], models.len() - 2)
                } else {
                    models.join(", ")
                };

                let outlier_count = insight.outlier_flags.len();
                let outliers_str = if outlier_count > 0 {
                    format!("{} issues", outlier_count)
                } else {
                    "None".to_string()
                };

                let duration_str = if insight.duration_minutes > 60.0 {
                    format!("{:.1}h", insight.duration_minutes / 60.0)
                } else {
                    format!("{:.1}m", insight.duration_minutes)
                };

                ConversationRow {
                    conversation_id,
                    project: insight.project_name.clone(),
                    messages: format_number(insight.message_count),
                    total_cost: crate::models::currency::format_currency(
                        insight.total_cost,
                        currency,
                        decimal_places,
                    ),
                    efficiency: format!("{:.1}%", insight.efficiency_score),
                    models: models_str,
                    outliers: outliers_str,
                    duration: duration_str,
                }
            })
            .collect();

        apply_table_style_with_color(Table::new(rows), colored, TableType::Conversations)
    }
}

impl Default for ConversationFilter {
    fn default() -> Self {
        Self {
            project_name: None,
            model_name: None,
            since: None,
            until: None,
            min_cost: None,
            max_cost: None,
            outliers_only: false,
            min_efficiency: None,
            max_efficiency: None,
        }
    }
}
