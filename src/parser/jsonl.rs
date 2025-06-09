use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Message {
    pub content: Option<String>,
    pub model: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Usage {
    #[serde(rename = "inputTokens")]
    pub input_tokens: Option<u64>,
    #[serde(rename = "outputTokens")]
    pub output_tokens: Option<u64>,
    #[serde(rename = "cacheCreationInputTokens")]
    pub cache_creation_input_tokens: Option<u64>,
    #[serde(rename = "cacheReadInputTokens")]
    pub cache_read_input_tokens: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct UsageData {
    pub timestamp: String,
    pub uuid: Option<String>,
    #[serde(rename = "requestId")]
    pub request_id: Option<String>,
    pub message: Option<Message>,
    pub usage: Option<Usage>,
    #[serde(rename = "costUSD")]
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ParsedConversation {
    pub project_path: PathBuf,
    pub file_path: PathBuf,
    pub messages: Vec<UsageData>,
    pub total_lines: usize,
    pub parsed_lines: usize,
    pub skipped_lines: usize,
}

pub struct JsonlParser {
    base_dir: PathBuf,
}

impl JsonlParser {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Extract project name from directory structure
    /// Expects paths like ~/.claude/projects/PROJECT_NAME/conversation_id.jsonl
    pub fn extract_project_path(&self, file_path: &Path) -> Result<PathBuf> {
        let relative_path = file_path
            .strip_prefix(&self.base_dir)
            .map_err(|_| anyhow!("File path is not within base directory"))?;

        // Get the first component after base_dir, which should be the project name
        if let Some(project_name) = relative_path.components().next() {
            Ok(PathBuf::from(project_name.as_os_str()))
        } else {
            Err(anyhow!("Could not extract project name from path: {}", file_path.display()))
        }
    }

    /// Parse a single JSONL file
    pub fn parse_file(&self, file_path: &Path) -> Result<ParsedConversation> {
        let project_path = self.extract_project_path(file_path)?;
        
        let file = File::open(file_path)
            .map_err(|e| anyhow!("Failed to open file {}: {}", file_path.display(), e))?;
        
        let reader = BufReader::new(file);
        let mut messages = Vec::new();
        let mut total_lines = 0;
        let mut parsed_lines = 0;
        let mut skipped_lines = 0;

        for (line_num, line_result) in reader.lines().enumerate() {
            total_lines += 1;
            
            match line_result {
                Ok(line) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    
                    match self.parse_line(&line, line_num + 1, file_path) {
                        Ok(Some(usage_data)) => {
                            messages.push(usage_data);
                            parsed_lines += 1;
                        }
                        Ok(None) => {
                            // Line was intentionally skipped (e.g., missing required fields)
                            skipped_lines += 1;
                        }
                        Err(e) => {
                            eprintln!("Warning: Skipping malformed JSON at {}:{}: {}", 
                                     file_path.display(), line_num + 1, e);
                            skipped_lines += 1;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to read line {} in {}: {}", 
                             line_num + 1, file_path.display(), e);
                    skipped_lines += 1;
                }
            }
        }

        Ok(ParsedConversation {
            project_path,
            file_path: file_path.to_path_buf(),
            messages,
            total_lines,
            parsed_lines,
            skipped_lines,
        })
    }

    /// Parse a single line of JSONL
    fn parse_line(&self, line: &str, line_num: usize, file_path: &Path) -> Result<Option<UsageData>> {
        let usage_data: UsageData = serde_json::from_str(line)
            .map_err(|e| anyhow!("JSON parse error: {}", e))?;

        // Validate required fields for meaningful data
        if usage_data.timestamp.is_empty() {
            return Ok(None); // Skip entries without timestamps
        }

        // For deduplication purposes, we prefer both uuid and request_id
        // but we'll warn if they're missing rather than skip entirely
        if usage_data.uuid.is_none() && usage_data.request_id.is_none() {
            eprintln!("Warning: Message at {}:{} has no uuid or requestId - may not deduplicate properly", 
                     file_path.display(), line_num);
        }

        Ok(Some(usage_data))
    }

    /// Find all JSONL files in the projects directory
    pub fn find_jsonl_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.find_jsonl_files_recursive(&self.base_dir, &mut files)?;
        Ok(files)
    }

    fn find_jsonl_files_recursive(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if !dir.exists() {
            return Err(anyhow!("Directory does not exist: {}", dir.display()));
        }

        let entries = std::fs::read_dir(dir)
            .map_err(|e| anyhow!("Failed to read directory {}: {}", dir.display(), e))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Recursively search subdirectories
                self.find_jsonl_files_recursive(&path, files)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                files.push(path);
            }
        }

        Ok(())
    }

    /// Parse all JSONL files in the base directory
    pub fn parse_all_files(&self) -> Result<Vec<ParsedConversation>> {
        let files = self.find_jsonl_files()?;
        let mut conversations = Vec::new();

        for file_path in files {
            match self.parse_file(&file_path) {
                Ok(conversation) => {
                    conversations.push(conversation);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to parse file {}: {}", file_path.display(), e);
                }
            }
        }

        Ok(conversations)
    }
}

