use std::path::Path;
use rusqlite::Connection;
use anyhow::{Result, Context};
use crate::storage::migrations::apply_migrations;
use crate::models::pricing::ModelPricing;

pub struct Database {
    connection: Connection,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("connection", &"<SQLite Connection>")
            .finish()
    }
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

    // Pricing management methods
    pub fn save_model_pricing(&self, model_name: &str, pricing: &ModelPricing) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        
        self.connection.execute(
            "INSERT OR REPLACE INTO model_pricing (model_name, input_cost_per_mtok, output_cost_per_mtok, cache_cost_per_mtok, last_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![model_name, pricing.input_cost_per_mtok, pricing.output_cost_per_mtok, pricing.cache_cost_per_mtok, now]
        ).context("Failed to save model pricing")?;

        Ok(())
    }

    pub fn get_model_pricing(&self, model_name: &str) -> Result<Option<ModelPricing>> {
        let mut stmt = self.connection.prepare(
            "SELECT input_cost_per_mtok, output_cost_per_mtok, cache_cost_per_mtok FROM model_pricing WHERE model_name = ?1"
        ).context("Failed to prepare pricing query")?;

        let result = stmt.query_row([model_name], |row| {
            Ok(ModelPricing::new(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?
            ))
        });

        match result {
            Ok(pricing) => Ok(Some(pricing)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into())
        }
    }

    pub fn list_model_pricing(&self) -> Result<Vec<(String, ModelPricing)>> {
        let mut stmt = self.connection.prepare(
            "SELECT model_name, input_cost_per_mtok, output_cost_per_mtok, cache_cost_per_mtok FROM model_pricing ORDER BY model_name"
        ).context("Failed to prepare pricing list query")?;

        let pricing_iter = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ModelPricing::new(
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?
                )
            ))
        }).context("Failed to execute pricing list query")?;

        let mut results = Vec::new();
        for pricing in pricing_iter {
            results.push(pricing.context("Failed to parse pricing row")?);
        }

        Ok(results)
    }

    pub fn delete_model_pricing(&self, model_name: &str) -> Result<bool> {
        let rows_affected = self.connection.execute(
            "DELETE FROM model_pricing WHERE model_name = ?1",
            [model_name]
        ).context("Failed to delete model pricing")?;

        Ok(rows_affected > 0)
    }

    // Exchange rate management methods
    pub fn get_exchange_rate(&self, base_currency: &str, target_currency: &str) -> Result<Option<(f64, String)>> {
        let mut stmt = self.connection.prepare(
            "SELECT rate, fetched_at FROM exchange_rates WHERE base_currency = ?1 AND target_currency = ?2"
        ).context("Failed to prepare exchange rate query")?;

        let result = stmt.query_row([base_currency, target_currency], |row| {
            Ok((
                row.get::<_, f64>(0)?,
                row.get::<_, String>(1)?,
            ))
        });

        match result {
            Ok(data) => Ok(Some(data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into())
        }
    }

    pub fn save_exchange_rate(&self, base_currency: &str, target_currency: &str, rate: f64, fetched_at: &str) -> Result<()> {
        self.connection.execute(
            "INSERT OR REPLACE INTO exchange_rates (base_currency, target_currency, rate, fetched_at) VALUES (?1, ?2, ?3, ?4)",
            [base_currency, target_currency, &rate.to_string(), fetched_at]
        ).context("Failed to save exchange rate")?;

        Ok(())
    }

    pub fn get_supported_currencies(&self) -> Result<Vec<String>> {
        let mut stmt = self.connection.prepare(
            "SELECT DISTINCT target_currency FROM exchange_rates ORDER BY target_currency"
        ).context("Failed to prepare currency query")?;

        let currency_iter = stmt.query_map([], |row| {
            Ok(row.get::<_, String>(0)?)
        }).context("Failed to execute currency query")?;

        let mut results = Vec::new();
        for currency in currency_iter {
            results.push(currency.context("Failed to parse currency row")?);
        }

        Ok(results)
    }

    pub fn cleanup_exchange_rates(&self, cutoff_time: &str) -> Result<usize> {
        let rows_affected = self.connection.execute(
            "DELETE FROM exchange_rates WHERE fetched_at < ?1",
            [cutoff_time]
        ).context("Failed to cleanup exchange rates")?;

        Ok(rows_affected)
    }

    // Export methods for sync functionality
    pub fn list_processed_messages(&self) -> Result<Vec<(String, String, Option<String>, String)>> {
        let mut stmt = self.connection.prepare(
            "SELECT message_hash, project_name, session_id, processed_at FROM processed_messages ORDER BY processed_at"
        ).context("Failed to prepare processed messages query")?;

        let message_iter = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,    // message_hash
                row.get::<_, String>(1)?,    // project_name
                row.get::<_, Option<String>>(2)?, // session_id
                row.get::<_, String>(3)?,    // processed_at
            ))
        }).context("Failed to execute processed messages query")?;

        let mut results = Vec::new();
        for message in message_iter {
            results.push(message.context("Failed to parse processed message row")?);
        }

        Ok(results)
    }

    pub fn list_exchange_rates(&self) -> Result<Vec<(String, String, f64, String)>> {
        let mut stmt = self.connection.prepare(
            "SELECT base_currency, target_currency, rate, fetched_at FROM exchange_rates ORDER BY base_currency, target_currency"
        ).context("Failed to prepare exchange rates query")?;

        let rate_iter = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,    // base_currency
                row.get::<_, String>(1)?,    // target_currency
                row.get::<_, f64>(2)?,       // rate
                row.get::<_, String>(3)?,    // fetched_at
            ))
        }).context("Failed to execute exchange rates query")?;

        let mut results = Vec::new();
        for rate in rate_iter {
            results.push(rate.context("Failed to parse exchange rate row")?);
        }

        Ok(results)
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

    #[test]
    fn test_save_and_get_model_pricing() {
        let (_temp_dir, db) = setup_test_db();
        db.init_schema().unwrap();

        let model_name = "test-model";
        let pricing = ModelPricing::new(5.0, 25.0, 0.5);

        // Save pricing
        db.save_model_pricing(model_name, &pricing).unwrap();

        // Retrieve pricing
        let retrieved = db.get_model_pricing(model_name).unwrap().expect("Should have pricing");
        
        assert!((retrieved.input_cost_per_mtok - 5.0).abs() < 0.001);
        assert!((retrieved.output_cost_per_mtok - 25.0).abs() < 0.001);
        assert!((retrieved.cache_cost_per_mtok - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_get_nonexistent_model_pricing() {
        let (_temp_dir, db) = setup_test_db();
        db.init_schema().unwrap();

        let result = db.get_model_pricing("nonexistent-model").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_model_pricing() {
        let (_temp_dir, db) = setup_test_db();
        db.init_schema().unwrap();

        let model_name = "update-test-model";
        let original_pricing = ModelPricing::new(3.0, 15.0, 0.3);
        let updated_pricing = ModelPricing::new(4.0, 20.0, 0.4);

        // Save original pricing
        db.save_model_pricing(model_name, &original_pricing).unwrap();

        // Update pricing
        db.save_model_pricing(model_name, &updated_pricing).unwrap();

        // Verify updated pricing
        let retrieved = db.get_model_pricing(model_name).unwrap().expect("Should have pricing");
        assert!((retrieved.input_cost_per_mtok - 4.0).abs() < 0.001);
        assert!((retrieved.output_cost_per_mtok - 20.0).abs() < 0.001);
        assert!((retrieved.cache_cost_per_mtok - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_list_model_pricing() {
        let (_temp_dir, db) = setup_test_db();
        db.init_schema().unwrap();

        // Add multiple model pricing entries
        db.save_model_pricing("model-a", &ModelPricing::new(1.0, 5.0, 0.1)).unwrap();
        db.save_model_pricing("model-b", &ModelPricing::new(2.0, 10.0, 0.2)).unwrap();
        db.save_model_pricing("model-c", &ModelPricing::new(3.0, 15.0, 0.3)).unwrap();

        let all_pricing = db.list_model_pricing().unwrap();
        
        assert_eq!(all_pricing.len(), 3);
        
        // Verify ordering (should be alphabetical by model name)
        assert_eq!(all_pricing[0].0, "model-a");
        assert_eq!(all_pricing[1].0, "model-b");
        assert_eq!(all_pricing[2].0, "model-c");
        
        // Verify pricing values
        assert!((all_pricing[0].1.input_cost_per_mtok - 1.0).abs() < 0.001);
        assert!((all_pricing[1].1.input_cost_per_mtok - 2.0).abs() < 0.001);
        assert!((all_pricing[2].1.input_cost_per_mtok - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_delete_model_pricing() {
        let (_temp_dir, db) = setup_test_db();
        db.init_schema().unwrap();

        let model_name = "delete-test-model";
        let pricing = ModelPricing::new(5.0, 25.0, 0.5);

        // Save pricing
        db.save_model_pricing(model_name, &pricing).unwrap();
        assert!(db.get_model_pricing(model_name).unwrap().is_some());

        // Delete pricing
        let deleted = db.delete_model_pricing(model_name).unwrap();
        assert!(deleted);

        // Verify deletion
        assert!(db.get_model_pricing(model_name).unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent_model_pricing() {
        let (_temp_dir, db) = setup_test_db();
        db.init_schema().unwrap();

        let deleted = db.delete_model_pricing("nonexistent-model").unwrap();
        assert!(!deleted);
    }
}