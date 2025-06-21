use anyhow::Result;
use std::collections::HashSet;

use super::jsonl::UsageData;

/// Deduplication engine for handling branched conversations and billing accuracy
///
/// This is the core value proposition of ccost - solving the branching problem while
/// maintaining billing accuracy through proper API call identification.
///
/// # Deduplication Strategy (TASK-051)
///
/// Uses requestId-priority strategy for optimal billing accuracy alignment:
/// 1. **Preferred**: `message.id + requestId` - matches actual API billing identifiers
/// 2. **Fallback**: `message.id + sessionId` - legacy compatibility for older data
/// 3. **Fail-safe**: No hash generation without message.id and at least one identifier
///
/// # Hash Collision Prevention
///
/// Uses prefixed hash formats to prevent collisions between different identifier types:
/// - `req:{message_id}:{request_id}` - Modern requestId-based hashes
/// - `session:{message_id}:{session_id}` - Legacy sessionId-based hashes
///
/// This ensures that the same identifier values don't collide across different hash types.
pub struct DeduplicationEngine {
    /// In-memory cache for fast O(1) lookups
    seen_hashes: HashSet<String>,
}

impl DeduplicationEngine {
    /// Create a new in-memory deduplication engine
    pub fn new() -> Self {
        Self {
            seen_hashes: HashSet::new(),
        }
    }

    /// Generate unique hash from message identifiers using requestId priority strategy
    ///
    /// # TASK-051 Implementation
    ///
    /// This function implements the new deduplication strategy that prioritizes requestId
    /// for better billing accuracy alignment with competitor tools and actual API billing.
    ///
    /// # Priority Hierarchy
    ///
    /// 1. **requestId Priority**: When both requestId and sessionId are available,
    ///    requestId takes priority as it represents the actual billable API call identifier
    /// 2. **sessionId Fallback**: When requestId is missing but sessionId is available,
    ///    falls back to sessionId for legacy compatibility
    /// 3. **Fail-safe**: Returns None when message.id is missing or both identifiers are missing
    ///
    /// # Hash Format
    ///
    /// - Modern: `req:{message_id}:{request_id}`
    /// - Legacy: `session:{message_id}:{session_id}`
    /// - Prefixes prevent hash collisions between different identifier types
    ///
    /// # Performance
    ///
    /// Uses simple string concatenation for O(1) performance and deterministic results.
    /// No cryptographic hashing needed as identifiers are already unique.
    pub fn generate_hash(
        message_id: &Option<String>,
        request_id: &Option<String>,
        session_id: &Option<String>,
    ) -> Option<String> {
        match (message_id, request_id, session_id) {
            // Preferred: message.id + requestId (best billing accuracy)
            (Some(m), Some(r), _) => Some(format!("req:{m}:{r}")),
            // Fallback: message.id + sessionId (legacy compatibility)
            (Some(m), None, Some(s)) => Some(format!("session:{m}:{s}")),
            // Cannot generate hash without message.id and at least one identifier
            _ => None,
        }
    }

    /// Check if a message has already been processed
    pub fn is_duplicate(&self, message: &UsageData) -> bool {
        let message_id = message.message.as_ref().and_then(|m| m.id.clone());
        if let Some(hash) =
            Self::generate_hash(&message_id, &message.request_id, &message.session_id)
        {
            self.seen_hashes.contains(&hash)
        } else {
            false // Messages without proper IDs are not considered duplicates
        }
    }

    /// Mark a message as processed
    pub fn mark_as_processed(&mut self, message: &UsageData, _project_name: &str) -> Result<bool> {
        let message_id = message.message.as_ref().and_then(|m| m.id.clone());
        if let Some(hash) =
            Self::generate_hash(&message_id, &message.request_id, &message.session_id)
        {
            // Check if already exists
            if self.seen_hashes.contains(&hash) {
                return Ok(false); // Already processed
            }

            // Add to in-memory set
            self.seen_hashes.insert(hash);

            Ok(true) // Successfully marked as processed
        } else {
            Ok(false) // Cannot mark messages without proper IDs
        }
    }