impl Default for JsonlParser {
    fn default() -> Self {
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let base_dir = home_dir.join(".claude").join("projects");
        Self::new(base_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_jsonl_content() -> Vec<&'static str> {
        vec![
            // Valid complete entry
            r#"{"timestamp":"2025-06-09T10:30:00Z","uuid":"test-uuid-1","requestId":"req-1","message":{"content":"Hello","model":"claude-sonnet-4","role":"user"},"usage":{"inputTokens":10,"outputTokens":20},"costUSD":0.001}"#,
            
            // Valid minimal entry (missing optional fields)
            r#"{"timestamp":"2025-06-09T10:31:00Z","uuid":"test-uuid-2"}"#,
            
            // Entry with missing uuid but has requestId
            r#"{"timestamp":"2025-06-09T10:32:00Z","requestId":"req-3","usage":{"inputTokens":15,"outputTokens":25}}"#,
            
            // Entry with cache tokens
            r#"{"timestamp":"2025-06-09T10:33:00Z","uuid":"test-uuid-4","requestId":"req-4","usage":{"inputTokens":10,"outputTokens":20,"cacheCreationInputTokens":5,"cacheReadInputTokens":3}}"#,
            
            // Empty line (should be skipped)
            "",
            
            // Malformed JSON (should be skipped with warning)
            r#"{"timestamp":"2025-06-09T10:34:00Z","invalid":json"#,
            
            // Entry with empty timestamp (should be skipped)
            r#"{"timestamp":"","uuid":"test-uuid-empty"}"#,
            
            // Entry with no timestamp (should be skipped)
            r#"{"uuid":"test-uuid-no-timestamp","message":{"content":"Test"}}"#,
        ]
    }

    fn setup_test_environment() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let base_dir = temp_dir.path().to_path_buf();
        
        // Create project structure
        let project1_dir = base_dir.join("project1");
        let project2_dir = base_dir.join("project2");
        fs::create_dir_all(&project1_dir).expect("Failed to create project1 dir");
        fs::create_dir_all(&project2_dir).expect("Failed to create project2 dir");
        
        (temp_dir, base_dir)
    }

    #[test]
    fn test_parse_line_valid_complete() {
        let parser = JsonlParser::new(PathBuf::from("/test"));
        let test_path = Path::new("/test/file.jsonl");
        
        let line = r#"{"timestamp":"2025-06-09T10:30:00Z","uuid":"test-uuid","requestId":"req-1","message":{"content":"Hello","model":"claude-sonnet-4","role":"user"},"usage":{"inputTokens":10,"outputTokens":20},"costUSD":0.001}"#;
        
        let result = parser.parse_line(line, 1, test_path).unwrap();
        assert!(result.is_some());
        
        let usage_data = result.unwrap();
        assert_eq!(usage_data.timestamp, "2025-06-09T10:30:00Z");
        assert_eq!(usage_data.uuid, Some("test-uuid".to_string()));
        assert_eq!(usage_data.request_id, Some("req-1".to_string()));
        assert_eq!(usage_data.cost_usd, Some(0.001));
        
        let message = usage_data.message.unwrap();
        assert_eq!(message.content, Some("Hello".to_string()));
        assert_eq!(message.model, Some("claude-sonnet-4".to_string()));
        assert_eq!(message.role, Some("user".to_string()));
        
        let usage = usage_data.usage.unwrap();
        assert_eq!(usage.input_tokens, Some(10));
        assert_eq!(usage.output_tokens, Some(20));
    }

