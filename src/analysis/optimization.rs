//! Model usage optimization engine
//!
//! Analyzes historical model usage patterns to identify opportunities
//! for cost savings by suggesting more appropriate model choices.

use crate::models::PricingManager;
use crate::parser::jsonl::UsageData;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Confidence level for optimization recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfidenceLevel {
    Low,    // 0.0 - 0.4
    Medium, // 0.4 - 0.7
    High,   // 0.7 - 1.0
}

impl From<f32> for ConfidenceLevel {
    fn from(score: f32) -> Self {
        if score < 0.4 {
            ConfidenceLevel::Low
        } else if score < 0.7 {
            ConfidenceLevel::Medium
        } else {
            ConfidenceLevel::High
        }
    }
}

/// Analysis pattern for a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationPattern {
    pub uuid: String,
    pub message_count: u32,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub average_input_length: f64,
    pub average_output_length: f64,
    pub duration_minutes: Option<f64>,
    pub has_code_generation: bool,
    pub has_complex_reasoning: bool,
    pub is_simple_qa: bool,
    pub current_model: String,
    pub total_cost: f64,
}

/// Optimization recommendation for a conversation pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRecommendation {
    pub conversation_pattern: String,
    pub current_model: String,
    pub suggested_model: String,
    pub confidence_score: f32,
    pub confidence_level: ConfidenceLevel,
    pub potential_savings: f64,
    pub potential_savings_percentage: f64,
    pub reasoning: String,
    pub conversation_count: u32,
    pub total_current_cost: f64,
    pub total_potential_cost: f64,
}

/// Aggregated optimization results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSummary {
    pub total_conversations_analyzed: u32,
    pub total_current_cost: f64,
    pub total_potential_cost: f64,
    pub total_potential_savings: f64,
    pub savings_percentage: f64,
    pub recommendations: Vec<OptimizationRecommendation>,
    pub model_distribution: HashMap<String, u32>,
    pub optimization_opportunities: HashMap<String, f64>, // model -> potential savings
}

/// Pattern analyzer for detecting conversation characteristics
#[derive(Debug)]
pub struct ModelPatternAnalyzer {
    code_keywords: Vec<String>,
    simple_qa_patterns: Vec<String>,
    complex_reasoning_keywords: Vec<String>,
}

impl ModelPatternAnalyzer {
    pub fn new() -> Self {
        Self {
            code_keywords: vec![
                "function".to_string(),
                "class".to_string(),
                "def ".to_string(),
                "return".to_string(),
                "import".to_string(),
                "const ".to_string(),
                "let ".to_string(),
                "var ".to_string(),
                "```".to_string(),
                "console.log".to_string(),
                "print(".to_string(),
                "if __name__".to_string(),
                "export".to_string(),
                "module.exports".to_string(),
            ],
            simple_qa_patterns: vec![
                "what is".to_string(),
                "how do i".to_string(),
                "can you".to_string(),
                "please".to_string(),
                "explain".to_string(),
                "help me".to_string(),
                "?".to_string(),
            ],
            complex_reasoning_keywords: vec![
                "analyze".to_string(),
                "compare".to_string(),
                "evaluate".to_string(),
                "strategy".to_string(),
                "algorithm".to_string(),
                "optimization".to_string(),
                "architecture".to_string(),
                "design pattern".to_string(),
                "performance".to_string(),
                "scalability".to_string(),
                "complexity".to_string(),
                "tradeoff".to_string(),
            ],
        }
    }

