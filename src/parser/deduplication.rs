use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::HashSet;
use std::path::Path;
use sha2::{Sha256, Digest};

use super::jsonl::UsageData;

/// Deduplication engine for handling branched conversations
/// This is the core value proposition of ccost - solving the branching problem
pub struct DeduplicationEngine {
    /// In-memory cache for fast O(1) lookups
    seen_hashes: HashSet<String>,
    /// SQLite connection for persistent storage
    connection: Option<Connection>,
}

impl DeduplicationEngine {
    /// Create a new deduplication engine with optional SQLite persistence
    pub fn new() -> Self {
        Self {
            seen_hashes: HashSet::new(),
            connection: None,
        }
    }

    /// Initialize with SQLite database for persistence
    pub fn with_database(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        
        // Create table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS processed_messages (
                hash TEXT PRIMARY KEY,
                uuid TEXT,
                request_id TEXT,
                timestamp TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Create index for faster lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_processed_messages_timestamp 
             ON processed_messages(timestamp)",
            [],
        )?;

        let mut engine = Self {
            seen_hashes: HashSet::new(),
            connection: Some(conn),
        };

        // Load existing hashes into memory
        engine.load_existing_hashes()?;
        
        Ok(engine)
    }

    /// Load existing hashes from database into memory for fast lookups
    fn load_existing_hashes(&mut self) -> Result<()> {
        if let Some(conn) = &self.connection {
            let mut stmt = conn.prepare("SELECT hash FROM processed_messages")?;
            let hash_iter = stmt.query_map([], |row| row.get(0))?;

            for hash_result in hash_iter {
                let hash: String = hash_result?;
                self.seen_hashes.insert(hash);
            }
        }
        Ok(())
    }

    /// Generate unique hash from message identifiers
    pub fn generate_hash(uuid: &Option<String>, request_id: &Option<String>) -> Option<String> {
        match (uuid, request_id) {
            (Some(u), Some(r)) => {
                // Create deterministic hash from both IDs
                let mut hasher = Sha256::new();
                hasher.update(format!("{}_{}", u, r));
                let result = hasher.finalize();
                Some(format!("{:x}", result))
            }
            _ => None, // Cannot generate reliable hash without both IDs
        }
    }

    /// Check if a message has already been processed
    pub fn is_duplicate(&self, message: &UsageData) -> bool {
        if let Some(hash) = Self::generate_hash(&message.uuid, &message.request_id) {
            self.seen_hashes.contains(&hash)
        } else {
            false // Messages without proper IDs are not considered duplicates
        }
    }

    /// Mark a message as processed
    pub fn mark_as_processed(&mut self, message: &UsageData) -> Result<bool> {
        if let Some(hash) = Self::generate_hash(&message.uuid, &message.request_id) {
            // Check if already exists
            if self.seen_hashes.contains(&hash) {
                return Ok(false); // Already processed
            }

            // Add to in-memory set
            self.seen_hashes.insert(hash.clone());

            // Persist to database if available
            if let Some(conn) = &self.connection {
                conn.execute(
                    "INSERT OR IGNORE INTO processed_messages (hash, uuid, request_id, timestamp) 
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        hash,
                        message.uuid.as_deref().unwrap_or(""),
                        message.request_id.as_deref().unwrap_or(""),
                        message.timestamp.as_deref().unwrap_or("")
                    ],
                )?;
            }

            Ok(true) // Successfully marked as processed
        } else {
            Ok(false) // Cannot mark messages without proper IDs
        }
    }

    /// Process a batch of messages, returning only non-duplicates
    pub fn filter_duplicates(&mut self, messages: Vec<UsageData>) -> Result<Vec<UsageData>> {
        let mut unique_messages = Vec::new();
        let mut stats = DeduplicationStats::default();

        for message in messages {
            stats.total_messages += 1;

            if self.is_duplicate(&message) {
                stats.duplicates_found += 1;
                continue;
            }

            if let Some(_hash) = Self::generate_hash(&message.uuid, &message.request_id) {
                self.mark_as_processed(&message)?;
                unique_messages.push(message);
                stats.unique_messages += 1;
            } else {
                stats.messages_without_ids += 1;
                // Still include messages without IDs but warn about them
                unique_messages.push(message);
            }
        }

        // Only show stats in verbose mode - moved to caller to handle verbose flag

        Ok(unique_messages)
    }

    /// Clear all processed message history (useful for testing)
    pub fn clear_history(&mut self) -> Result<()> {
        self.seen_hashes.clear();
        
        if let Some(conn) = &self.connection {
            conn.execute("DELETE FROM processed_messages", [])?;
        }
        
        Ok(())
    }

    /// Get count of processed messages
    pub fn processed_count(&self) -> usize {
        self.seen_hashes.len()
    }
}

