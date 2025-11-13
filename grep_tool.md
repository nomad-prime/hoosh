# Ticket: Implement Grep Tool with Ripgrep Backend

**Priority:** High  
**Effort:** Medium (4-6 hours)  
**Depends on:** None

## Objective

Implement a dedicated `grep` tool that provides fast, structured code search capabilities using ripgrep as the backend.
This tool integrates with the existing Tool trait and provides LLM-friendly structured output for code navigation and
search tasks.

## Requirements

### Core Functionality

1. **Pattern matching** with ripgrep regex syntax
2. **Three output modes:**
    - `files_with_matches` (default) - Return only file paths
    - `content` - Return matching lines with optional context
    - `count` - Return match counts per file
3. **File filtering:**
    - `glob` - File pattern matching (e.g., `*.rs`, `**/*.{ts,tsx}`)
    - `type` - File type filtering (e.g., `rust`, `python`, `javascript`)
    - `path` - Optional search scope (defaults to current directory)
4. **Search options:**
    - `-i` - Case insensitive search
    - `-n` - Line numbers (default: true for content mode)
    - `-A/-B/-C` - Context lines (after/before/both)
    - `multiline` - Multi-line pattern matching
5. **Result limiting:**
    - `head_limit` - Limit to first N results
    - `offset` - Skip first N results (for pagination)

## Implementation

### File: `src/tools/grep.rs`

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::process::Command;

use crate::permissions::{ToolPermissionDescriptor, ToolPermissionBuilder};
use super::{Tool, ToolResult, ToolError};

#[derive(Debug)]
pub struct GrepTool;

#[derive(Debug, Deserialize)]
struct GrepArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default = "default_output_mode")]
    output_mode: OutputMode,
    #[serde(default)]
    glob: Option<String>,
    #[serde(rename = "type")]
    #[serde(default)]
    file_type: Option<String>,
    #[serde(rename = "-i")]
    #[serde(default)]
    case_insensitive: bool,
    #[serde(rename = "-n")]
    #[serde(default = "default_true")]
    line_numbers: bool,
    #[serde(rename = "-A")]
    #[serde(default)]
    after_context: Option<u32>,
    #[serde(rename = "-B")]
    #[serde(default)]
    before_context: Option<u32>,
    #[serde(rename = "-C")]
    #[serde(default)]
    context: Option<u32>,
    #[serde(default)]
    multiline: bool,
    #[serde(default)]
    head_limit: Option<u32>,
    #[serde(default)]
    offset: Option<u32>,
}

