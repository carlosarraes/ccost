use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;
use chrono::{DateTime, Utc};
use crate::storage::Database;
use crate::models::PricingManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportData {
    pub version: String,
    pub exported_at: DateTime<Utc>,
    pub processed_messages: Vec<ProcessedMessage>,
    pub model_pricing: Vec<ModelPricingEntry>,
    pub exchange_rates: Vec<ExchangeRateEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedMessage {
    pub message_hash: String,
    pub project_name: String,
    pub session_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricingEntry {
    pub model_name: String,
    pub input_cost_per_mtok: f64,
    pub output_cost_per_mtok: f64,
    pub cache_cost_per_mtok: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRateEntry {
    pub from_currency: String,
    pub to_currency: String,
    pub rate: f64,
    pub fetched_at: String,
}

#[derive(Debug)]
pub struct ExportImportManager {
    database: Database,
}

impl ExportImportManager {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    /// Export all data to JSON format
    pub fn export_to_json(&self, output_path: &Path) -> Result<ExportData> {
        let export_data = ExportData {
            version: "1.0.0".to_string(),
            exported_at: Utc::now(),
            processed_messages: self.export_processed_messages()?,
            model_pricing: self.export_model_pricing()?,
            exchange_rates: self.export_exchange_rates()?,
        };

        let json_content = serde_json::to_string_pretty(&export_data)?;
        fs::write(output_path, json_content)?;

        Ok(export_data)
    }

    /// Export all data to CSV format
    pub fn export_to_csv(&self, output_dir: &Path) -> Result<()> {
        // Create output directory if it doesn't exist
        fs::create_dir_all(output_dir)?;

        // Export processed messages to CSV
        let messages = self.export_processed_messages()?;
        let csv_path = output_dir.join("processed_messages.csv");
        self.write_processed_messages_csv(&messages, &csv_path)?;

        // Export model pricing to CSV
        let pricing = self.export_model_pricing()?;
        let pricing_path = output_dir.join("model_pricing.csv");
        self.write_model_pricing_csv(&pricing, &pricing_path)?;

        // Export exchange rates to CSV
        let rates = self.export_exchange_rates()?;
        let rates_path = output_dir.join("exchange_rates.csv");
        self.write_exchange_rates_csv(&rates, &rates_path)?;

        Ok(())
    }

    /// Import data from JSON file with optional merge
    pub fn import_from_json(&self, import_path: &Path, merge: bool) -> Result<ImportResult> {
        let json_content = fs::read_to_string(import_path)?;
        let import_data: ExportData = serde_json::from_str(&json_content)?;

        self.import_data(import_data, merge)
    }

    fn export_processed_messages(&self) -> Result<Vec<ProcessedMessage>> {
        // Note: This would need to be implemented in the Database trait
        // For now, return empty vector as placeholder
        Ok(Vec::new())
    }

    fn export_model_pricing(&self) -> Result<Vec<ModelPricingEntry>> {
        let pricing_data = self.database.list_model_pricing()?;
        let mut entries = Vec::new();

        for (model_name, pricing) in pricing_data {
            entries.push(ModelPricingEntry {
                model_name,
                input_cost_per_mtok: pricing.input_cost_per_mtok,
                output_cost_per_mtok: pricing.output_cost_per_mtok,
                cache_cost_per_mtok: pricing.cache_cost_per_mtok,
                updated_at: Utc::now().to_rfc3339(),
            });
        }

        Ok(entries)
    }

    fn export_exchange_rates(&self) -> Result<Vec<ExchangeRateEntry>> {
        // Note: This would need to be implemented in the Database trait
        // For now, return empty vector as placeholder
        Ok(Vec::new())
    }

    fn write_processed_messages_csv(&self, messages: &[ProcessedMessage], path: &Path) -> Result<()> {
        let mut csv_content = String::new();
        csv_content.push_str("message_hash,project_name,session_id,created_at\n");

        for message in messages {
            csv_content.push_str(&format!(
                "\"{}\",\"{}\",\"{}\",\"{}\"\n",
                message.message_hash,
                message.project_name,
                message.session_id.as_deref().unwrap_or(""),
                message.created_at
            ));
        }

        fs::write(path, csv_content)?;
        Ok(())
    }

    fn write_model_pricing_csv(&self, pricing: &[ModelPricingEntry], path: &Path) -> Result<()> {
        let mut csv_content = String::new();
        csv_content.push_str("model_name,input_cost_per_mtok,output_cost_per_mtok,cache_cost_per_mtok,updated_at\n");

        for entry in pricing {
            csv_content.push_str(&format!(
                "\"{}\",{},{},{},\"{}\"\n",
                entry.model_name,
                entry.input_cost_per_mtok,
                entry.output_cost_per_mtok,
                entry.cache_cost_per_mtok,
                entry.updated_at
            ));
        }

        fs::write(path, csv_content)?;
        Ok(())
    }

    fn write_exchange_rates_csv(&self, rates: &[ExchangeRateEntry], path: &Path) -> Result<()> {
        let mut csv_content = String::new();
        csv_content.push_str("from_currency,to_currency,rate,fetched_at\n");

        for entry in rates {
            csv_content.push_str(&format!(
                "\"{}\",\"{}\",{},\"{}\"\n",
                entry.from_currency,
                entry.to_currency,
                entry.rate,
                entry.fetched_at
            ));
        }

        fs::write(path, csv_content)?;
        Ok(())
    }

    fn import_data(&self, import_data: ExportData, merge: bool) -> Result<ImportResult> {
        let mut result = ImportResult {
            processed_messages_imported: 0,
            model_pricing_imported: 0,
            exchange_rates_imported: 0,
            conflicts_resolved: 0,
            errors: Vec::new(),
        };

        // If not merging, we could clear existing data first
        if !merge {
            // This would clear the database - implement with caution
            // self.database.clear_all_data()?;
        }

        // Import model pricing
        for pricing_entry in import_data.model_pricing {
            match self.database.save_model_pricing(
                &pricing_entry.model_name,
                &crate::models::ModelPricing::new(
                    pricing_entry.input_cost_per_mtok,
                    pricing_entry.output_cost_per_mtok,
                    pricing_entry.cache_cost_per_mtok,
                ),
            ) {
                Ok(()) => result.model_pricing_imported += 1,
                Err(e) => result.errors.push(format!("Failed to import pricing for {}: {}", pricing_entry.model_name, e)),
            }
        }

        // Import processed messages (placeholder)
        result.processed_messages_imported = import_data.processed_messages.len();

        // Import exchange rates (placeholder)
        result.exchange_rates_imported = import_data.exchange_rates.len();

        Ok(result)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub processed_messages_imported: usize,
    pub model_pricing_imported: usize,
    pub exchange_rates_imported: usize,
    pub conflicts_resolved: usize,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::sqlite::Database;
    use tempfile::TempDir;

    #[test]
    fn test_export_import_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).unwrap();
        
        let manager = ExportImportManager::new(database);
        
        // Just verify the manager was created
        // More comprehensive tests will be added as functionality is implemented
    }

    #[test]
    fn test_export_to_json() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).unwrap();
        
        let manager = ExportImportManager::new(database);
        let export_path = temp_dir.path().join("export.json");
        
        let result = manager.export_to_json(&export_path);
        assert!(result.is_ok());
        assert!(export_path.exists());
        
        // Verify the JSON structure
        let content = fs::read_to_string(&export_path).unwrap();
        let export_data: ExportData = serde_json::from_str(&content).unwrap();
        assert_eq!(export_data.version, "1.0.0");
    }

    #[test]
    fn test_export_to_csv() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).unwrap();
        
        let manager = ExportImportManager::new(database);
        let export_dir = temp_dir.path().join("csv_export");
        
        let result = manager.export_to_csv(&export_dir);
        assert!(result.is_ok());
        
        // Verify CSV files were created
        assert!(export_dir.join("processed_messages.csv").exists());
        assert!(export_dir.join("model_pricing.csv").exists());
        assert!(export_dir.join("exchange_rates.csv").exists());
    }

    #[test]
    fn test_import_from_json() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).unwrap();
        
        let manager = ExportImportManager::new(database);
        
        // First export some data
        let export_path = temp_dir.path().join("export.json");
        manager.export_to_json(&export_path).unwrap();
        
        // Then import it back
        let result = manager.import_from_json(&export_path, true);
        assert!(result.is_ok());
        
        let import_result = result.unwrap();
        assert_eq!(import_result.errors.len(), 0);
    }

    #[test]
    fn test_csv_formatting() {
        let pricing_entries = vec![
            ModelPricingEntry {
                model_name: "claude-test".to_string(),
                input_cost_per_mtok: 3.0,
                output_cost_per_mtok: 15.0,
                cache_cost_per_mtok: 0.3,
                updated_at: "2025-06-10T12:00:00Z".to_string(),
            }
        ];
        
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).unwrap();
        let manager = ExportImportManager::new(database);
        
        let csv_path = temp_dir.path().join("test.csv");
        manager.write_model_pricing_csv(&pricing_entries, &csv_path).unwrap();
        
        let content = fs::read_to_string(&csv_path).unwrap();
        assert!(content.contains("model_name,input_cost_per_mtok"));
        assert!(content.contains("claude-test"));
        assert!(content.contains("3,15,0.3"));
    }
}