    /// Analyze a conversation to extract patterns
    pub fn analyze_conversation(
        &self,
        conversation_data: &[UsageData],
    ) -> Result<ConversationPattern> {
        if conversation_data.is_empty() {
            return Err(anyhow::anyhow!("Cannot analyze empty conversation"));
        }

        let uuid = conversation_data[0]
            .uuid
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let message_count = conversation_data.len() as u32;

        // Extract tokens and model info
        let mut total_input_tokens = 0u64;
        let mut total_output_tokens = 0u64;
        let mut current_model = "unknown".to_string();
        let mut total_cost = 0.0;

        // Content analysis
        let mut has_code_generation = false;
        let mut has_complex_reasoning = false;
        let mut simple_qa_indicators = 0;
        let mut total_messages = 0;

        // Time analysis
        let mut timestamps = Vec::new();

        for usage_data in conversation_data {
            if let Some(usage) = &usage_data.usage {
                total_input_tokens += usage.input_tokens.unwrap_or(0);
                total_output_tokens += usage.output_tokens.unwrap_or(0);
            }

            if let Some(cost) = usage_data.cost_usd {
                total_cost += cost;
            }

            if let Some(message) = &usage_data.message {
                if let Some(model) = &message.model {
                    current_model = model.clone();
                }

                if let Some(content) = &message.content {
                    total_messages += 1;
                    let content_lower = content.to_lowercase();

                    // Check for code patterns
                    for keyword in &self.code_keywords {
                        if content_lower.contains(keyword) {
                            has_code_generation = true;
                            break;
                        }
                    }

                    // Check for complex reasoning
                    for keyword in &self.complex_reasoning_keywords {
                        if content_lower.contains(keyword) {
                            has_complex_reasoning = true;
                            break;
                        }
                    }

                    // Check for simple Q&A patterns
                    for pattern in &self.simple_qa_patterns {
                        if content_lower.contains(pattern) {
                            simple_qa_indicators += 1;
                            break;
                        }
                    }
                }
            }

            if let Some(timestamp_str) = &usage_data.timestamp {
                if let Ok(timestamp) = timestamp_str.parse::<DateTime<Utc>>() {
                    timestamps.push(timestamp);
                }
            }
        }

        // Calculate averages
        let average_input_length = if message_count > 0 {
            total_input_tokens as f64 / message_count as f64
        } else {
            0.0
        };

        let average_output_length = if message_count > 0 {
            total_output_tokens as f64 / message_count as f64
        } else {
            0.0
        };

        // Calculate duration
        let duration_minutes = if timestamps.len() >= 2 {
            timestamps.sort();
            let duration = timestamps
                .last()
                .unwrap()
                .signed_duration_since(*timestamps.first().unwrap());
            Some(duration.num_minutes() as f64)
        } else {
            None
        };

        // Determine if it's simple Q&A
        let is_simple_qa = !has_code_generation
            && !has_complex_reasoning
            && simple_qa_indicators >= (total_messages / 2)
            && message_count <= 5
            && total_input_tokens < 10000
            && total_output_tokens < 15000;

        Ok(ConversationPattern {
            uuid,
            message_count,
            total_input_tokens,
            total_output_tokens,
            average_input_length,
            average_output_length,
            duration_minutes,
            has_code_generation,
            has_complex_reasoning,
            is_simple_qa,
            current_model,
            total_cost,
        })
    }
}

/// Savings calculator for model switching recommendations
#[derive(Debug)]
pub struct SavingsCalculator {
    pricing_manager: PricingManager,
}

impl SavingsCalculator {
    pub fn new(pricing_manager: PricingManager) -> Self {
        Self { pricing_manager }
    }

    /// Calculate potential savings for switching models
    pub fn calculate_savings(
        &self,
        pattern: &ConversationPattern,
        suggested_model: &str,
    ) -> Result<(f64, f64)> {
        let current_pricing = self
            .pricing_manager
            .get_pricing(&pattern.current_model)
            .unwrap_or_else(|| {
                self.pricing_manager
                    .get_pricing_with_fallback(&pattern.current_model)
            });

        let suggested_pricing = self
            .pricing_manager
            .get_pricing(suggested_model)
            .unwrap_or_else(|| {
                self.pricing_manager
                    .get_pricing_with_fallback(suggested_model)
            });

        // Calculate costs for the conversation tokens
        let current_cost = current_pricing.calculate_cost(
            pattern.total_input_tokens,
            pattern.total_output_tokens,
            0, // We don't have cache info in pattern analysis
            0,
        );

        let suggested_cost = suggested_pricing.calculate_cost(
            pattern.total_input_tokens,
            pattern.total_output_tokens,
            0,
            0,
        );

        let savings = current_cost - suggested_cost;
        let savings_percentage = if current_cost > 0.0 {
            (savings / current_cost) * 100.0
        } else {
            0.0
        };

        Ok((savings, savings_percentage))
    }
}

/// Recommendation generator for model optimization
#[derive(Debug)]
pub struct RecommendationGenerator {
    pattern_analyzer: ModelPatternAnalyzer,
    savings_calculator: SavingsCalculator,
}

impl RecommendationGenerator {
    pub fn new(pricing_manager: PricingManager) -> Self {
        Self {
            pattern_analyzer: ModelPatternAnalyzer::new(),
            savings_calculator: SavingsCalculator::new(pricing_manager),
        }
    }

