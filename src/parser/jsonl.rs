use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct Message {
    #[serde(deserialize_with = "deserialize_content")]
    pub content: Option<String>,
    pub model: Option<String>,
    pub role: Option<String>,
    // Claude Code format includes usage in message
    pub usage: Option<ClaudeCodeUsage>,
}

/// Custom deserializer for content field that handles both string and array formats
fn deserialize_content<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<Value> = Option::deserialize(deserializer)?;
    
    match value {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(Value::Array(arr)) => {
            // Extract text content from array of content blocks
            let mut text_parts = Vec::new();
            
            for item in arr {
                if let Value::Object(obj) = item {
                    // Check if this is a text content block
                    if let (Some(Value::String(content_type)), Some(Value::String(text))) = 
                        (obj.get("type"), obj.get("text")) {
                        if content_type == "text" {
                            text_parts.push(text.clone());
                        }
                    }
                    // For tool_use blocks, we could extract the tool name/input if needed
                    // but for now we'll just skip them as they don't contribute to text content
                }
            }
            
            if text_parts.is_empty() {
                Ok(None)
            } else {
                Ok(Some(text_parts.join(" ")))
            }
        }
        Some(_) => {
            // For any other type, convert to string representation
            Ok(Some(value.unwrap().to_string()))
        }
    }
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
pub struct ClaudeCodeUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct UsageData {
    pub timestamp: Option<String>,
    pub uuid: Option<String>,
    #[serde(rename = "requestId")]
    pub request_id: Option<String>,
    pub message: Option<Message>,
    pub usage: Option<Usage>,
    #[serde(rename = "costUSD")]
    pub cost_usd: Option<f64>,
    pub cwd: Option<String>,
    #[serde(rename = "originalCwd")]
    pub original_cwd: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedConversation {
    pub project_path: PathBuf,
    pub file_path: PathBuf,
    pub messages: Vec<UsageData>,
    pub total_lines: usize,
    pub parsed_lines: usize,
    pub skipped_lines: usize,
    pub smart_project_name: Option<String>,
}

#[derive(Clone)]
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
        self.parse_file_with_verbose(file_path, false)
    }

    /// Parse a single JSONL file with optional verbose output
    pub fn parse_file_with_verbose(&self, file_path: &Path, verbose: bool) -> Result<ParsedConversation> {
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
                            if verbose {
                                eprintln!("Warning: Skipping malformed JSON at {}:{}: {}", 
                                         file_path.display(), line_num + 1, e);
                            }
                            skipped_lines += 1;
                        }
                    }
                }
                Err(e) => {
                    if verbose {
                        eprintln!("Warning: Failed to read line {} in {}: {}", 
                                 line_num + 1, file_path.display(), e);
                    }
                    skipped_lines += 1;
                }
            }
        }

        // Extract smart project name from cwd/originalCwd in messages
        let smart_project_name = self.extract_smart_project_name(&messages);

        Ok(ParsedConversation {
            project_path,
            file_path: file_path.to_path_buf(),
            messages,
            total_lines,
            parsed_lines,
            skipped_lines,
            smart_project_name,
        })
    }

    /// Parse a single line of JSONL
    fn parse_line(&self, line: &str, line_num: usize, file_path: &Path) -> Result<Option<UsageData>> {
        let usage_data: UsageData = serde_json::from_str(line)
            .map_err(|e| anyhow!("JSON parse error: {}", e))?;

        // Validate timestamp field - skip only if present but empty
        if let Some(ref timestamp) = usage_data.timestamp {
            if timestamp.is_empty() {
                return Ok(None); // Skip entries with empty timestamps
            }
        }
        // If timestamp is None, we'll continue processing - it's now optional

        // For deduplication purposes, we prefer both uuid and request_id
        // Note: Missing UUIDs/requestIds will be handled by deduplication engine
        // Warnings will be shown only in verbose mode

        // Normalize usage data to handle Claude Code format
        let normalized_usage_data = self.normalize_usage_data(usage_data);

        Ok(Some(normalized_usage_data))
    }

    /// Normalize usage data to handle both old and Claude Code formats
    fn normalize_usage_data(&self, mut usage_data: UsageData) -> UsageData {
        // If usage field is empty but message has usage (Claude Code format)
        if usage_data.usage.is_none() {
            if let Some(ref message) = usage_data.message {
                if let Some(ref claude_usage) = message.usage {
                    // Convert Claude Code usage to our standard format
                    usage_data.usage = Some(Usage {
                        input_tokens: claude_usage.input_tokens,
                        output_tokens: claude_usage.output_tokens,
                        cache_creation_input_tokens: claude_usage.cache_creation_input_tokens,
                        cache_read_input_tokens: claude_usage.cache_read_input_tokens,
                    });
                }
            }
        }
        
        usage_data
    }

    /// Extract smart project name from cwd/originalCwd fields in messages
    fn extract_smart_project_name(&self, messages: &[UsageData]) -> Option<String> {
        // Try to find a message with cwd or originalCwd
        for message in messages {
            if let Some(ref cwd) = message.cwd {
                if let Some(project_name) = self.extract_project_name_from_path(cwd) {
                    return Some(project_name);
                }
            }
            if let Some(ref original_cwd) = message.original_cwd {
                if let Some(project_name) = self.extract_project_name_from_path(original_cwd) {
                    return Some(project_name);
                }
            }
        }
        None
    }

    /// Extract meaningful project name from a file path
    fn extract_project_name_from_path(&self, path: &str) -> Option<String> {
        let path = Path::new(path);
        
        // Handle special cases like .config/nvim -> nvim
        if let Some(parent) = path.parent() {
            if let Some(parent_name) = parent.file_name().and_then(|n| n.to_str()) {
                if parent_name == ".config" {
                    return path.file_name()?.to_str().map(|s| s.to_string());
                }
            }
        }
        
        // Default: use the last directory component
        path.file_name()?.to_str().map(|s| s.to_string())
    }

    /// Unified project name extraction for consistency across all commands
    /// This is the single source of truth for project names in the application
    pub fn get_unified_project_name(&self, file_path: &Path, messages: &[UsageData]) -> String {
        // Priority 1: Try to extract smart name from cwd/originalCwd in messages
        for message in messages {
            if let Some(ref cwd) = message.cwd {
                if let Some(project_name) = self.extract_project_name_from_path(cwd) {
                    return project_name;
                }
            }
            if let Some(ref original_cwd) = message.original_cwd {
                if let Some(project_name) = self.extract_project_name_from_path(original_cwd) {
                    return project_name;
                }
            }
        }
        
        // Priority 2: Fallback to directory-based extraction
        match self.extract_project_path(file_path) {
            Ok(project_path) => project_path.to_string_lossy().to_string(),
            Err(_) => "Unknown".to_string(),
        }
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
        self.parse_all_files_with_verbose(false)
    }

    /// Parse all JSONL files in the base directory with optional verbose output
    pub fn parse_all_files_with_verbose(&self, verbose: bool) -> Result<Vec<ParsedConversation>> {
        let files = self.find_jsonl_files()?;
        let mut conversations = Vec::new();

        for file_path in files {
            match self.parse_file_with_verbose(&file_path, verbose) {
                Ok(conversation) => {
                    conversations.push(conversation);
                }
                Err(e) => {
                    if verbose {
                        eprintln!("Warning: Failed to parse file {}: {}", file_path.display(), e);
                    }
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
            
            // Entry with no timestamp (should now be processed)
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
        assert_eq!(usage_data.timestamp, Some("2025-06-09T10:30:00Z".to_string()));
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
        assert_eq!(usage_data.timestamp, Some("2025-06-09T10:30:00Z".to_string()));
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
        
        // This should now succeed because timestamp field is optional
        let result = parser.parse_line(line, 1, test_path).unwrap();
        assert!(result.is_some());
        
        let usage_data = result.unwrap();
        assert_eq!(usage_data.timestamp, None);
        assert_eq!(usage_data.uuid, Some("test-uuid".to_string()));
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
        assert_eq!(result.parsed_lines, 5); // 5 valid entries including one without timestamp
        assert_eq!(result.skipped_lines, 2); // 1 empty + 1 malformed + 1 empty timestamp
        assert_eq!(result.total_lines, 8);
        assert_eq!(result.messages.len(), 5);
        
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

    #[test]
    fn test_new_format_with_array_content() {
        let parser = JsonlParser::new(PathBuf::from("/test"));
        let test_path = Path::new("/test/file.jsonl");
        
        // Test the new array-based content format
        let line = r#"{"timestamp":"2025-06-09T10:30:00Z","uuid":"test-uuid","message":{"content":[{"type":"text","text":"Hello world"},{"type":"tool_use","id":"tool1","name":"bash","input":{"command":"ls"}}],"model":"claude-sonnet-4","role":"assistant"},"usage":{"inputTokens":10,"outputTokens":20}}"#;
        
        let result = parser.parse_line(line, 1, test_path).unwrap();
        assert!(result.is_some());
        
        let usage_data = result.unwrap();
        let message = usage_data.message.unwrap();
        
        // Should extract only the text content, ignoring tool_use blocks
        assert_eq!(message.content, Some("Hello world".to_string()));
        assert_eq!(message.model, Some("claude-sonnet-4".to_string()));
        assert_eq!(message.role, Some("assistant".to_string()));
    }
    
    #[test]
    fn test_mixed_content_array() {
        let parser = JsonlParser::new(PathBuf::from("/test"));
        let test_path = Path::new("/test/file.jsonl");
        
        // Test array with multiple text blocks
        let line = r#"{"timestamp":"2025-06-09T10:30:00Z","uuid":"test-uuid","message":{"content":[{"type":"text","text":"First part"},{"type":"text","text":"Second part"}],"model":"claude-sonnet-4","role":"assistant"}}"#;
        
        let result = parser.parse_line(line, 1, test_path).unwrap();
        assert!(result.is_some());
        
        let usage_data = result.unwrap();
        let message = usage_data.message.unwrap();
        
        // Should join multiple text parts
        assert_eq!(message.content, Some("First part Second part".to_string()));
    }
    
    #[test]
    fn test_backwards_compatibility_string_content() {
        let parser = JsonlParser::new(PathBuf::from("/test"));
        let test_path = Path::new("/test/file.jsonl");
        
        // Test that old string format still works
        let line = r#"{"timestamp":"2025-06-09T10:30:00Z","uuid":"test-uuid","message":{"content":"Simple string content","model":"claude-sonnet-4","role":"user"}}"#;
        
        let result = parser.parse_line(line, 1, test_path).unwrap();
        assert!(result.is_some());
        
        let usage_data = result.unwrap();
        let message = usage_data.message.unwrap();
        
        // Should preserve string content as-is
        assert_eq!(message.content, Some("Simple string content".to_string()));
    }

    #[test]
    fn test_unified_project_name_extraction() {
        let parser = JsonlParser::new(PathBuf::from("/home/user/.claude/projects"));
        
        // Test case 1: Smart name from cwd field
        let usage_data = vec![UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("test-uuid".to_string()),
            request_id: Some("req-1".to_string()),
            message: None,
            usage: None,
            cost_usd: None,
            cwd: Some("/home/user/projs/transcribr".to_string()),
            original_cwd: None,
        }];
        
        let file_path = PathBuf::from("/home/user/.claude/projects/-home-user-projs-transcribr/conversation.jsonl");
        let project_name = parser.get_unified_project_name(&file_path, &usage_data);
        assert_eq!(project_name, "transcribr"); // Smart name from cwd, not directory
        
        // Test case 2: Smart name from originalCwd field
        let usage_data2 = vec![UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("test-uuid".to_string()),
            request_id: Some("req-1".to_string()),
            message: None,
            usage: None,
            cost_usd: None,
            cwd: None,
            original_cwd: Some("/home/user/.claude".to_string()),
        }];
        
        let file_path2 = PathBuf::from("/home/user/.claude/projects/-home-user--claude/conversation.jsonl");
        let project_name2 = parser.get_unified_project_name(&file_path2, &usage_data2);
        assert_eq!(project_name2, ".claude"); // Smart name from originalCwd
        
        // Test case 3: Fallback to directory extraction when no cwd available
        let usage_data3 = vec![UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("test-uuid".to_string()),
            request_id: Some("req-1".to_string()),
            message: None,
            usage: None,
            cost_usd: None,
            cwd: None,
            original_cwd: None,
        }];
        
        let file_path3 = PathBuf::from("/home/user/.claude/projects/simple-project/conversation.jsonl");
        let project_name3 = parser.get_unified_project_name(&file_path3, &usage_data3);
        assert_eq!(project_name3, "simple-project"); // Directory-based fallback
        
        // Test case 4: .config special case handling
        let usage_data4 = vec![UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("test-uuid".to_string()),
            request_id: Some("req-1".to_string()),
            message: None,
            usage: None,
            cost_usd: None,
            cwd: Some("/home/user/.config/nvim".to_string()),
            original_cwd: None,
        }];
        
        let file_path4 = PathBuf::from("/home/user/.claude/projects/-home-user--config-nvim/conversation.jsonl");
        let project_name4 = parser.get_unified_project_name(&file_path4, &usage_data4);
        assert_eq!(project_name4, "nvim"); // Special .config handling
    }

    #[test]
    fn test_consistency_across_commands() {
        // This test ensures that all commands use the same unified project name extraction
        let parser = JsonlParser::new(PathBuf::from("/home/user/.claude/projects"));
        
        // Test scenario: Directory name is verbose but cwd provides clean name
        let messages = vec![UsageData {
            timestamp: Some("2025-06-09T10:30:00Z".to_string()),
            uuid: Some("test-uuid".to_string()),
            request_id: Some("req-1".to_string()),
            message: None,
            usage: None,
            cost_usd: None,
            cwd: Some("/home/user/moneyz/transcribr".to_string()),
            original_cwd: None,
        }];
        
        // Test file with verbose directory name
        let file_path = PathBuf::from("/home/user/.claude/projects/-home-user-moneyz-transcribr/conversation.jsonl");
        
        // All functions that use project names should return the same result
        let unified_name = parser.get_unified_project_name(&file_path, &messages);
        
        // This should be the smart name from cwd, not the directory name
        assert_eq!(unified_name, "transcribr");
        
        // Verify fallback behavior when no cwd is available
        let empty_messages = vec![];
        let fallback_name = parser.get_unified_project_name(&file_path, &empty_messages);
        assert_eq!(fallback_name, "-home-user-moneyz-transcribr"); // Directory-based fallback
    }
}