#[derive(Default, Debug)]
struct DeduplicationStats {
    total_messages: usize,
    unique_messages: usize,
    duplicates_found: usize,
    messages_without_ids: usize,
}

impl std::fmt::Display for DeduplicationStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Total: {}, Unique: {}, Duplicates: {}, Missing IDs: {}",
            self.total_messages,
            self.unique_messages,
            self.duplicates_found,
            self.messages_without_ids
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::jsonl::{Message, Usage};
    use tempfile::NamedTempFile;

    fn create_test_message(uuid: Option<String>, request_id: Option<String>) -> UsageData {
        UsageData {
            timestamp: Some("2025-06-09T10:00:00Z".to_string()),
            uuid,
            request_id,
            message: Some(Message {
                content: Some("Test message".to_string()),
                model: Some("claude-sonnet-4".to_string()),
                role: Some("user".to_string()),
                usage: None,
            }),
            usage: Some(Usage {
                input_tokens: Some(10),
                output_tokens: Some(20),
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            }),
            cost_usd: Some(0.001),
        }
    }

    #[test]
    fn test_generate_hash_with_both_ids() {
        let hash = DeduplicationEngine::generate_hash(
            &Some("uuid-123".to_string()),
            &Some("req-456".to_string())
        );
        
        assert!(hash.is_some());
        let hash_value = hash.unwrap();
        
        // Hash should be deterministic
        let hash2 = DeduplicationEngine::generate_hash(
            &Some("uuid-123".to_string()),
            &Some("req-456".to_string())
        ).unwrap();
        
        assert_eq!(hash_value, hash2);
        
        // Different IDs should produce different hash
        let hash3 = DeduplicationEngine::generate_hash(
            &Some("uuid-789".to_string()),
            &Some("req-456".to_string())
        ).unwrap();
        
        assert_ne!(hash_value, hash3);
    }

    #[test]
    fn test_generate_hash_missing_uuid() {
        let hash = DeduplicationEngine::generate_hash(
            &None,
            &Some("req-456".to_string())
        );
        
        assert!(hash.is_none());
    }

    #[test]
    fn test_generate_hash_missing_request_id() {
        let hash = DeduplicationEngine::generate_hash(
            &Some("uuid-123".to_string()),
            &None
        );
        
        assert!(hash.is_none());
    }

    #[test]
    fn test_is_duplicate_new_message() {
        let engine = DeduplicationEngine::new();
        let message = create_test_message(
            Some("uuid-123".to_string()),
            Some("req-456".to_string())
        );
        
        assert!(!engine.is_duplicate(&message));
    }

    #[test]
    fn test_mark_as_processed() {
        let mut engine = DeduplicationEngine::new();
        let message = create_test_message(
            Some("uuid-123".to_string()),
            Some("req-456".to_string())
        );
        
        // First time should return true (newly processed)
        assert!(engine.mark_as_processed(&message).unwrap());
        
        // Now it should be a duplicate
        assert!(engine.is_duplicate(&message));
        
        // Second time should return false (already processed)
        assert!(!engine.mark_as_processed(&message).unwrap());
    }

    #[test]
    fn test_mark_as_processed_missing_ids() {
        let mut engine = DeduplicationEngine::new();
        let message = create_test_message(None, None);
        
        // Cannot mark messages without IDs
        assert!(!engine.mark_as_processed(&message).unwrap());
    }

    #[test]
    fn test_filter_duplicates_basic() {
        let mut engine = DeduplicationEngine::new();
        
        let messages = vec![
            create_test_message(Some("uuid-1".to_string()), Some("req-1".to_string())),
            create_test_message(Some("uuid-2".to_string()), Some("req-2".to_string())),
            create_test_message(Some("uuid-1".to_string()), Some("req-1".to_string())), // Duplicate
            create_test_message(Some("uuid-3".to_string()), Some("req-3".to_string())),
        ];
        
        let unique = engine.filter_duplicates(messages).unwrap();
        
        assert_eq!(unique.len(), 3); // One duplicate removed
        assert_eq!(engine.processed_count(), 3);
    }

    #[test]
    fn test_filter_duplicates_with_missing_ids() {
        let mut engine = DeduplicationEngine::new();
        
        let messages = vec![
            create_test_message(Some("uuid-1".to_string()), Some("req-1".to_string())),
            create_test_message(None, Some("req-2".to_string())), // Missing UUID
            create_test_message(Some("uuid-3".to_string()), None), // Missing request ID
            create_test_message(None, None), // Missing both
        ];
        
        let unique = engine.filter_duplicates(messages).unwrap();
        
        // All messages should be included (none are duplicates)
        assert_eq!(unique.len(), 4);
        // But only the first one can be tracked for deduplication
        assert_eq!(engine.processed_count(), 1);
    }

    #[test]
    fn test_branched_conversation_scenario() {
        let mut engine = DeduplicationEngine::new();
        
        // Simulate a branched conversation where the same message appears in multiple branches
        let branch1 = vec![
            create_test_message(Some("uuid-1".to_string()), Some("req-1".to_string())),
            create_test_message(Some("uuid-2".to_string()), Some("req-2".to_string())),
            create_test_message(Some("uuid-3".to_string()), Some("req-3".to_string())),
        ];
        
        let branch2 = vec![
            create_test_message(Some("uuid-1".to_string()), Some("req-1".to_string())), // Duplicate
            create_test_message(Some("uuid-2".to_string()), Some("req-2".to_string())), // Duplicate
            create_test_message(Some("uuid-4".to_string()), Some("req-4".to_string())), // New
        ];
        
        // Process branch 1
        let unique1 = engine.filter_duplicates(branch1).unwrap();
        assert_eq!(unique1.len(), 3);
        
        // Process branch 2
        let unique2 = engine.filter_duplicates(branch2).unwrap();
        assert_eq!(unique2.len(), 1); // Only uuid-4 is new
        
        assert_eq!(engine.processed_count(), 4); // Total unique messages
    }

    #[test]
    fn test_with_database_persistence() {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path();
        
        // Create engine and process some messages
        {
            let mut engine = DeduplicationEngine::with_database(db_path).unwrap();
            
            let message1 = create_test_message(Some("uuid-1".to_string()), Some("req-1".to_string()));
            let message2 = create_test_message(Some("uuid-2".to_string()), Some("req-2".to_string()));
            
            assert!(engine.mark_as_processed(&message1).unwrap());
            assert!(engine.mark_as_processed(&message2).unwrap());
            assert_eq!(engine.processed_count(), 2);
        }
        
        // Create new engine with same database - should load existing hashes
        {
            let engine = DeduplicationEngine::with_database(db_path).unwrap();
            assert_eq!(engine.processed_count(), 2); // Should have loaded from DB
            
            let message1 = create_test_message(Some("uuid-1".to_string()), Some("req-1".to_string()));
            assert!(engine.is_duplicate(&message1)); // Should recognize as duplicate
        }
    }

    #[test]
    fn test_clear_history() {
        let mut engine = DeduplicationEngine::new();
        
        let message = create_test_message(Some("uuid-1".to_string()), Some("req-1".to_string()));
        engine.mark_as_processed(&message).unwrap();
        
        assert_eq!(engine.processed_count(), 1);
        
        engine.clear_history().unwrap();
        
        assert_eq!(engine.processed_count(), 0);
        assert!(!engine.is_duplicate(&message)); // No longer a duplicate
    }

    #[test]
    fn test_performance_large_dataset() {
        let mut engine = DeduplicationEngine::new();
        let mut messages = Vec::new();
        
        // Create 10,000 messages with some duplicates
        for i in 0..10000 {
            // First 8000 are unique, last 2000 are duplicates of the first 2000
            let idx = if i < 8000 { i } else { i - 8000 };
            let uuid = format!("uuid-{}", idx);
            let req_id = format!("req-{}", idx);
            messages.push(create_test_message(Some(uuid), Some(req_id)));
        }
        
        let start = std::time::Instant::now();
        let unique = engine.filter_duplicates(messages).unwrap();
        let duration = start.elapsed();
        
        assert_eq!(unique.len(), 8000); // Should have 8000 unique messages
        assert!(duration.as_secs() < 1, "Performance test took too long: {:?}", duration);
        
        // Verify O(1) lookup performance
        let test_message = create_test_message(
            Some("uuid-5000".to_string()),
            Some("req-5000".to_string())
        );
        
        let lookup_start = std::time::Instant::now();
        let _ = engine.is_duplicate(&test_message);
        let lookup_duration = lookup_start.elapsed();
        
        assert!(lookup_duration.as_micros() < 100, "Lookup should be very fast (O(1))");
    }
}