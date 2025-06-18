use std::collections::HashMap;
use std::sync::Mutex;

/// Privacy utility for generating consistent dummy project names
/// Uses a deterministic mapping to ensure the same real project name
/// always maps to the same dummy name within a session
pub struct PrivacyManager {
    name_mapping: Mutex<HashMap<String, String>>,
}

impl PrivacyManager {
    pub fn new() -> Self {
        Self {
            name_mapping: Mutex::new(HashMap::new()),
        }
    }

    /// Get a dummy project name for the given real project name
    /// Returns consistent dummy names for the same input
    pub fn get_dummy_project_name(&self, real_name: &str) -> String {
        let mut mapping = self.name_mapping.lock().unwrap();

        // Return existing mapping if it exists
        if let Some(dummy_name) = mapping.get(real_name) {
            return dummy_name.clone();
        }

        // Generate new dummy name
        let dummy_name = self.generate_dummy_name(mapping.len() + 1);
        mapping.insert(real_name.to_string(), dummy_name.clone());

        dummy_name
    }

    /// Generate a dummy project name based on index
    fn generate_dummy_name(&self, index: usize) -> String {
        const PROJECT_NAMES: &[&str] = &[
            "project-alpha",
            "project-beta",
            "project-gamma",
            "project-delta",
            "project-epsilon",
            "project-zeta",
            "project-eta",
            "project-theta",
            "project-iota",
            "project-kappa",
            "project-lambda",
            "project-mu",
            "project-nu",
            "project-xi",
            "project-omicron",
            "project-pi",
            "project-rho",
            "project-sigma",
            "project-tau",
            "project-upsilon",
            "project-phi",
            "project-chi",
            "project-psi",
            "project-omega",
        ];

        if index <= PROJECT_NAMES.len() {
            PROJECT_NAMES[index - 1].to_string()
        } else {
            // For more than 24 projects, use numbered format
            format!("project-{:02}", index)
        }
    }
}

impl Default for PrivacyManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global privacy manager instance
static PRIVACY_MANAGER: std::sync::LazyLock<PrivacyManager> =
    std::sync::LazyLock::new(PrivacyManager::new);

/// Apply privacy transformation to project name if hidden flag is enabled
pub fn maybe_hide_project_name(project_name: &str, hidden: bool) -> String {
    if hidden {
        PRIVACY_MANAGER.get_dummy_project_name(project_name)
    } else {
        project_name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consistent_dummy_names() {
        let manager = PrivacyManager::new();

        // Same input should produce same output
        let name1 = manager.get_dummy_project_name("my-secret-project");
        let name2 = manager.get_dummy_project_name("my-secret-project");
        assert_eq!(name1, name2);

        // Different inputs should produce different outputs
        let name3 = manager.get_dummy_project_name("another-project");
        assert_ne!(name1, name3);
    }

    #[test]
    fn test_dummy_name_generation() {
        let manager = PrivacyManager::new();

        // First few should use Greek letters
        assert_eq!(manager.generate_dummy_name(1), "project-alpha");
        assert_eq!(manager.generate_dummy_name(2), "project-beta");
        assert_eq!(manager.generate_dummy_name(24), "project-omega");

        // Beyond 24 should use numbers
        assert_eq!(manager.generate_dummy_name(25), "project-25");
        assert_eq!(manager.generate_dummy_name(100), "project-100");
    }

    #[test]
    fn test_multiple_projects_mapping() {
        let manager = PrivacyManager::new();

        let projects = vec!["project-a", "project-b", "project-c"];
        let mut dummy_names = Vec::new();

        for project in &projects {
            dummy_names.push(manager.get_dummy_project_name(project));
        }

        // All dummy names should be different
        assert_eq!(dummy_names.len(), 3);
        assert_ne!(dummy_names[0], dummy_names[1]);
        assert_ne!(dummy_names[1], dummy_names[2]);
        assert_ne!(dummy_names[0], dummy_names[2]);

        // Should start with expected names
        assert_eq!(dummy_names[0], "project-alpha");
        assert_eq!(dummy_names[1], "project-beta");
        assert_eq!(dummy_names[2], "project-gamma");
    }

    #[test]
    fn test_maybe_hide_project_name() {
        // When hidden=false, should return original
        assert_eq!(
            maybe_hide_project_name("real-project", false),
            "real-project"
        );

        // When hidden=true, should return dummy name
        let dummy1 = maybe_hide_project_name("real-project", true);
        let dummy2 = maybe_hide_project_name("real-project", true);

        // Should be consistent
        assert_eq!(dummy1, dummy2);
        assert_ne!(dummy1, "real-project");

        // Should be a valid dummy name
        assert!(dummy1.starts_with("project-"));
    }
}
