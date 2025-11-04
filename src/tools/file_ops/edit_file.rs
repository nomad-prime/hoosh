use crate::permissions::{ToolPermissionBuilder, ToolPermissionDescriptor};
use crate::security::PathValidator;
use crate::tools::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use colored::Colorize;
use serde::Deserialize;
use serde_json::{Value, json};
use similar::{ChangeTag, TextDiff};
use std::path::PathBuf;
use tokio::fs;

pub struct EditFileTool {
    path_validator: PathValidator,
}

impl EditFileTool {
    pub fn new() -> Self {
        let working_directory = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            path_validator: PathValidator::new(working_directory),
        }
    }

    pub fn with_working_directory(working_dir: PathBuf) -> Self {
        Self {
            path_validator: PathValidator::new(working_dir),
        }
    }

    async fn execute_impl(&self, args: &Value) -> ToolResult<String> {
        let args: EditFileArgs =
            serde_json::from_value(args.clone()).map_err(|e| ToolError::InvalidArguments {
                tool: "edit_file".to_string(),
                message: e.to_string(),
            })?;

        // Validate that old_string and new_string are different
        if args.old_string == args.new_string {
            return Err(ToolError::EditFailed {
                message: "old_string and new_string must be different".to_string(),
            });
        }

        let file_path = self
            .path_validator
            .validate_and_resolve(&args.path)
            .map_err(|e| ToolError::SecurityViolation {
                message: e.to_string(),
            })?;

        // Read the file
        let content = fs::read_to_string(&file_path)
            .await
            .map_err(|_| ToolError::ReadFailed {
                path: file_path.clone(),
            })?;

        // Perform the replacement
        let new_content = if args.replace_all {
            // Replace all occurrences
            let count = content.matches(&args.old_string).count();
            if count == 0 {
                return Err(ToolError::EditFailed {
                    message: format!(
                        "String not found in file: '{}'",
                        if args.old_string.len() > 50 {
                            format!("{}...", &args.old_string[..50])
                        } else {
                            args.old_string.clone()
                        }
                    ),
                });
            }
            let result = content.replace(&args.old_string, &args.new_string);
            (result, count)
        } else {
            // Replace only if unique
            let matches: Vec<_> = content.match_indices(&args.old_string).collect();
            match matches.len() {
                0 => {
                    return Err(ToolError::EditFailed {
                        message: format!(
                            "String not found in file: '{}'",
                            if args.old_string.len() > 50 {
                                format!("{}...", &args.old_string[..50])
                            } else {
                                args.old_string.clone()
                            }
                        ),
                    });
                }
                1 => {
                    let result = content.replacen(&args.old_string, &args.new_string, 1);
                    (result, 1)
                }
                n => {
                    return Err(ToolError::EditFailed {
                        message: format!(
                            "String appears {} times in file. Use replace_all=true to replace all occurrences, or provide more context to make the match unique.",
                            n
                        ),
                    });
                }
            }
        };

        // Write the modified content back
        fs::write(&file_path, &new_content.0)
            .await
            .map_err(|_| ToolError::WriteFailed {
                path: file_path.clone(),
            })?;

        Ok(format!(
            "Successfully edited {} (replaced {} occurrence{})",
            file_path.display(),
            new_content.1,
            if new_content.1 == 1 { "" } else { "s" }
        ))
    }
}

#[derive(Deserialize)]
struct EditFileArgs {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[async_trait]
impl Tool for EditFileTool {
    async fn execute(&self, args: &Value) -> ToolResult<String> {
        self.execute_impl(args).await
    }

    fn name(&self) -> &'static str {
        "edit_file"
    }