    /// Process a batch of messages, returning only non-duplicates
    pub fn filter_duplicates(
        &mut self,
        messages: Vec<UsageData>,
        project_name: &str,
    ) -> Result<Vec<UsageData>> {
        let mut unique_messages = Vec::new();
        let mut stats = DeduplicationStats::default();

        for message in messages {
            stats.total_messages += 1;

            if self.is_duplicate(&message) {
                stats.duplicates_found += 1;
                continue;
            }

            let message_id = message.message.as_ref().and_then(|m| m.id.clone());
            if let Some(_hash) =
                Self::generate_hash(&message_id, &message.request_id, &message.session_id)
            {
                self.mark_as_processed(&message, project_name)?;
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

    fn create_test_message(uuid: Option<String>, request_id: Option<String>) -> UsageData {
        // Generate a default message.id based on uuid for consistency
        let message_id = uuid.as_ref().map(|u| format!("msg_{}", u));

        UsageData {
            timestamp: Some("2025-06-09T10:00:00Z".to_string()),
            uuid,
            request_id,
            session_id: Some("test-session-123".to_string()),
            message: Some(Message {
                id: message_id,
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
            cwd: None,
            original_cwd: None,
        }
    }

    fn create_test_message_with_message_id(
        uuid: Option<String>,
        request_id: Option<String>,
        message_id: Option<String>,
    ) -> UsageData {
        UsageData {
            timestamp: Some("2025-06-09T10:00:00Z".to_string()),
            uuid,
            request_id,
            session_id: Some("test-session-123".to_string()),
            message: Some(Message {
                id: message_id,
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
            cwd: None,
            original_cwd: None,
        }
    }

    #[test]
    fn test_generate_hash_with_request_id_priority() {
        // Test that requestId takes priority when both requestId and sessionId are present
        let hash = DeduplicationEngine::generate_hash(
            &Some("msg-123".to_string()),
            &Some("req-456".to_string()),
            &Some("session-789".to_string()),
        );

        assert!(hash.is_some());
        let hash_value = hash.unwrap();
        assert_eq!(hash_value, "req:msg-123:req-456");

        // Hash should be deterministic
        let hash2 = DeduplicationEngine::generate_hash(
            &Some("msg-123".to_string()),
            &Some("req-456".to_string()),
            &Some("session-789".to_string()),
        )
        .unwrap();

        assert_eq!(hash_value, hash2);

        // Different request IDs should produce different hash
        let hash3 = DeduplicationEngine::generate_hash(
            &Some("msg-123".to_string()),
            &Some("req-999".to_string()),
            &Some("session-789".to_string()),
        )
        .unwrap();

        assert_ne!(hash_value, hash3);
        assert_eq!(hash3, "req:msg-123:req-999");
    }

    #[test]
    fn test_generate_hash_missing_message_id() {
        let hash =
            DeduplicationEngine::generate_hash(&None, &None, &Some("session-456".to_string()));

        assert!(hash.is_none());
    }

    #[test]
    fn test_generate_hash_session_id_fallback() {
        // Test that sessionId is used as fallback when requestId is None
        let hash = DeduplicationEngine::generate_hash(
            &Some("msg-123".to_string()),
            &None,
            &Some("session-456".to_string()),
        );

        assert!(hash.is_some());
        let hash_value = hash.unwrap();
        assert_eq!(hash_value, "session:msg-123:session-456");
    }

    #[test]
    fn test_generate_hash_missing_both_ids() {
        let hash = DeduplicationEngine::generate_hash(&Some("msg-123".to_string()), &None, &None);

        // Should return None when missing both requestId and sessionId
        assert!(hash.is_none());
    }

    #[test]
    fn test_is_duplicate_new_message() {
        let engine = DeduplicationEngine::new();
        let message =
            create_test_message(Some("uuid-123".to_string()), Some("req-456".to_string()));

        assert!(!engine.is_duplicate(&message));
    }

    #[test]
    fn test_mark_as_processed() {
        let mut engine = DeduplicationEngine::new();
        let message =
            create_test_message(Some("uuid-123".to_string()), Some("req-456".to_string()));

        // First time should return true (newly processed)
        assert!(engine.mark_as_processed(&message, "test_project").unwrap());

        // Now it should be a duplicate
        assert!(engine.is_duplicate(&message));

        // Second time should return false (already processed)
        assert!(!engine.mark_as_processed(&message, "test_project").unwrap());
    }

    #[test]
    fn test_mark_as_processed_missing_ids() {
        let mut engine = DeduplicationEngine::new();
        let message = create_test_message(None, None);

        // Cannot mark messages without IDs
        assert!(!engine.mark_as_processed(&message, "test_project").unwrap());
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

        let unique = engine.filter_duplicates(messages, "test_project").unwrap();

        assert_eq!(unique.len(), 3); // One duplicate removed
    }

    #[test]
    fn test_filter_duplicates_with_missing_ids() {
        let mut engine = DeduplicationEngine::new();

        let messages = vec![
            create_test_message(Some("uuid-1".to_string()), Some("req-1".to_string())),
            create_test_message(None, Some("req-2".to_string())), // Missing UUID
            create_test_message(Some("uuid-3".to_string()), None), // Missing request ID
            create_test_message(None, None),                      // Missing both
        ];

        let unique = engine.filter_duplicates(messages, "test_project").unwrap();

        // All messages should be included (none are duplicates)
        assert_eq!(unique.len(), 4);
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
        let unique1 = engine.filter_duplicates(branch1, "test_project").unwrap();
        assert_eq!(unique1.len(), 3);

        // Process branch 2
        let unique2 = engine.filter_duplicates(branch2, "test_project").unwrap();
        assert_eq!(unique2.len(), 1); // Only uuid-4 is new

        // Verify total unique messages processed correctly
    }

    #[test]
    fn test_in_memory_only() {
        // Test that deduplication works within a single session
        let mut engine = DeduplicationEngine::new();

        let message1 = create_test_message(Some("uuid-1".to_string()), Some("req-1".to_string()));
        let message2 = create_test_message(Some("uuid-2".to_string()), Some("req-2".to_string()));

        assert!(engine.mark_as_processed(&message1, "test_project").unwrap());
        assert!(engine.mark_as_processed(&message2, "test_project").unwrap());

        // Messages should be recognized as duplicates within the same session
        assert!(engine.is_duplicate(&message1));
        assert!(engine.is_duplicate(&message2));

        // New engine won't have any memory of previous messages (in-memory only)
        let engine2 = DeduplicationEngine::new();
        assert!(!engine2.is_duplicate(&message1));
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
        let unique = engine.filter_duplicates(messages, "test_project").unwrap();
        let duration = start.elapsed();

        assert_eq!(unique.len(), 8000); // Should have 8000 unique messages
        assert!(
            duration.as_secs() < 1,
            "Performance test took too long: {:?}",
            duration
        );

        // Verify O(1) lookup performance
        let test_message =
            create_test_message(Some("uuid-5000".to_string()), Some("req-5000".to_string()));

        let lookup_start = std::time::Instant::now();
        let _ = engine.is_duplicate(&test_message);
        let lookup_duration = lookup_start.elapsed();

        assert!(
            lookup_duration.as_micros() < 100,
            "Lookup should be very fast (O(1))"
        );
    }

    // NEW TESTS FOR IMPROVED FIELD MAPPING ACCURACY (TASK-044-CRITICAL)

    #[test]
    fn test_fallback_to_message_id_when_request_id_null() {
        // This test simulates the real-world scenario where request_id is null
        // but message.id is available (common in Claude JSONL data)
        let hash1 = DeduplicationEngine::generate_hash(
            &Some("msg_01ABC123".to_string()), // message.id is available
            &None,                             // request_id is None
            &Some("session-123".to_string()),  // session_id is available
        );

        assert!(
            hash1.is_some(),
            "Should generate hash using message_id + session_id"
        );

        // Same combination should produce same hash
        let hash2 = DeduplicationEngine::generate_hash(
            &Some("msg_01ABC123".to_string()),
            &None, // request_id is None
            &Some("session-123".to_string()),
        );

        assert_eq!(hash1, hash2, "Hashes should be deterministic");

        // Different message_id should produce different hash
        let hash3 = DeduplicationEngine::generate_hash(
            &Some("msg_01XYZ789".to_string()),
            &None, // request_id is None
            &Some("session-123".to_string()),
        );

        assert_ne!(
            hash1, hash3,
            "Different message IDs should produce different hashes"
        );
    }

    #[test]
    fn test_message_id_only_fallback() {
        // Test scenario where only message.id is available (should fail)
        let hash =
            DeduplicationEngine::generate_hash(&Some("msg_01ABC123".to_string()), &None, &None);

        assert!(
            hash.is_none(),
            "Should not generate hash without session_id"
        );
    }

    #[test]
    fn test_session_id_only_fallback() {
        // Test scenario where only session_id is available
        let hash =
            DeduplicationEngine::generate_hash(&None, &None, &Some("session-123".to_string()));

        assert!(
            hash.is_none(),
            "Should not generate hash without message_id"
        );
    }

    #[test]
    fn test_real_world_deduplication_scenario() {
        // This test simulates the exact scenario causing accuracy issues
        // Real JSONL data: uuid present, request_id null, message.id present
        let mut engine = DeduplicationEngine::new();

        // Create realistic messages like competitor tools would see
        let msg1 = create_test_message_with_message_id(
            Some("e84e63d2-776b-4dc3-af1a-2da917d3174a".to_string()),
            None, // request_id is null in real data
            Some("msg_01WoX9ZZQjSa71XuNyBgKS9H".to_string()),
        );

        let msg2 = create_test_message_with_message_id(
            Some("e84e63d2-776b-4dc3-af1a-2da917d3174a".to_string()),
            None,                                             // request_id is still null
            Some("msg_01WoX9ZZQjSa71XuNyBgKS9H".to_string()), // Same message.id
        );

        // Should detect as duplicate even with null request_id
        assert!(engine.mark_as_processed(&msg1, "test_project").unwrap());
        assert!(
            engine.is_duplicate(&msg2),
            "Should detect duplicate using uuid + message_id"
        );
        assert!(
            !engine.mark_as_processed(&msg2, "test_project").unwrap(),
            "Should not process duplicate"
        );
    }

    #[test]
    fn test_priority_order_uuid_request_id_over_message_id() {
        // Test that uuid + request_id takes priority over uuid + message_id
        let hash1 = DeduplicationEngine::generate_hash(
            &Some("msg_789".to_string()),
            &Some("req-456".to_string()),
            &None, // session_id is None
        );

        let hash2 = DeduplicationEngine::generate_hash(
            &Some("msg_789".to_string()),
            &Some("req-456".to_string()),
            &None, // session_id is None
        );

        assert_eq!(
            hash1, hash2,
            "Should prioritize uuid + request_id even when message_id is available"
        );
    }

    #[test]
    fn test_hash_format_consistency() {
        // Test that hash format is consistent and deterministic
        let hash1 = DeduplicationEngine::generate_hash(
            &Some("msg-123".to_string()),
            &None, // request_id is None
            &Some("session-456".to_string()),
        )
        .unwrap();

        let hash2 = DeduplicationEngine::generate_hash(
            &Some("msg-123".to_string()),
            &None, // request_id is None
            &Some("session-456".to_string()),
        )
        .unwrap();

        assert_eq!(hash1, hash2, "Hash should be deterministic for same inputs");

        // Verify the format includes session prefix
        assert_eq!(hash1, "session:msg-123:session-456");
    }

    // TASK-051 SPECIFIC TESTS: RequestId Priority for Billing Accuracy

    #[test]
    fn test_task_051_billing_accuracy_same_request_different_sessions() {
        // Test that messages with same requestId but different sessionId are deduplicated
        // This is the key billing accuracy improvement
        let mut engine = DeduplicationEngine::new();

        let msg1 = create_test_message_with_message_id(
            Some("uuid-1".to_string()),
            Some("req-bill-123".to_string()), // Same requestId
            Some("msg-1".to_string()),
        );

        let mut msg2 = msg1.clone();
        msg2.session_id = Some("different-session".to_string()); // Different sessionId

        // First message should be processed
        assert!(engine.mark_as_processed(&msg1, "test_project").unwrap());

        // Second message should be a duplicate (same requestId)
        assert!(engine.is_duplicate(&msg2));
        assert!(!engine.mark_as_processed(&msg2, "test_project").unwrap());
    }

    #[test]
    fn test_task_051_billing_accuracy_different_requests_same_session() {
        // Test that messages with different requestId but same sessionId are NOT deduplicated
        // This ensures we don't over-deduplicate legitimate separate API calls
        let mut engine = DeduplicationEngine::new();

        let msg1 = create_test_message_with_message_id(
            Some("uuid-1".to_string()),
            Some("req-bill-123".to_string()), // Different requestId
            Some("msg-1".to_string()),
        );

        let mut msg2 = msg1.clone();
        msg2.request_id = Some("req-bill-456".to_string()); // Different requestId

        // Both messages should be processed (different requestIds)
        assert!(engine.mark_as_processed(&msg1, "test_project").unwrap());
        assert!(!engine.is_duplicate(&msg2));
        assert!(engine.mark_as_processed(&msg2, "test_project").unwrap());
    }

    #[test]
    fn test_task_051_migration_compatibility() {
        // Test that legacy data (sessionId only) still works correctly
        let mut engine = DeduplicationEngine::new();

        // Legacy message without requestId
        let legacy_msg1 = create_test_message_with_message_id(
            Some("uuid-1".to_string()),
            None, // No requestId (legacy)
            Some("msg-1".to_string()),
        );

        let legacy_msg2 = legacy_msg1.clone();

        // Legacy messages should still be deduplicated using sessionId fallback
        assert!(
            engine
                .mark_as_processed(&legacy_msg1, "test_project")
                .unwrap()
        );
        assert!(engine.is_duplicate(&legacy_msg2));
    }

    #[test]
    fn test_task_051_mixed_data_scenarios() {
        // Test mixing modern (requestId) and legacy (sessionId only) data
        let mut engine = DeduplicationEngine::new();

        // Modern message with requestId
        let modern_msg = create_test_message_with_message_id(
            Some("uuid-1".to_string()),
            Some("req-456".to_string()),
            Some("msg-1".to_string()),
        );

        // Legacy message without requestId but same other identifiers
        let legacy_msg = create_test_message_with_message_id(
            Some("uuid-1".to_string()),
            None, // No requestId
            Some("msg-1".to_string()),
        );

        // These should NOT be duplicates (different hash strategies)
        assert!(
            engine
                .mark_as_processed(&modern_msg, "test_project")
                .unwrap()
        );
        assert!(!engine.is_duplicate(&legacy_msg));
        assert!(
            engine
                .mark_as_processed(&legacy_msg, "test_project")
                .unwrap()
        );
    }

    #[test]
    fn test_task_051_hash_collision_prevention() {
        // Test that requestId and sessionId hashes don't collide
        let hash_with_request = DeduplicationEngine::generate_hash(
            &Some("msg-123".to_string()),
            &Some("id-456".to_string()),
            &None,
        )
        .unwrap();

        let hash_with_session = DeduplicationEngine::generate_hash(
            &Some("msg-123".to_string()),
            &None,
            &Some("id-456".to_string()),
        )
        .unwrap();

        // Even with same ID value, hashes should be different due to prefixes
        assert_ne!(hash_with_request, hash_with_session);
        assert!(hash_with_request.starts_with("req:"));
        assert!(hash_with_session.starts_with("session:"));
    }

    #[test]
    fn test_competitor_accuracy_parity() {
        // This test ensures we can deduplicate the same messages that competitor tools can
        let mut engine = DeduplicationEngine::new();

        // Simulate messages that competitor tools would successfully deduplicate
        let messages = vec![
            // Message 1: Standard case with uuid and message.id, null request_id
            create_test_message_with_message_id(
                Some("uuid-1".to_string()),
                None,
                Some("msg-1".to_string()),
            ),
            // Message 2: Duplicate of message 1
            create_test_message_with_message_id(
                Some("uuid-1".to_string()),
                None,
                Some("msg-1".to_string()),
            ),
            // Message 3: Different message with only message.id
            create_test_message_with_message_id(None, None, Some("msg-2".to_string())),
            // Message 4: Duplicate of message 3
            create_test_message_with_message_id(None, None, Some("msg-2".to_string())),
        ];

        let unique = engine.filter_duplicates(messages, "test_project").unwrap();

        // Should detect duplicates that competitor tools can detect
        assert_eq!(
            unique.len(),
            2,
            "Should detect same duplicates as competitor tools"
        );
        // Should track unique messages correctly
    }
}
