use crate::analysis::usage::ProjectUsage;
// Removed unused import: OutputFormat
use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectSortBy {
    Name,   // Default alphabetical sorting
    Cost,   // Sort by total cost (highest first)
    Tokens, // Sort by total token usage (highest first)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectSummary {
    pub project_name: String,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
    pub message_count: u64,
    pub model_count: usize,
}

impl ProjectSummary {
    pub fn from_project_usage(usage: &ProjectUsage) -> Self {
        Self {
            project_name: usage.project_name.clone(),
            total_input_tokens: usage.total_input_tokens,
            total_output_tokens: usage.total_output_tokens,
            total_cost_usd: usage.total_cost_usd,
            message_count: usage.message_count,
            model_count: usage.model_usage.len(),
        }
    }
}

pub struct ProjectAnalyzer;

impl ProjectAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze_projects(
        &self,
        project_usage: Vec<ProjectUsage>,
        sort_by: ProjectSortBy,
    ) -> Vec<ProjectSummary> {
        let mut summaries: Vec<ProjectSummary> = project_usage
            .iter()
            .map(ProjectSummary::from_project_usage)
            .collect();

        // Sort according to the specified criteria
        match sort_by {
            ProjectSortBy::Name => {
                summaries.sort_by(|a, b| a.project_name.cmp(&b.project_name));
            }
            ProjectSortBy::Cost => {
                summaries.sort_by(|a, b| {
                    b.total_cost_usd
                        .partial_cmp(&a.total_cost_usd)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            ProjectSortBy::Tokens => {
                let total_tokens_a = |summary: &ProjectSummary| {
                    summary.total_input_tokens + summary.total_output_tokens
                };
                let total_tokens_b = |summary: &ProjectSummary| {
                    summary.total_input_tokens + summary.total_output_tokens
                };
                summaries.sort_by(|a, b| total_tokens_b(b).cmp(&total_tokens_a(a)));
            }
        }

        summaries
    }

    pub fn get_project_statistics(&self, summaries: &[ProjectSummary]) -> ProjectStatistics {
        if summaries.is_empty() {
            return ProjectStatistics::default();
        }

        let total_projects = summaries.len();
        let total_cost: f64 = summaries.iter().map(|s| s.total_cost_usd).sum();
        let total_input_tokens: u64 = summaries.iter().map(|s| s.total_input_tokens).sum();
        let total_output_tokens: u64 = summaries.iter().map(|s| s.total_output_tokens).sum();
        let total_messages: u64 = summaries.iter().map(|s| s.message_count).sum();
        let total_models: usize = summaries.iter().map(|s| s.model_count).sum();

        // Find highest cost project
        let highest_cost_project = summaries
            .iter()
            .max_by(|a, b| {
                a.total_cost_usd
                    .partial_cmp(&b.total_cost_usd)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|s| s.project_name.clone());

        // Find most active project (by message count)
        let most_active_project = summaries
            .iter()
            .max_by_key(|s| s.message_count)
            .map(|s| s.project_name.clone());

        ProjectStatistics {
            total_projects,
            total_cost,
            total_input_tokens,
            total_output_tokens,
            total_messages,
            total_models,
            highest_cost_project,
            most_active_project,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectStatistics {
    pub total_projects: usize,
    pub total_cost: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_messages: u64,
    pub total_models: usize,
    pub highest_cost_project: Option<String>,
    pub most_active_project: Option<String>,
}

impl Default for ProjectStatistics {
    fn default() -> Self {
        Self {
            total_projects: 0,
            total_cost: 0.0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_messages: 0,
            total_models: 0,
            highest_cost_project: None,
            most_active_project: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::usage::ModelUsage;
    use std::collections::HashMap;

    fn create_test_project_usage(
        name: &str,
        input_tokens: u64,
        output_tokens: u64,
        cost: f64,
        messages: u64,
        models: &[&str],
    ) -> ProjectUsage {
        let mut model_usage = HashMap::new();

        for model_name in models {
            model_usage.insert(
                model_name.to_string(),
                ModelUsage {
                    model_name: model_name.to_string(),
                    input_tokens: input_tokens / models.len() as u64,
                    output_tokens: output_tokens / models.len() as u64,
                    cache_creation_tokens: 0,
                    cache_read_tokens: 0,
                    cost_usd: cost / models.len() as f64,
                    message_count: messages / models.len() as u64,
                },
            );
        }

        ProjectUsage {
            project_name: name.to_string(),
            total_input_tokens: input_tokens,
            total_output_tokens: output_tokens,
            total_cache_creation_tokens: 0,
            total_cache_read_tokens: 0,
            total_cost_usd: cost,
            model_usage,
            message_count: messages,
        }
    }

    #[test]
    fn test_project_summary_creation() {
        let project_usage = create_test_project_usage(
            "test-project",
            1000,
            500,
            2.5,
            10,
            &["claude-sonnet-4", "claude-opus-4"],
        );

        let summary = ProjectSummary::from_project_usage(&project_usage);

        assert_eq!(summary.project_name, "test-project");
        assert_eq!(summary.total_input_tokens, 1000);
        assert_eq!(summary.total_output_tokens, 500);
        assert_eq!(summary.total_cost_usd, 2.5);
        assert_eq!(summary.message_count, 10);
        assert_eq!(summary.model_count, 2);
    }

    #[test]
    fn test_project_analyzer_creation() {
        let _analyzer = ProjectAnalyzer::new();
        // Should not panic and create analyzer
    }

    #[test]
    fn test_sorting_by_name() {
        let analyzer = ProjectAnalyzer::new();

        let projects = vec![
            create_test_project_usage("zebra", 100, 50, 1.0, 5, &["claude-sonnet-4"]),
            create_test_project_usage("alpha", 200, 100, 2.0, 10, &["claude-sonnet-4"]),
            create_test_project_usage("beta", 150, 75, 1.5, 7, &["claude-sonnet-4"]),
        ];

        let summaries = analyzer.analyze_projects(projects, ProjectSortBy::Name);

        assert_eq!(summaries.len(), 3);
        assert_eq!(summaries[0].project_name, "alpha");
        assert_eq!(summaries[1].project_name, "beta");
        assert_eq!(summaries[2].project_name, "zebra");
    }

    #[test]
    fn test_sorting_by_cost() {
        let analyzer = ProjectAnalyzer::new();

        let projects = vec![
            create_test_project_usage("low-cost", 100, 50, 1.0, 5, &["claude-sonnet-4"]),
            create_test_project_usage("high-cost", 200, 100, 5.0, 10, &["claude-sonnet-4"]),
            create_test_project_usage("medium-cost", 150, 75, 3.0, 7, &["claude-sonnet-4"]),
        ];

        let summaries = analyzer.analyze_projects(projects, ProjectSortBy::Cost);

        assert_eq!(summaries.len(), 3);
        assert_eq!(summaries[0].project_name, "high-cost");
        assert_eq!(summaries[1].project_name, "medium-cost");
        assert_eq!(summaries[2].project_name, "low-cost");

        // Verify costs are in descending order
        assert!(summaries[0].total_cost_usd >= summaries[1].total_cost_usd);
        assert!(summaries[1].total_cost_usd >= summaries[2].total_cost_usd);
    }

    #[test]
    fn test_sorting_by_tokens() {
        let analyzer = ProjectAnalyzer::new();

        let projects = vec![
            create_test_project_usage("low-tokens", 100, 50, 1.0, 5, &["claude-sonnet-4"]), // 150 total
            create_test_project_usage("high-tokens", 1000, 500, 2.0, 10, &["claude-sonnet-4"]), // 1500 total
            create_test_project_usage("medium-tokens", 300, 200, 1.5, 7, &["claude-sonnet-4"]), // 500 total
        ];

        let summaries = analyzer.analyze_projects(projects, ProjectSortBy::Tokens);

        assert_eq!(summaries.len(), 3);
        assert_eq!(summaries[0].project_name, "high-tokens");
        assert_eq!(summaries[1].project_name, "medium-tokens");
        assert_eq!(summaries[2].project_name, "low-tokens");

        // Verify token counts are in descending order
        let total_tokens_0 = summaries[0].total_input_tokens + summaries[0].total_output_tokens;
        let total_tokens_1 = summaries[1].total_input_tokens + summaries[1].total_output_tokens;
        let total_tokens_2 = summaries[2].total_input_tokens + summaries[2].total_output_tokens;

        assert!(total_tokens_0 >= total_tokens_1);
        assert!(total_tokens_1 >= total_tokens_2);
    }

    #[test]
    fn test_project_statistics() {
        let analyzer = ProjectAnalyzer::new();

        let projects = vec![
            create_test_project_usage("project-a", 100, 50, 1.0, 5, &["claude-sonnet-4"]),
            create_test_project_usage(
                "project-b",
                200,
                100,
                3.0,
                10,
                &["claude-sonnet-4", "claude-opus-4"],
            ),
            create_test_project_usage("project-c", 300, 150, 2.0, 15, &["claude-haiku-3-5"]),
        ];

        let summaries = analyzer.analyze_projects(projects, ProjectSortBy::Name);
        let stats = analyzer.get_project_statistics(&summaries);

        assert_eq!(stats.total_projects, 3);
        assert_eq!(stats.total_cost, 6.0);
        assert_eq!(stats.total_input_tokens, 600);
        assert_eq!(stats.total_output_tokens, 300);
        assert_eq!(stats.total_messages, 30);
        assert_eq!(stats.total_models, 4); // 1 + 2 + 1
        assert_eq!(stats.highest_cost_project, Some("project-b".to_string()));
        assert_eq!(stats.most_active_project, Some("project-c".to_string()));
    }

    #[test]
    fn test_empty_project_statistics() {
        let analyzer = ProjectAnalyzer::new();
        let summaries: Vec<ProjectSummary> = vec![];
        let stats = analyzer.get_project_statistics(&summaries);

        assert_eq!(stats.total_projects, 0);
        assert_eq!(stats.total_cost, 0.0);
        assert_eq!(stats.total_input_tokens, 0);
        assert_eq!(stats.total_output_tokens, 0);
        assert_eq!(stats.total_messages, 0);
        assert_eq!(stats.total_models, 0);
        assert_eq!(stats.highest_cost_project, None);
        assert_eq!(stats.most_active_project, None);
    }

    #[test]
    fn test_single_project_statistics() {
        let analyzer = ProjectAnalyzer::new();

        let projects = vec![create_test_project_usage(
            "only-project",
            1000,
            500,
            5.5,
            20,
            &["claude-sonnet-4", "claude-opus-4"],
        )];

        let summaries = analyzer.analyze_projects(projects, ProjectSortBy::Name);
        let stats = analyzer.get_project_statistics(&summaries);

        assert_eq!(stats.total_projects, 1);
        assert_eq!(stats.total_cost, 5.5);
        assert_eq!(stats.total_input_tokens, 1000);
        assert_eq!(stats.total_output_tokens, 500);
        assert_eq!(stats.total_messages, 20);
        assert_eq!(stats.total_models, 2);
        assert_eq!(stats.highest_cost_project, Some("only-project".to_string()));
        assert_eq!(stats.most_active_project, Some("only-project".to_string()));
    }

    #[test]
    fn test_equal_cost_projects_sorting() {
        let analyzer = ProjectAnalyzer::new();

        let projects = vec![
            create_test_project_usage("project-a", 100, 50, 2.0, 5, &["claude-sonnet-4"]),
            create_test_project_usage("project-b", 200, 100, 2.0, 10, &["claude-sonnet-4"]),
        ];

        let summaries = analyzer.analyze_projects(projects, ProjectSortBy::Cost);

        assert_eq!(summaries.len(), 2);
        // When costs are equal, order should be stable
        assert!(summaries[0].total_cost_usd == summaries[1].total_cost_usd);
    }

    #[test]
    fn test_edge_case_zero_values() {
        let analyzer = ProjectAnalyzer::new();

        let projects = vec![
            create_test_project_usage("empty-project", 0, 0, 0.0, 0, &[]),
            create_test_project_usage("normal-project", 100, 50, 1.0, 5, &["claude-sonnet-4"]),
        ];

        let summaries = analyzer.analyze_projects(projects, ProjectSortBy::Cost);
        let stats = analyzer.get_project_statistics(&summaries);

        assert_eq!(summaries.len(), 2);
        assert_eq!(stats.total_projects, 2);
        assert_eq!(stats.total_cost, 1.0);
        assert_eq!(
            stats.highest_cost_project,
            Some("normal-project".to_string())
        );
    }
}