    /// Generate model recommendation based on conversation pattern
    pub fn suggest_model(&self, pattern: &ConversationPattern) -> (String, f32, String) {
        // Opus 4 models (high cost, high capability)
        let is_opus = pattern.current_model.contains("opus");

        // Complex reasoning or large context requires Opus
        if pattern.has_complex_reasoning && pattern.total_input_tokens > 50000 {
            return (
                "claude-opus-4-20250514".to_string(),
                0.9,
                "Complex reasoning with large context requires Opus capabilities".to_string(),
            );
        }

        // Large conversations with code generation might benefit from Sonnet
        if pattern.has_code_generation && pattern.message_count > 10 {
            return (
                "claude-sonnet-4-20250514".to_string(),
                0.8,
                "Code generation in extended conversations is well-suited for Sonnet".to_string(),
            );
        }

        // Simple Q&A can use Haiku
        if pattern.is_simple_qa {
            return (
                "claude-haiku-3-5-20241022".to_string(),
                0.9,
                "Simple question-and-answer patterns are perfect for Haiku".to_string(),
            );
        }

        // Short conversations without complex reasoning can use Haiku
        if pattern.message_count <= 3
            && !pattern.has_complex_reasoning
            && pattern.total_input_tokens < 5000
        {
            return (
                "claude-haiku-3-5-20241022".to_string(),
                0.8,
                "Short, simple conversations are cost-effective with Haiku".to_string(),
            );
        }

        // Code generation without complex reasoning can use Sonnet
        if pattern.has_code_generation && !pattern.has_complex_reasoning {
            return (
                "claude-sonnet-4-20250514".to_string(),
                0.7,
                "Code generation tasks are well-handled by Sonnet".to_string(),
            );
        }

        // Medium-length conversations without special requirements can use Sonnet
        if pattern.message_count <= 8
            && pattern.total_input_tokens < 20000
            && !pattern.has_complex_reasoning
        {
            return (
                "claude-sonnet-4-20250514".to_string(),
                0.6,
                "Standard conversations work well with Sonnet".to_string(),
            );
        }

        // If using Opus for simple tasks, recommend Sonnet
        if is_opus && !pattern.has_complex_reasoning && pattern.total_input_tokens < 30000 {
            return (
                "claude-sonnet-4-20250514".to_string(),
                0.7,
                "Opus may be overkill for this conversation pattern; Sonnet could provide similar results".to_string()
            );
        }

        // Default to current model if no clear optimization path
        (
            pattern.current_model.clone(),
            0.1,
            "Current model selection appears appropriate for this use case".to_string(),
        )
    }

    /// Generate optimization recommendation for a pattern
    pub fn generate_recommendation(
        &self,
        pattern: &ConversationPattern,
    ) -> Result<Option<OptimizationRecommendation>> {
        let (suggested_model, confidence_score, reasoning) = self.suggest_model(pattern);

        // Only recommend if it's a different model and confidence is reasonable
        if suggested_model == pattern.current_model || confidence_score < 0.3 {
            return Ok(None);
        }

        let (potential_savings, savings_percentage) = self
            .savings_calculator
            .calculate_savings(pattern, &suggested_model)?;

        // Only recommend if there are meaningful savings (at least 10% or $0.01)
        if potential_savings < 0.01 && savings_percentage < 10.0 {
            return Ok(None);
        }

        let total_potential_cost = pattern.total_cost - potential_savings;

        Ok(Some(OptimizationRecommendation {
            conversation_pattern: format!(
                "{} messages, {} input tokens",
                pattern.message_count, pattern.total_input_tokens
            ),
            current_model: pattern.current_model.clone(),
            suggested_model,
            confidence_score,
            confidence_level: confidence_score.into(),
            potential_savings,
            potential_savings_percentage: savings_percentage,
            reasoning,
            conversation_count: 1,
            total_current_cost: pattern.total_cost,
            total_potential_cost,
        }))
    }
}

/// Main optimization engine
#[derive(Debug)]
pub struct OptimizationEngine {
    pattern_analyzer: ModelPatternAnalyzer,
    savings_calculator: SavingsCalculator,
    recommendation_generator: RecommendationGenerator,
}

