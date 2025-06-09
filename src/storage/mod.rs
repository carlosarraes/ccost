// Database operations module
pub mod sqlite;
pub mod migrations;

// Re-export key types for easier access
pub use sqlite::Database;

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_database_integration_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("integration_test.db");
        
        // Create database - should automatically initialize schema
        let db = Database::new(&db_path).unwrap();
        
        // Database file should exist
        assert!(db_path.exists());
        
        // Test basic deduplication workflow
        let message_hash = "integration_test_hash";
        let project_name = "integration_project";
        
        // Message should not be processed initially
        assert!(!db.is_message_processed(message_hash).unwrap());
        
        // Mark message as processed
        db.mark_message_processed(message_hash, project_name, Some("session_1")).unwrap();
        
        // Message should now be processed
        assert!(db.is_message_processed(message_hash).unwrap());
        
        // Re-opening database should preserve state
        drop(db);
        let db2 = Database::new(&db_path).unwrap();
        assert!(db2.is_message_processed(message_hash).unwrap());
    }
}