    fn display_name(&self) -> &'static str {
        "edit"
    }

    fn description(&self) -> &'static str {
        "Edit a file by replacing exact string matches. Use this for surgical edits to existing files."
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to edit (relative to working directory)"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to find and replace (must be unique unless replace_all=true)"
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement string (must be different from old_string)"
                },
                "replace_all": {
                    "type": "boolean",
                    "default": false,
                    "description": "If true, replace all occurrences. If false (default), the string must be unique in the file."
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn format_call_display(&self, args: &Value) -> String {
        if let Ok(parsed_args) = serde_json::from_value::<EditFileArgs>(args.clone()) {
            format!("Edit({})", parsed_args.path)
        } else {
            "Edit(?)".to_string()
        }
    }

    fn result_summary(&self, result: &str) -> String {
        // Extract occurrence count from result like "Successfully edited ... (replaced N occurrence(s))"
        if let Some(replaced_part) = result.split("replaced ").nth(1)
            && let Some(count_str) = replaced_part.split(" occurrence").next()
        {
            return format!(
                "Replaced {} occurrence{}",
                count_str,
                if count_str == "1" { "" } else { "s" }
            );
        }
        "File edited successfully".to_string()
    }

    async fn generate_preview(&self, args: &Value) -> Option<String> {
        let args: EditFileArgs = serde_json::from_value(args.clone()).ok()?;

        let file_path = self.path_validator.validate_and_resolve(&args.path).ok()?;
        let content = fs::read_to_string(&file_path).await.ok()?;

        // Generate unified diff
        let preview = self.generate_diff(
            &content,
            &args.old_string,
            &args.new_string,
            args.replace_all,
        );
        Some(preview)
    }

    fn describe_permission(&self, target: Option<&str>) -> ToolPermissionDescriptor {
        use crate::permissions::FilePatternMatcher;
        use std::sync::Arc;

        ToolPermissionBuilder::new(self, target.unwrap_or("*"))
            .into_destructive()
            .with_display_name("Edit")
            .with_pattern_matcher(Arc::new(FilePatternMatcher))
            .build()
            .expect("Failed to build EditFileTool permission descriptor")
    }
}

impl EditFileTool {
    /// Generate a unified diff showing what will change using the similar crate
    fn generate_diff(
        &self,
        content: &str,
        old_string: &str,
        new_string: &str,
        replace_all: bool,
    ) -> String {
        // Find all matches
        let matches: Vec<_> = content.match_indices(old_string).collect();

        if matches.is_empty() {
            return format!("No matches found for:\n{}", old_string);
        }

        // Determine which matches will be replaced
        let replacements = if replace_all {
            matches.len()
        } else {
            if matches.len() > 1 {
                return format!(
                    "Found {} matches (use replace_all=true to replace all)",
                    matches.len()
                );
            }
            1
        };

        // Perform the replacement to get the new content
        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        // Use similar crate to generate unified diff
        let diff = TextDiff::from_lines(content, &new_content);

        let mut output = String::new();
        output.push_str(&format!(
            "{}\n\n",
            format!(
                "Will replace {} occurrence{}:",
                replacements,
                if replacements == 1 { "" } else { "s" }
            )
            .bold()
            .cyan()
        ));

        // Collect all changes first to determine context windows
        let all_changes: Vec<_> = diff.iter_all_changes().collect();

        // Find indices of lines that have actual changes (not Equal)
        let mut changed_indices = Vec::new();
        for (idx, change) in all_changes.iter().enumerate() {
            if !matches!(change.tag(), ChangeTag::Equal) {
                changed_indices.push(idx);
            }
        }

        if changed_indices.is_empty() {
            return output;
        }

        // Determine which lines to show (changed lines + 5 lines context on each side)
        const CONTEXT_LINES: usize = 5;
        let mut lines_to_show = std::collections::HashSet::new();

        for &changed_idx in &changed_indices {
            let start = changed_idx.saturating_sub(CONTEXT_LINES);
            let end = (changed_idx + CONTEXT_LINES + 1).min(all_changes.len());
            for i in start..end {
                lines_to_show.insert(i);
            }
        }

        // Convert to sorted vec for sequential processing
        let mut lines_to_show: Vec<_> = lines_to_show.into_iter().collect();
        lines_to_show.sort_unstable();

        // Track line numbers for old and new files
        let mut old_line = 1;
        let mut new_line = 1;
        let mut last_shown_idx = None;

        // Show diff with context, with ellipsis for skipped sections
        for (actual_idx, change) in all_changes.iter().enumerate() {
            // Update line counters
            match change.tag() {
                ChangeTag::Delete => old_line += 1,
                ChangeTag::Insert => new_line += 1,
                ChangeTag::Equal => {
                    old_line += 1;
                    new_line += 1;
                }
            }

            // Check if we should show this line
            if !lines_to_show.contains(&actual_idx) {
                continue;
            }

            // Show ellipsis if we skipped lines
            if let Some(last_idx) = last_shown_idx
                && actual_idx > last_idx + 1
            {
                output.push_str(&format!("  {}\n", "...".dimmed()));
            }
            last_shown_idx = Some(actual_idx);

            let line_content = change.to_string();
            let line_content = line_content.trim_end();

            let formatted_line = match change.tag() {
                ChangeTag::Delete => {
                    let line_str = format!("  {:4} {:4} - {}", old_line - 1, " ", line_content);
                    line_str.bright_red().to_string()
                }
                ChangeTag::Insert => {
                    let line_str = format!("  {:4} {:4} + {}", " ", new_line - 1, line_content);
                    line_str.green().to_string()
                }
                ChangeTag::Equal => {
                    let line_str =
                        format!("  {:4} {:4}   {}", old_line - 1, new_line - 1, line_content);
                    line_str.dimmed().to_string()
                }
            };
            output.push_str(&formatted_line);
            output.push('\n');
        }

        output
    }
}