    #[test]
    fn test_parse_line_minimal_valid() {
        let parser = JsonlParser::new(PathBuf::from("/test"));
        let test_path = Path::new("/test/file.jsonl");
        
        let line = r#"{"timestamp":"2025-06-09T10:30:00Z","uuid":"test-uuid"}"#;
        
        let result = parser.parse_line(line, 1, test_path).unwrap();
        assert!(result.is_some());
        
        let usage_data = result.unwrap();
        assert_eq!(usage_data.timestamp, "2025-06-09T10:30:00Z");
        assert_eq!(usage_data.uuid, Some("test-uuid".to_string()));
        assert_eq!(usage_data.request_id, None);
        assert_eq!(usage_data.message, None);
        assert_eq!(usage_data.usage, None);
        assert_eq!(usage_data.cost_usd, None);
    }

    #[test]
    fn test_parse_line_empty_timestamp() {
        let parser = JsonlParser::new(PathBuf::from("/test"));
        let test_path = Path::new("/test/file.jsonl");
        
        let line = r#"{"timestamp":"","uuid":"test-uuid"}"#;
        
        let result = parser.parse_line(line, 1, test_path).unwrap();
        assert!(result.is_none()); // Should be skipped
    }

    #[test]
    fn test_parse_line_missing_timestamp() {
        let parser = JsonlParser::new(PathBuf::from("/test"));
        let test_path = Path::new("/test/file.jsonl");
        
        let line = r#"{"uuid":"test-uuid"}"#;
        
        // This should fail because timestamp field is missing entirely
        let result = parser.parse_line(line, 1, test_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_line_malformed_json() {
        let parser = JsonlParser::new(PathBuf::from("/test"));
        let test_path = Path::new("/test/file.jsonl");
        
        let line = r#"{"timestamp":"2025-06-09T10:30:00Z","invalid":json"#;
        
        let result = parser.parse_line(line, 1, test_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_project_path() {
        let base_dir = PathBuf::from("/home/user/.claude/projects");
        let parser = JsonlParser::new(base_dir.clone());
        
        let file_path = base_dir.join("my-project").join("conversation-123.jsonl");
        let project_path = parser.extract_project_path(&file_path).unwrap();
        
        assert_eq!(project_path, PathBuf::from("my-project"));
    }

    #[test]
    fn test_extract_project_path_nested() {
        let base_dir = PathBuf::from("/home/user/.claude/projects");
        let parser = JsonlParser::new(base_dir.clone());
        
        let file_path = base_dir.join("my-project").join("subfolder").join("conversation-123.jsonl");
        let project_path = parser.extract_project_path(&file_path).unwrap();
        
        assert_eq!(project_path, PathBuf::from("my-project"));
    }

    #[test]
    fn test_extract_project_path_outside_base() {
        let base_dir = PathBuf::from("/home/user/.claude/projects");
        let parser = JsonlParser::new(base_dir);
        
        let file_path = PathBuf::from("/other/path/conversation-123.jsonl");
        let result = parser.extract_project_path(&file_path);
        
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_file_success() {
        let (_temp_dir, base_dir) = setup_test_environment();
        let parser = JsonlParser::new(base_dir.clone());
        
        let file_path = base_dir.join("project1").join("conversation.jsonl");
        let content = create_test_jsonl_content().join("\n");
        fs::write(&file_path, content).expect("Failed to write test file");
        
        let result = parser.parse_file(&file_path).unwrap();
        
        assert_eq!(result.project_path, PathBuf::from("project1"));
        assert_eq!(result.file_path, file_path);
        assert_eq!(result.parsed_lines, 4); // 4 valid entries  
        assert_eq!(result.skipped_lines, 3); // 1 empty + 1 malformed + 1 empty timestamp (missing timestamp fails JSON parse)
        assert_eq!(result.total_lines, 8);
        assert_eq!(result.messages.len(), 4);
        
        // Verify first message
        let first_msg = &result.messages[0];
        assert_eq!(first_msg.uuid, Some("test-uuid-1".to_string()));
        assert_eq!(first_msg.request_id, Some("req-1".to_string()));
    }

    #[test]
    fn test_find_jsonl_files() {
        let (_temp_dir, base_dir) = setup_test_environment();
        let parser = JsonlParser::new(base_dir.clone());
        
        // Create test files
        let file1 = base_dir.join("project1").join("conv1.jsonl");
        let file2 = base_dir.join("project1").join("conv2.jsonl");
        let file3 = base_dir.join("project2").join("conv3.jsonl");
        let non_jsonl = base_dir.join("project1").join("readme.txt");
        
        fs::write(&file1, "test").expect("Failed to write file1");
        fs::write(&file2, "test").expect("Failed to write file2");
        fs::write(&file3, "test").expect("Failed to write file3");
        fs::write(&non_jsonl, "test").expect("Failed to write non-jsonl");
        
        let mut files = parser.find_jsonl_files().unwrap();
        files.sort(); // Sort for consistent comparison
        
        let mut expected = vec![file1, file2, file3];
        expected.sort();
        
        assert_eq!(files, expected);
        assert!(!files.contains(&non_jsonl));
    }

    #[test]
    fn test_find_jsonl_files_empty_directory() {
        let (_temp_dir, base_dir) = setup_test_environment();
        let parser = JsonlParser::new(base_dir);
        
        let files = parser.find_jsonl_files().unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_find_jsonl_files_nonexistent_directory() {
        let parser = JsonlParser::new(PathBuf::from("/nonexistent/path"));
        
        let result = parser.find_jsonl_files();
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_all_files() {
        let (_temp_dir, base_dir) = setup_test_environment();
        let parser = JsonlParser::new(base_dir.clone());
        
        // Create multiple test files
        let file1 = base_dir.join("project1").join("conv1.jsonl");
        let file2 = base_dir.join("project2").join("conv2.jsonl");
        
        let content1 = r#"{"timestamp":"2025-06-09T10:30:00Z","uuid":"uuid1"}
{"timestamp":"2025-06-09T10:31:00Z","uuid":"uuid2"}"#;
        
        let content2 = r#"{"timestamp":"2025-06-09T10:32:00Z","uuid":"uuid3"}"#;
        
        fs::write(&file1, content1).expect("Failed to write file1");
        fs::write(&file2, content2).expect("Failed to write file2");
        
        let conversations = parser.parse_all_files().unwrap();
        
        assert_eq!(conversations.len(), 2);
        
        // Find conversations by project
        let proj1_conv = conversations.iter()
            .find(|c| c.project_path == PathBuf::from("project1"))
            .unwrap();
        let proj2_conv = conversations.iter()
            .find(|c| c.project_path == PathBuf::from("project2"))
            .unwrap();
        
        assert_eq!(proj1_conv.messages.len(), 2);
        assert_eq!(proj2_conv.messages.len(), 1);
    }

    #[test]
    fn test_cache_tokens_parsing() {
        let parser = JsonlParser::new(PathBuf::from("/test"));
        let test_path = Path::new("/test/file.jsonl");
        
        let line = r#"{"timestamp":"2025-06-09T10:30:00Z","uuid":"test-uuid","usage":{"inputTokens":10,"outputTokens":20,"cacheCreationInputTokens":5,"cacheReadInputTokens":3}}"#;
        
        let result = parser.parse_line(line, 1, test_path).unwrap();
        assert!(result.is_some());
        
        let usage_data = result.unwrap();
        let usage = usage_data.usage.unwrap();
        
        assert_eq!(usage.input_tokens, Some(10));
        assert_eq!(usage.output_tokens, Some(20));
        assert_eq!(usage.cache_creation_input_tokens, Some(5));
        assert_eq!(usage.cache_read_input_tokens, Some(3));
    }

    #[test]
    fn test_default_parser_uses_correct_path() {
        let parser = JsonlParser::default();
        
        // The default should use ~/.claude/projects
        let expected_suffix = PathBuf::from(".claude").join("projects");
        assert!(parser.base_dir.ends_with(&expected_suffix));
    }

    #[test]
    fn test_performance_large_file() {
        let (_temp_dir, base_dir) = setup_test_environment();
        let parser = JsonlParser::new(base_dir.clone());
        
        let file_path = base_dir.join("project1").join("large_conversation.jsonl");
        
        // Create a large file with 1000 lines
        let mut content = String::new();
        for i in 0..1000 {
            let line = format!(
                r#"{{"timestamp":"2025-06-09T10:30:{:02}Z","uuid":"test-uuid-{}","requestId":"req-{}","usage":{{"inputTokens":{},"outputTokens":{}}}}}"#,
                i % 60, // Vary seconds
                i,
                i,
                10 + (i % 100), // Vary input tokens
                20 + (i % 200)  // Vary output tokens
            );
            content.push_str(&line);
            content.push('\n');
        }
        
        fs::write(&file_path, content).expect("Failed to write large test file");
        
        // Measure performance
        let start = std::time::Instant::now();
        let result = parser.parse_file(&file_path).unwrap();
        let duration = start.elapsed();
        
        // Assertions
        assert_eq!(result.messages.len(), 1000);
        assert_eq!(result.parsed_lines, 1000);
        assert_eq!(result.skipped_lines, 0);
        assert_eq!(result.total_lines, 1000);
        
        // Performance assertion: should parse 1000 lines in less than 1 second
        assert!(duration.as_secs() < 1, "Parsing took too long: {:?}", duration);
        
        println!("Performance test: Parsed 1000 lines in {:?}", duration);
    }

    #[test]
    fn test_integration_with_real_claude_structure() {
        let (_temp_dir, base_dir) = setup_test_environment();
        let parser = JsonlParser::new(base_dir.clone());
        
        // Create a realistic Claude data structure
        let transcribr_dir = base_dir.join("transcribr");
        fs::create_dir_all(&transcribr_dir).expect("Failed to create transcribr dir");
        let file_path = transcribr_dir.join("conversation-abc123.jsonl");
        
        // Realistic Claude JSONL content based on actual Claude Code output structure
        let content = r#"{"timestamp":"2025-06-09T10:30:00.123Z","uuid":"msg_01ABC123DEF456GHI789","requestId":"req_01XYZ789ABC123DEF456","message":{"content":"Please help me with this task","model":"claude-sonnet-4-20250514","role":"user"},"usage":{"inputTokens":15,"outputTokens":0}}
{"timestamp":"2025-06-09T10:30:05.456Z","uuid":"msg_01ABC123DEF456GHI790","requestId":"req_01XYZ789ABC123DEF457","message":{"content":"I'll help you with that task. Let me break it down...","model":"claude-sonnet-4-20250514","role":"assistant"},"usage":{"inputTokens":15,"outputTokens":85},"costUSD":0.00167}
{"timestamp":"2025-06-09T10:30:15.789Z","uuid":"msg_01ABC123DEF456GHI791","requestId":"req_01XYZ789ABC123DEF458","message":{"content":"Thank you, that's very helpful!","model":"claude-sonnet-4-20250514","role":"user"},"usage":{"inputTokens":100,"outputTokens":0},"costUSD":0.0003}
{"timestamp":"2025-06-09T10:30:20.012Z","uuid":"msg_01ABC123DEF456GHI792","requestId":"req_01XYZ789ABC123DEF459","message":{"content":"You're welcome! Is there anything else I can help with?","model":"claude-sonnet-4-20250514","role":"assistant"},"usage":{"inputTokens":105,"outputTokens":15,"cacheReadInputTokens":25},"costUSD":0.00058}"#;
        
        fs::write(&file_path, content).expect("Failed to write realistic test file");
        
        let result = parser.parse_file(&file_path).unwrap();
        
        // Verify project extraction
        assert_eq!(result.project_path, PathBuf::from("transcribr"));
        
        // Verify all messages parsed correctly
        assert_eq!(result.messages.len(), 4);
        assert_eq!(result.parsed_lines, 4);
        assert_eq!(result.skipped_lines, 0);
        
        // Verify specific message content
        let first_msg = &result.messages[0];
        assert_eq!(first_msg.uuid, Some("msg_01ABC123DEF456GHI789".to_string()));
        assert_eq!(first_msg.request_id, Some("req_01XYZ789ABC123DEF456".to_string()));
        assert!(first_msg.message.is_some());
        assert!(first_msg.usage.is_some());
        assert_eq!(first_msg.cost_usd, None); // First message has no cost
        
        let second_msg = &result.messages[1];
        assert_eq!(second_msg.cost_usd, Some(0.00167));
        
        // Verify cache tokens in last message
        let last_msg = &result.messages[3];
        let usage = last_msg.usage.as_ref().unwrap();
        assert_eq!(usage.cache_read_input_tokens, Some(25));
        assert_eq!(last_msg.cost_usd, Some(0.00058));
    }
}