fn default_output_mode() -> OutputMode {
    OutputMode::FilesWithMatches
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
enum OutputMode {
    FilesWithMatches,
    Content,
    Count,
}

// Ripgrep JSON message types
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum RipgrepMessage {
    #[serde(rename = "match")]
    Match { data: MatchData },
    #[serde(rename = "context")]
    Context { data: ContextData },
    #[serde(rename = "begin")]
    Begin { data: BeginData },
    #[serde(rename = "end")]
    End { data: EndData },
}

#[derive(Debug, Deserialize)]
struct MatchData {
    path: PathData,
    lines: LineData,
    line_number: Option<u64>,
    #[serde(default)]
    submatches: Vec<SubMatch>,
}

#[derive(Debug, Deserialize)]
struct ContextData {
    path: PathData,
    lines: LineData,
    line_number: u64,
}

#[derive(Debug, Deserialize)]
struct BeginData {
    path: PathData,
}

#[derive(Debug, Deserialize)]
struct EndData {
    path: PathData,
    #[serde(default)]
    stats: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct PathData {
    text: String,
}

#[derive(Debug, Deserialize)]
struct LineData {
    text: String,
}

#[derive(Debug, Deserialize)]
struct SubMatch {
    #[serde(rename = "match")]
    match_text: MatchText,
}

#[derive(Debug, Deserialize)]
struct MatchText {
    text: String,
}

#[derive(Debug, Serialize)]
struct GrepResult {
    matches: Vec<Match>,
    total_count: usize,
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct Match {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    line_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<u32>,
}

impl GrepTool {
    pub fn new() -> Self {
        Self
    }

    fn build_command(&self, args: &GrepArgs) -> Result<Command, ToolError> {
        // Check if ripgrep is available
        if which::which("rg").is_err() {
            return Err(ToolError::ExecutionFailed(
                "ripgrep (rg) not found in PATH. Install with:\n  \
                 macOS:        brew install ripgrep\n  \
                 Ubuntu/Debian: apt install ripgrep\n  \
                 Arch:         pacman -S ripgrep\n  \
                 Windows:      choco install ripgrep\n  \
                 Cargo:        cargo install ripgrep".to_string()
            ));
        }

        let mut cmd = Command::new("rg");

        // Always use JSON output for structured parsing
        cmd.arg("--json");

        // Output mode
        match args.output_mode {
            OutputMode::FilesWithMatches => {
                cmd.arg("--files-with-matches");
            }
            OutputMode::Content => {
                // Default behavior
                if args.line_numbers {
                    cmd.arg("--line-number");
                } else {
                    cmd.arg("--no-line-number");
                }
            }
            OutputMode::Count => {
                cmd.arg("--count");
            }
        }

        // Case sensitivity
        if args.case_insensitive {
            cmd.arg("--ignore-case");
        }

        // Context lines
        if let Some(context) = args.context {
            cmd.arg(format!("--context={}", context));
        } else {
            if let Some(after) = args.after_context {
                cmd.arg(format!("--after-context={}", after));
            }
            if let Some(before) = args.before_context {
                cmd.arg(format!("--before-context={}", before));
            }
        }

        // Multiline
        if args.multiline {
            cmd.arg("--multiline");
        }

        // File filtering
        if let Some(glob) = &args.glob {
            cmd.arg("--glob").arg(glob);
        }
        if let Some(file_type) = &args.file_type {
            cmd.arg("--type").arg(file_type);
        }

        // Result limiting (ripgrep's max-count applies per-file, but it's the best we can do)
        if let Some(limit) = args.head_limit {
            cmd.arg("--max-count").arg(limit.to_string());
        }

        // Pattern
        cmd.arg(&args.pattern);

        // Path (defaults to current directory)
        if let Some(path) = &args.path {
            cmd.arg(path);
        } else {
            cmd.arg(".");
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        Ok(cmd)
    }

    async fn parse_output(
        &self,
        args: &GrepArgs,
        output: String,
    ) -> Result<GrepResult, ToolError> {
        let mut matches = Vec::new();
        let mut current_file_matches: Vec<Match> = Vec::new();

        for line in output.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let msg: RipgrepMessage = serde_json::from_str(line)
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse ripgrep JSON: {}", e)))?;

            match msg {
                RipgrepMessage::Begin { .. } => {
                    current_file_matches.clear();
                }
                RipgrepMessage::Match { data } => {
                    let match_entry = Match {
                        path: data.path.text.clone(),
                        line_number: data.line_number,
                        content: Some(data.lines.text),
                        count: None,
                    };
                    current_file_matches.push(match_entry);
                }
                RipgrepMessage::Context { data } => {
                    // Context lines are included with matches in content mode
                    if matches!(args.output_mode, OutputMode::Content) {
                        let context_entry = Match {
                            path: data.path.text,
                            line_number: Some(data.line_number),
                            content: Some(format!("  {}", data.lines.text)), // Indent context
                            count: None,
                        };
                        current_file_matches.push(context_entry);
                    }
                }
                RipgrepMessage::End { data } => {
                    if matches!(args.output_mode, OutputMode::Count) {
                        // For count mode, stats contain the count
                        if let Some(stats) = data.stats {
                            if let Some(matches_count) = stats.get("matches") {
                                if let Some(count) = matches_count.as_u64() {
                                    matches.push(Match {
                                        path: data.path.text,
                                        line_number: None,
                                        content: None,
                                        count: Some(count as u32),
                                    });
                                }
                            }
                        }
                    } else {
                        // Add accumulated matches for this file
                        matches.extend(current_file_matches.drain(..));
                    }
                }
            }
        }

        // Handle offset and limit
        let total_count = matches.len();
        let offset = args.offset.unwrap_or(0) as usize;
        let limit = args.head_limit.map(|l| l as usize);

        let matches = if offset > 0 {
            matches.into_iter().skip(offset).collect::<Vec<_>>()
        } else {
            matches
        };

        let (matches, truncated) = if let Some(limit) = limit {
            let truncated = matches.len() > limit;
            (matches.into_iter().take(limit).collect(), truncated)
        } else {
            (matches, false)
        };

        Ok(GrepResult {
            matches,
            total_count,
            truncated,
        })
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn display_name(&self) -> &'static str {
        "Grep"
    }

    fn description(&self) -> &'static str {
        "Search code using regex patterns. Built on ripgrep for fast, accurate searches across large codebases. Use for finding specific code patterns, function definitions, imports, or any text pattern."
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for (ripgrep syntax). Escape literal braces: interface\\{\\}"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in (defaults to current directory)"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["files_with_matches", "content", "count"],
                    "description": "files_with_matches: paths only | content: matching lines | count: match counts",
                    "default": "files_with_matches"
                },
                "glob": {
                    "type": "string",
                    "description": "File pattern: *.rs, **/*.{ts,tsx}"
                },
                "type": {
                    "type": "string",
                    "description": "File type filter: rust, python, javascript, etc."
                },
                "-i": {
                    "type": "boolean",
                    "description": "Case insensitive search"
                },
                "-n": {
                    "type": "boolean",
                    "description": "Show line numbers (default: true for content mode)",
                    "default": true
                },
                "-A": {
                    "type": "integer",
                    "description": "Lines of context after match"
                },
                "-B": {
                    "type": "integer",
                    "description": "Lines of context before match"
                },
                "-C": {
                    "type": "integer",
                    "description": "Lines of context before and after match"
                },
                "multiline": {
                    "type": "boolean",
                    "description": "Enable multi-line pattern matching"
                },
                "head_limit": {
                    "type": "integer",
                    "description": "Limit to first N results"
                },
                "offset": {
                    "type": "integer",
                    "description": "Skip first N results (for pagination)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: &Value) -> ToolResult<String> {
        let args: GrepArgs = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::InvalidArguments(format!("Invalid grep arguments: {}", e)))?;

        let mut cmd = self.build_command(&args)?;

        let output = cmd.output().await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to execute ripgrep: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionFailed(format!("ripgrep failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let result = self.parse_output(&args, stdout.to_string()).await?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to serialize result: {}", e)))
    }

    fn describe_permission(&self, target: Option<&str>) -> ToolPermissionDescriptor {
        ToolPermissionBuilder::new(self, target.unwrap_or("*"))
            .into_read_only()
            .build()
            .expect("Failed to build grep permission descriptor")
    }

    fn format_call_display(&self, args: &Value) -> String {
        if let Ok(grep_args) = serde_json::from_value::<GrepArgs>(args.clone()) {
            let path_str = grep_args.path.as_deref().unwrap_or(".");
            format!("Grep({}, {})", grep_args.pattern, path_str)
        } else {
            "Grep(...)".to_string()
        }
    }

    fn result_summary(&self, result: &str) -> String {
        if let Ok(grep_result) = serde_json::from_str::<GrepResult>(result) {
            let count = grep_result.matches.len();
            let truncated = if grep_result.truncated { " (truncated)" } else { "" };
            format!("Found {} match{}{}", count, if count == 1 { "" } else { "es" }, truncated)
        } else {
            "Search completed".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grep_tool_creation() {
        let tool = GrepTool::new();
        assert_eq!(tool.name(), "grep");
        assert_eq!(tool.display_name(), "Grep");
    }

    #[test]
    fn test_parameter_schema() {
        let tool = GrepTool::new();
        let schema = tool.parameter_schema();

        assert!(schema["properties"]["pattern"].is_object());
        assert!(schema["properties"]["output_mode"].is_object());
        assert_eq!(schema["required"], json!(["pattern"]));
    }

    #[tokio::test]
    async fn test_ripgrep_not_installed() {
        // This test verifies error handling when ripgrep is missing
        // Skip if ripgrep is actually installed
        if which::which("rg").is_ok() {
            return;
        }

        let tool = GrepTool::new();
        let args = json!({
            "pattern": "test"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ripgrep"));
    }

    // Integration tests - run with: cargo test --ignored
    #[tokio::test]
    #[ignore]
    async fn test_files_with_matches() {
        let tool = GrepTool::new();
        let args = json!({
            "pattern": "async fn",
            "glob": "*.rs",
            "output_mode": "files_with_matches"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_ok());

        let result_str = result.unwrap();
        let grep_result: GrepResult = serde_json::from_str(&result_str).unwrap();
        assert!(!grep_result.matches.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_content_with_context() {
        let tool = GrepTool::new();
        let args = json!({
            "pattern": "impl Tool",
            "output_mode": "content",
            "-C": 2,
            "-n": true,
            "type": "rust"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_count_mode() {
        let tool = GrepTool::new();
        let args = json!({
            "pattern": "use ",
            "output_mode": "count",
            "glob": "src/**/*.rs"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_case_insensitive() {
        let tool = GrepTool::new();
        let args = json!({
            "pattern": "TODO",
            "-i": true,
            "output_mode": "files_with_matches"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_ok());
    }
}
```

### File: `src/tools/mod.rs`

Add to existing exports:

```rust
pub mod grep;
pub use grep::GrepTool;
```

### File: `src/tools/provider.rs`

Add to `BuiltinToolProvider::new()`:

```rust
impl BuiltinToolProvider {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir: working_dir.clone(),
            tools: vec![
                Arc::new(ReadFileTool::new(working_dir.clone())),
                Arc::new(WriteFileTool::new(working_dir.clone())),
                Arc::new(EditFileTool::new(working_dir.clone())),
                Arc::new(ListDirectoryTool::new(working_dir.clone())),
                Arc::new(BashTool::new(working_dir.clone())),
                Arc::new(GrepTool::new()), // Add this
            ],
        }
    }
}
```

### File: `Cargo.toml`

Add dependency:

```toml
which = "6.0"
```

## Testing Strategy

### Unit Tests

```bash
# Run basic tests
cargo test grep

# Run integration tests (requires ripgrep installed)
cargo test grep --ignored
```

### Manual Testing

```bash
# Test basic search
hoosh chat "search for 'async fn' in rust files"

# Test with context
hoosh chat "find TODO comments with 3 lines of context"

# Test count mode
hoosh chat "count how many times 'impl Tool' appears"

# Test glob patterns
hoosh chat "search for 'error' in src/tools/*.rs"
```

## Documentation

### README.md

Add section:

```markdown
### Grep Tool

Fast code search using ripgrep.

**Requirement:** Ripgrep must be installed on your system.

- macOS: `brew install ripgrep`
- Ubuntu/Debian: `apt install ripgrep`
- Arch: `pacman -S ripgrep`
- Windows: `choco install ripgrep`
- Cargo: `cargo install ripgrep`

**Features:**

- Regex pattern matching
- File type and glob filtering
- Context lines for matches
- Three output modes: files only, content, or counts
- Case-insensitive search
- Multi-line pattern matching
- Result pagination

**Examples:**

```json
// Find all async functions
{"pattern": "async fn", "type": "rust"}

// Search with context
{"pattern": "TODO", "output_mode": "content", "-C": 3}

// Count occurrences
{"pattern": "unwrap", "output_mode": "count"}
```

```

## Error Handling

The tool handles these error cases:
1. **Ripgrep not installed** - Clear installation instructions
2. **Invalid regex pattern** - Pass through ripgrep's error message
3. **Permission errors** - Report file access issues
4. **Invalid arguments** - JSON deserialization errors with context

## Future Enhancements
- Add optional preview for grep results (show first few matches)
- Support for `.ripgreprc` configuration files
- Caching for repeated searches
- Search performance metrics
- Custom file type definitions

## Acceptance Criteria
- [ ] Tool successfully executes ripgrep with all parameter combinations
- [ ] JSON output is properly parsed and formatted
- [ ] Clear error message when ripgrep is not installed
- [ ] All three output modes work correctly
- [ ] Context lines display properly
- [ ] Pagination (offset/limit) works as expected
- [ ] Permission descriptor is read-only
- [ ] Tool integrates with existing tool registry
- [ ] Unit tests pass
- [ ] Integration tests pass (with ripgrep installed)
- [ ] Documentation is complete