impl Default for EditFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_edit_file_tool_basic() {
        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let test_file = temp_dir.path().join("test.txt");
        let content = "Hello, World!\nThis is a test.";

        fs::write(&test_file, content)
            .await
            .expect("Failed to write test file");

        let tool = EditFileTool::with_working_directory(temp_dir.path().to_path_buf());
        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "World",
            "new_string": "Rust"
        });

        let result = tool.execute(&args).await.expect("Failed to execute tool");
        assert!(result.contains("Successfully edited"));
        assert!(result.contains("replaced 1 occurrence"));

        let modified_content = fs::read_to_string(&test_file)
            .await
            .expect("Failed to read modified file");
        assert_eq!(modified_content, "Hello, Rust!\nThis is a test.");
    }

    #[tokio::test]
    async fn test_edit_file_tool_replace_all() {
        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let test_file = temp_dir.path().join("test.txt");
        let content = "foo bar foo baz foo";

        fs::write(&test_file, content)
            .await
            .expect("Failed to write test file");

        let tool = EditFileTool::with_working_directory(temp_dir.path().to_path_buf());
        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "foo",
            "new_string": "qux",
            "replace_all": true
        });

        let result = tool.execute(&args).await.expect("Failed to execute tool");
        assert!(result.contains("Successfully edited"));
        assert!(result.contains("replaced 3 occurrences"));

        let modified_content = fs::read_to_string(&test_file)
            .await
            .expect("Failed to read modified file");
        assert_eq!(modified_content, "qux bar qux baz qux");
    }

    #[tokio::test]
    async fn test_edit_file_tool_not_unique() {
        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let test_file = temp_dir.path().join("test.txt");
        let content = "foo bar foo baz";

        fs::write(&test_file, content)
            .await
            .expect("Failed to write test file");

        let tool = EditFileTool::with_working_directory(temp_dir.path().to_path_buf());
        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "foo",
            "new_string": "qux"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        let error = result.expect_err("Expected error but got success");
        assert!(error.to_string().contains("appears 2 times"));
        assert!(error.to_string().contains("replace_all=true"));
    }

    #[tokio::test]
    async fn test_edit_file_tool_string_not_found() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let content = "Hello, World!";

        fs::write(&test_file, content).await.unwrap();

        let tool = EditFileTool::with_working_directory(temp_dir.path().to_path_buf());
        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "Goodbye",
            "new_string": "Hello"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("String not found"));
    }

    #[tokio::test]
    async fn test_edit_file_tool_same_strings() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let content = "Hello, World!";

        fs::write(&test_file, content).await.unwrap();

        let tool = EditFileTool::with_working_directory(temp_dir.path().to_path_buf());
        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "World",
            "new_string": "World"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("must be different"));
    }

    #[tokio::test]
    async fn test_edit_file_tool_multiline() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let content = "fn main() {\n    println!(\"Hello\");\n}";

        fs::write(&test_file, content).await.unwrap();

        let tool = EditFileTool::with_working_directory(temp_dir.path().to_path_buf());
        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "fn main() {\n    println!(\"Hello\");\n}",
            "new_string": "fn main() {\n    println!(\"Goodbye\");\n}"
        });

        let result = tool.execute(&args).await.unwrap();
        assert!(result.contains("Successfully edited"));

        let modified_content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(
            modified_content,
            "fn main() {\n    println!(\"Goodbye\");\n}"
        );
    }
}