impl OptimizationEngine {
    pub fn new(pricing_manager: PricingManager) -> Self {
        // Create a new PricingManager without database for calculations
        let calc_pricing_manager = PricingManager::new();
        let savings_calculator = SavingsCalculator::new(calc_pricing_manager);
        let recommendation_generator = RecommendationGenerator::new(pricing_manager);

        Self {
            pattern_analyzer: ModelPatternAnalyzer::new(),
            savings_calculator,
            recommendation_generator,
        }
    }

    /// Analyze usage data and generate optimization recommendations
    pub fn analyze_optimization_opportunities(
        &self,
        usage_data: Vec<(UsageData, String)>,
    ) -> Result<OptimizationSummary> {
        // Group usage data by conversation UUID
        let mut conversations: HashMap<String, Vec<UsageData>> = HashMap::new();

        for (usage, _project) in usage_data {
            let uuid = usage
                .uuid
                .clone()
                .unwrap_or_else(|| format!("unknown-{}", uuid::Uuid::new_v4()));
            conversations
                .entry(uuid)
                .or_insert_with(Vec::new)
                .push(usage);
        }

        let mut all_recommendations = Vec::new();
        let mut total_current_cost = 0.0;
        let mut total_potential_cost = 0.0;
        let mut model_distribution = HashMap::new();
        let mut optimization_opportunities = HashMap::new();

        // Get the count before taking ownership
        let total_conversations = conversations.len() as u32;

        // Analyze each conversation
        for (_uuid, conv_data) in conversations {
            let pattern = self.pattern_analyzer.analyze_conversation(&conv_data)?;

            // Update model distribution
            *model_distribution
                .entry(pattern.current_model.clone())
                .or_insert(0) += 1;
            total_current_cost += pattern.total_cost;

            // Generate recommendation
            if let Some(recommendation) = self
                .recommendation_generator
                .generate_recommendation(&pattern)?
            {
                total_potential_cost += recommendation.total_potential_cost;

                // Update optimization opportunities by model
                let opportunity_savings = optimization_opportunities
                    .entry(pattern.current_model.clone())
                    .or_insert(0.0);
                *opportunity_savings += recommendation.potential_savings;

                all_recommendations.push(recommendation);
            } else {
                total_potential_cost += pattern.total_cost;
            }
        }

        // Group similar recommendations together
        let mut grouped_recommendations = HashMap::new();
        for rec in all_recommendations {
            let key = format!("{}→{}", rec.current_model, rec.suggested_model);
            let grouped =
                grouped_recommendations
                    .entry(key)
                    .or_insert_with(|| OptimizationRecommendation {
                        conversation_pattern: format!("Multiple conversations"),
                        current_model: rec.current_model.clone(),
                        suggested_model: rec.suggested_model.clone(),
                        confidence_score: 0.0,
                        confidence_level: ConfidenceLevel::Medium,
                        potential_savings: 0.0,
                        potential_savings_percentage: 0.0,
                        reasoning: rec.reasoning.clone(),
                        conversation_count: 0,
                        total_current_cost: 0.0,
                        total_potential_cost: 0.0,
                    });

            grouped.conversation_count += rec.conversation_count;
            grouped.potential_savings += rec.potential_savings;
            grouped.total_current_cost += rec.total_current_cost;
            grouped.total_potential_cost += rec.total_potential_cost;
        }

        // Calculate averages and update confidence for grouped recommendations
        let mut recommendations = Vec::new();
        for (_, mut rec) in grouped_recommendations {
            if rec.conversation_count > 0 {
                rec.potential_savings_percentage = if rec.total_current_cost > 0.0 {
                    (rec.potential_savings / rec.total_current_cost) * 100.0
                } else {
                    0.0
                };

                // Adjust confidence based on number of conversations
                rec.confidence_score = match rec.conversation_count {
                    1 => 0.5,
                    2..=5 => 0.7,
                    _ => 0.9,
                };
                rec.confidence_level = rec.confidence_score.into();

                recommendations.push(rec);
            }
        }

        // Sort recommendations by potential savings
        recommendations.sort_by(|a, b| {
            b.potential_savings
                .partial_cmp(&a.potential_savings)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total_potential_savings = total_current_cost - total_potential_cost;
        let savings_percentage = if total_current_cost > 0.0 {
            (total_potential_savings / total_current_cost) * 100.0
        } else {
            0.0
        };

        Ok(OptimizationSummary {
            total_conversations_analyzed: total_conversations,
            total_current_cost,
            total_potential_cost,
            total_potential_savings,
            savings_percentage,
            recommendations,
            model_distribution,
            optimization_opportunities,
        })
    }

    /// Filter recommendations by confidence threshold
    pub fn filter_by_confidence(
        &self,
        summary: OptimizationSummary,
        min_confidence: f32,
    ) -> OptimizationSummary {
        let filtered_recommendations: Vec<OptimizationRecommendation> = summary
            .recommendations
            .into_iter()
            .filter(|rec| rec.confidence_score >= min_confidence)
            .collect();

        // Recalculate totals based on filtered recommendations
        let total_filtered_savings: f64 = filtered_recommendations
            .iter()
            .map(|rec| rec.potential_savings)
            .sum();

        let total_filtered_current_cost: f64 = filtered_recommendations
            .iter()
            .map(|rec| rec.total_current_cost)
            .sum();

        let _total_filtered_potential_cost = total_filtered_current_cost - total_filtered_savings;

        let _filtered_savings_percentage = if total_filtered_current_cost > 0.0 {
            (total_filtered_savings / total_filtered_current_cost) * 100.0
        } else {
            0.0
        };

        OptimizationSummary {
            total_conversations_analyzed: summary.total_conversations_analyzed,
            total_current_cost: summary.total_current_cost,
            total_potential_cost: summary.total_potential_cost,
            total_potential_savings: summary.total_potential_savings,
            savings_percentage: summary.savings_percentage,
            recommendations: filtered_recommendations,
            model_distribution: summary.model_distribution,
            optimization_opportunities: summary.optimization_opportunities,
        }
    }

    /// Filter recommendations by specific model transition
    pub fn filter_by_model_transition(
        &self,
        summary: OptimizationSummary,
        from_model: Option<String>,
        to_model: Option<String>,
    ) -> OptimizationSummary {
        let filtered_recommendations: Vec<OptimizationRecommendation> = summary
            .recommendations
            .into_iter()
            .filter(|rec| {
                let from_match = from_model
                    .as_ref()
                    .map_or(true, |from| rec.current_model.contains(from));
                let to_match = to_model
                    .as_ref()
                    .map_or(true, |to| rec.suggested_model.contains(to));
                from_match && to_match
            })
            .collect();

        OptimizationSummary {
            recommendations: filtered_recommendations,
            ..summary
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::jsonl::{Message, Usage};

    fn create_test_usage_data(
        model: &str,
        content: &str,
        input_tokens: u64,
        output_tokens: u64,
        cost: f64,
    ) -> UsageData {
        UsageData {
            timestamp: Some("2025-06-09T10:00:00Z".to_string()),
            uuid: Some("test-uuid".to_string()),
            request_id: Some("req-1".to_string()),
            session_id: Some("test-session-123".to_string()),
            message: Some(Message {
                id: None,
                content: Some(content.to_string()),
                model: Some(model.to_string()),
                role: Some("user".to_string()),
                usage: None,
            }),
            usage: Some(Usage {
                input_tokens: Some(input_tokens),
                output_tokens: Some(output_tokens),
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            }),
            cost_usd: Some(cost),
            cwd: None,
            original_cwd: None,
        }
    }

    #[test]
    fn test_pattern_analyzer_simple_qa() {
        let analyzer = ModelPatternAnalyzer::new();

        let usage_data = vec![
            create_test_usage_data(
                "claude-opus-4",
                "What is the capital of France?",
                100,
                50,
                1.0,
            ),
            create_test_usage_data(
                "claude-opus-4",
                "Can you help me understand this concept?",
                150,
                75,
                1.5,
            ),
        ];

        let pattern = analyzer.analyze_conversation(&usage_data).unwrap();

        assert_eq!(pattern.message_count, 2);
        assert!(pattern.is_simple_qa);
        assert!(!pattern.has_code_generation);
        assert!(!pattern.has_complex_reasoning);
    }

    #[test]
    fn test_pattern_analyzer_code_generation() {
        let analyzer = ModelPatternAnalyzer::new();

        let usage_data = vec![
            create_test_usage_data(
                "claude-sonnet-4",
                "Write a function to sort an array",
                200,
                300,
                2.0,
            ),
            create_test_usage_data(
                "claude-sonnet-4",
                "```python\ndef sort_array(arr):\n    return sorted(arr)\n```",
                150,
                400,
                2.5,
            ),
        ];

        let pattern = analyzer.analyze_conversation(&usage_data).unwrap();

        assert!(pattern.has_code_generation);
        assert!(!pattern.is_simple_qa);
    }

    #[test]
    fn test_recommendation_generator_opus_to_sonnet() {
        let pricing_manager = PricingManager::new();
        let generator = RecommendationGenerator::new(pricing_manager);

        let pattern = ConversationPattern {
            uuid: "test".to_string(),
            message_count: 3,
            total_input_tokens: 5000,
            total_output_tokens: 2000,
            average_input_length: 1666.0,
            average_output_length: 666.0,
            duration_minutes: Some(10.0),
            has_code_generation: true,
            has_complex_reasoning: false,
            is_simple_qa: false,
            current_model: "claude-opus-4-20250514".to_string(),
            total_cost: 5.0,
        };

        let (suggested_model, confidence, reasoning) = generator.suggest_model(&pattern);

        assert_eq!(suggested_model, "claude-sonnet-4-20250514");
        assert!(confidence > 0.5);
        assert!(reasoning.contains("Code generation"));
    }

    #[test]
    fn test_recommendation_generator_simple_qa_to_haiku() {
        let pricing_manager = PricingManager::new();
        let generator = RecommendationGenerator::new(pricing_manager);

        let pattern = ConversationPattern {
            uuid: "test".to_string(),
            message_count: 2,
            total_input_tokens: 200,
            total_output_tokens: 100,
            average_input_length: 100.0,
            average_output_length: 50.0,
            duration_minutes: Some(2.0),
            has_code_generation: false,
            has_complex_reasoning: false,
            is_simple_qa: true,
            current_model: "claude-opus-4-20250514".to_string(),
            total_cost: 3.0,
        };

        let (suggested_model, confidence, reasoning) = generator.suggest_model(&pattern);

        assert_eq!(suggested_model, "claude-haiku-3-5-20241022");
        assert!(confidence > 0.8);
        assert!(reasoning.contains("Simple question-and-answer"));
    }

    #[test]
    fn test_savings_calculator() {
        let pricing_manager = PricingManager::new();
        let calculator = SavingsCalculator::new(pricing_manager);

        let pattern = ConversationPattern {
            uuid: "test".to_string(),
            message_count: 2,
            total_input_tokens: 1000000,  // 1M tokens
            total_output_tokens: 1000000, // 1M tokens
            average_input_length: 500000.0,
            average_output_length: 500000.0,
            duration_minutes: Some(5.0),
            has_code_generation: false,
            has_complex_reasoning: false,
            is_simple_qa: true,
            current_model: "claude-opus-4-20250514".to_string(),
            total_cost: 90.0, // Opus: 15 + 75 = 90
        };

        let (savings, percentage) = calculator
            .calculate_savings(&pattern, "claude-haiku-3-5-20241022")
            .unwrap();

        // Opus: 15 + 75 = 90, Haiku: 1 + 5 = 6, Savings: 84
        assert!((savings - 84.0).abs() < 0.1);
        assert!((percentage - 93.33).abs() < 0.1);
    }

    #[test]
    fn test_optimization_engine_integration() {
        let pricing_manager = PricingManager::new();
        let engine = OptimizationEngine::new(pricing_manager);

        let usage_data = vec![
            (
                create_test_usage_data("claude-opus-4-20250514", "What is 2+2?", 100, 50, 1.0),
                "project1".to_string(),
            ),
            (
                create_test_usage_data("claude-opus-4-20250514", "Can you help me?", 150, 75, 1.5),
                "project1".to_string(),
            ),
        ];

        let summary = engine
            .analyze_optimization_opportunities(usage_data)
            .unwrap();

        assert_eq!(summary.total_conversations_analyzed, 1);
        assert!(summary.total_potential_savings > 0.0);
        assert!(!summary.recommendations.is_empty());
        assert!(
            summary
                .model_distribution
                .contains_key("claude-opus-4-20250514")
        );
    }

    #[test]
    fn test_confidence_level_conversion() {
        assert!(matches!(ConfidenceLevel::from(0.2), ConfidenceLevel::Low));
        assert!(matches!(
            ConfidenceLevel::from(0.5),
            ConfidenceLevel::Medium
        ));
        assert!(matches!(ConfidenceLevel::from(0.8), ConfidenceLevel::High));
    }
}
