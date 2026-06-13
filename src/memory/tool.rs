use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::memory::entrypoint::ENTRYPOINT_NAME;
use crate::permissions::{ToolPermissionBuilder, ToolPermissionDescriptor};
use crate::tools::{Tool, ToolError, ToolExecutionContext, ToolResult};

pub const VALID_TYPES: &[&str] = &["user", "feedback", "project", "reference"];

const MAX_INDEX_ENTRY_CHARS: usize = 200;

pub struct SaveMemoryTool {
    memory_root: PathBuf,
}

impl SaveMemoryTool {
    pub fn new(memory_root: PathBuf) -> Self {
        Self { memory_root }
    }
}

fn slugify(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() || ch == '/' || ch == '\\' {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches(|c: char| c == '_' || c == '-').to_string()
}

fn render_memory_file(name: &str, description: &str, kind: &str, body: &str) -> String {
    format!(
        "---\nname: {}\ndescription: {}\ntype: {}\n---\n\n{}\n",
        name,
        description.replace('\n', " "),
        kind,
        body.trim_end()
    )
}

fn render_index_entry(name: &str, slug: &str, description: &str) -> String {
    let line = format!("- [{}]({}.md) — {}", name, slug, description);
    if line.chars().count() <= MAX_INDEX_ENTRY_CHARS {
        return line;
    }
    let target = MAX_INDEX_ENTRY_CHARS.saturating_sub(1);
    let truncated: String = line.chars().take(target).collect();
    format!("{}…", truncated)
}

fn update_index(memory_root: &Path, slug: &str, entry: &str) -> std::io::Result<()> {
    let index_path = memory_root.join(ENTRYPOINT_NAME);
    let existing = fs::read_to_string(&index_path).unwrap_or_default();

    let needle = format!("]({}.md)", slug);
    let mut found = false;
    let mut out: Vec<String> = existing
        .lines()
        .map(|line| {
            if line.contains(&needle) {
                found = true;
                entry.to_string()
            } else {
                line.to_string()
            }
        })
        .collect();

    if !found {
        out.push(entry.to_string());
    }

    let mut joined = out.join("\n");
    if !joined.ends_with('\n') {
        joined.push('\n');
    }
    fs::write(&index_path, joined)
}

#[async_trait]
impl Tool for SaveMemoryTool {
    fn name(&self) -> &'static str {
        "save_memory"
    }

    fn display_name(&self) -> &'static str {
        "SaveMemory"
    }

    fn description(&self) -> &'static str {
        "Persist a memory across sessions. Use this when you learn something \
        about the user, receive feedback to apply in future sessions, gain \
        project context not derivable from code, or learn about an external \
        reference. Pick the type that fits and write a short description and \
        body. Updates the MEMORY.md index automatically."
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Short human-readable name. Becomes the slug for the file (e.g. 'user role' → user_role.md).",
                    "minLength": 1
                },
                "type": {
                    "type": "string",
                    "enum": VALID_TYPES,
                    "description": "user | feedback | project | reference"
                },
                "description": {
                    "type": "string",
                    "description": "One-line summary shown in MEMORY.md. Be specific — this drives future-session relevance.",
                    "minLength": 1
                },
                "body": {
                    "type": "string",
                    "description": "Memory body in markdown. For feedback/project, lead with the rule/fact then **Why:** and **How to apply:** lines.",
                    "minLength": 1
                }
            },
            "required": ["name", "type", "description", "body"]
        })
    }

    async fn execute(&self, args: &Value, _context: &ToolExecutionContext) -> ToolResult<String> {
        let name = args.get("name").and_then(Value::as_str).ok_or_else(|| {
            ToolError::InvalidArguments {
                tool: "save_memory".to_string(),
                message: "missing required field: name".to_string(),
            }
        })?;
        let kind = args.get("type").and_then(Value::as_str).ok_or_else(|| {
            ToolError::InvalidArguments {
                tool: "save_memory".to_string(),
                message: "missing required field: type".to_string(),
            }
        })?;
        let description = args
            .get("description")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "save_memory".to_string(),
                message: "missing required field: description".to_string(),
            })?;
        let body = args.get("body").and_then(Value::as_str).ok_or_else(|| {
            ToolError::InvalidArguments {
                tool: "save_memory".to_string(),
                message: "missing required field: body".to_string(),
            }
        })?;

        if !VALID_TYPES.contains(&kind) {
            return Err(ToolError::InvalidArguments {
                tool: "save_memory".to_string(),
                message: format!(
                    "invalid type {:?}; expected one of: {}",
                    kind,
                    VALID_TYPES.join(", ")
                ),
            });
        }

        let slug = slugify(name);
        if slug.is_empty() {
            return Err(ToolError::InvalidArguments {
                tool: "save_memory".to_string(),
                message: "name produces an empty slug after normalization".to_string(),
            });
        }

        let file_path = self.memory_root.join(format!("{}.md", slug));

        if !file_path.starts_with(&self.memory_root) {
            return Err(ToolError::ExecutionFailed {
                message: format!("refusing to write outside memory root: {:?}", file_path),
            });
        }

        fs::create_dir_all(&self.memory_root).map_err(|e| ToolError::ExecutionFailed {
            message: format!("save_memory: failed to create memory dir: {}", e),
        })?;

        let rendered = render_memory_file(name, description, kind, body);
        fs::write(&file_path, rendered).map_err(|e| ToolError::ExecutionFailed {
            message: format!("save_memory: failed to write file: {}", e),
        })?;

        let entry = render_index_entry(name, &slug, description);
        update_index(&self.memory_root, &slug, &entry).map_err(|e| ToolError::ExecutionFailed {
            message: format!("save_memory: failed to update index: {}", e),
        })?;

        Ok(format!("Saved {} memory to {}.md", kind, slug))
    }

    fn describe_permission(&self, target: Option<&str>) -> ToolPermissionDescriptor {
        let default = self
            .memory_root
            .join("<name>.md")
            .to_string_lossy()
            .into_owned();
        let target_str = target.unwrap_or(&default);
        ToolPermissionBuilder::new(self, target_str)
            .into_write_safe()
            .build()
            .expect("Failed to build save_memory permission descriptor")
    }

    fn format_call_display(&self, args: &Value) -> String {
        let name = args.get("name").and_then(Value::as_str).unwrap_or("?");
        let kind = args.get("type").and_then(Value::as_str).unwrap_or("?");
        format!("SaveMemory({}: {})", kind, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_context() -> ToolExecutionContext {
        ToolExecutionContext {
            tool_call_id: "test".to_string(),
            event_tx: None,
            parent_conversation_id: None,
        }
    }

    #[test]
    fn slugify_normalizes_human_names() {
        assert_eq!(slugify("User role"), "user_role");
        assert_eq!(slugify("Stop  summarizing!"), "stop_summarizing");
        assert_eq!(slugify("path/traversal"), "path_traversal");
        assert_eq!(slugify("--leading-dashes--"), "leading-dashes");
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn render_index_entry_caps_at_max_chars() {
        let long_desc = "x".repeat(500);
        let entry = render_index_entry("name", "name", &long_desc);
        assert!(entry.chars().count() <= MAX_INDEX_ENTRY_CHARS);
        assert!(entry.ends_with('…'));
    }

    #[tokio::test]
    async fn saves_memory_file_with_frontmatter_and_updates_index() {
        let dir = TempDir::new().unwrap();
        let tool = SaveMemoryTool::new(dir.path().to_path_buf());
        let result = tool
            .execute(
                &json!({
                    "name": "User role",
                    "type": "user",
                    "description": "data scientist focused on logging",
                    "body": "user is a data scientist currently investigating observability"
                }),
                &make_context(),
            )
            .await
            .expect("save succeeds");
        assert!(result.contains("user_role"));

        let file = dir.path().join("user_role.md");
        let content = fs::read_to_string(&file).unwrap();
        assert!(content.starts_with("---\n"));
        assert!(content.contains("type: user\n"));
        assert!(content.contains("data scientist focused on logging"));
        assert!(content.contains("observability"));

        let index = fs::read_to_string(dir.path().join(ENTRYPOINT_NAME)).unwrap();
        assert!(index.contains("[User role](user_role.md)"));
        assert!(index.contains("data scientist focused on logging"));
    }

    #[tokio::test]
    async fn rejects_invalid_type() {
        let dir = TempDir::new().unwrap();
        let tool = SaveMemoryTool::new(dir.path().to_path_buf());
        let result = tool
            .execute(
                &json!({
                    "name": "x",
                    "type": "garbage",
                    "description": "d",
                    "body": "b"
                }),
                &make_context(),
            )
            .await;
        assert!(matches!(result, Err(ToolError::InvalidArguments { .. })));
    }

    #[tokio::test]
    async fn rejects_name_that_slugs_empty() {
        let dir = TempDir::new().unwrap();
        let tool = SaveMemoryTool::new(dir.path().to_path_buf());
        let result = tool
            .execute(
                &json!({
                    "name": "!!!",
                    "type": "user",
                    "description": "d",
                    "body": "b"
                }),
                &make_context(),
            )
            .await;
        assert!(matches!(result, Err(ToolError::InvalidArguments { .. })));
    }

    #[tokio::test]
    async fn updating_same_name_replaces_index_entry() {
        let dir = TempDir::new().unwrap();
        let tool = SaveMemoryTool::new(dir.path().to_path_buf());

        tool.execute(
            &json!({
                "name": "User role",
                "type": "user",
                "description": "first description",
                "body": "first body"
            }),
            &make_context(),
        )
        .await
        .unwrap();

        tool.execute(
            &json!({
                "name": "User role",
                "type": "user",
                "description": "second description",
                "body": "second body"
            }),
            &make_context(),
        )
        .await
        .unwrap();

        let index = fs::read_to_string(dir.path().join(ENTRYPOINT_NAME)).unwrap();
        let occurrences = index.matches("user_role.md").count();
        assert_eq!(occurrences, 1);
        assert!(index.contains("second description"));
        assert!(!index.contains("first description"));
    }
}
