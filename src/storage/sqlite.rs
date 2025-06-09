use std::path::Path;
use rusqlite::Connection;
use anyhow::{Result, Context};
use crate::storage::migrations::apply_migrations;

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn new(path: &Path) -> Result<Self> {
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create database directory: {}", parent.display()))?;
        }

        // Open or create database
        let connection = Connection::open(path)
            .with_context(|| format!("Failed to open database at: {}", path.display()))?;

        // Configure SQLite for better performance and reliability
        connection.execute_batch("
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA cache_size = 1000;
            PRAGMA temp_store = memory;
        ").context("Failed to configure SQLite pragmas")?;

        let db = Database { connection };

        // Apply migrations to ensure schema is up to date
        db.init_schema()?;

        Ok(db)
    }

    pub fn init_schema(&self) -> Result<()> {
        apply_migrations(&self.connection)
            .context("Failed to apply database migrations")
    }

    pub fn is_message_processed(&self, message_hash: &str) -> Result<bool> {
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM processed_messages WHERE message_hash = ?1)",
            [message_hash],
            |row| row.get(0)
        ).context("Failed to check if message is processed")?;

        Ok(exists)
    }

    pub fn mark_message_processed(&self, message_hash: &str, project_name: &str, session_id: Option<&str>) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        
        self.connection.execute(
            "INSERT OR IGNORE INTO processed_messages (message_hash, project_name, session_id, processed_at) VALUES (?1, ?2, ?3, ?4)",
            [Some(message_hash), Some(project_name), session_id, Some(&now)]
        ).context("Failed to mark message as processed")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Database) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).unwrap();
        (temp_dir, db)
    }

    #[test]
    fn test_database_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        // Database file should not exist yet
        assert!(!db_path.exists());
        
        // Create database
        let _db = Database::new(&db_path).unwrap();
        
        // Database file should now exist
        assert!(db_path.exists());
    }

    #[test]
    fn test_schema_initialization() {
        let (_temp_dir, db) = setup_test_db();
        
        // Schema initialization should succeed
        db.init_schema().unwrap();
        
        // Verify tables exist by querying sqlite_master
        let table_count: i32 = db.connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('processed_messages', 'exchange_rates', 'model_pricing', 'usage_summary')",
            [],
            |row| row.get(0)
        ).unwrap();
        
        assert_eq!(table_count, 4);
    }

    #[test]
    fn test_message_processing_tracking() {
        let (_temp_dir, db) = setup_test_db();
        db.init_schema().unwrap();
        
        let message_hash = "test_hash_123";
        let project_name = "test_project";
        
        // Message should not be processed initially
        assert!(!db.is_message_processed(message_hash).unwrap());
        
        // Mark message as processed
        db.mark_message_processed(message_hash, project_name, Some("session_1")).unwrap();
        
        // Message should now be marked as processed
        assert!(db.is_message_processed(message_hash).unwrap());
    }

    #[test]
    fn test_duplicate_message_handling() {
        let (_temp_dir, db) = setup_test_db();
        db.init_schema().unwrap();
        
        let message_hash = "duplicate_hash";
        let project_name = "test_project";
        
        // Mark message as processed twice - should not error
        db.mark_message_processed(message_hash, project_name, None).unwrap();
        db.mark_message_processed(message_hash, project_name, None).unwrap();
        
        // Should still be marked as processed
        assert!(db.is_message_processed(message_hash).unwrap());
    }

    #[test]
    fn test_concurrent_access() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("concurrent_test.db");
        
        // Create database and initialize schema
        {
            let db = Database::new(&db_path).unwrap();
            db.init_schema().unwrap();
        }
        
        // Open multiple connections to same database
        let db1 = Database::new(&db_path).unwrap();
        let db2 = Database::new(&db_path).unwrap();
        
        // Both should be able to read/write
        db1.mark_message_processed("hash1", "project1", None).unwrap();
        db2.mark_message_processed("hash2", "project2", None).unwrap();
        
        assert!(db1.is_message_processed("hash2").unwrap());
        assert!(db2.is_message_processed("hash1").unwrap());
    }

    #[test]
    fn test_database_directory_creation() {
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir.path().join("nested").join("dirs").join("test.db");
        
        // Parent directories should not exist
        assert!(!nested_path.parent().unwrap().exists());
        
        // Creating database should create parent directories
        let _db = Database::new(&nested_path).unwrap();
        
        // Parent directories should now exist
        assert!(nested_path.parent().unwrap().exists());
        assert!(nested_path.exists());
    }
}