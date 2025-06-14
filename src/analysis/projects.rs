use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectSummary {
    pub project_name: String,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
    pub message_count: u64,
    pub model_count: usize,
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
