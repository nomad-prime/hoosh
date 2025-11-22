use crate::agent::AgentEvent;
use crate::permissions::{ToolPermissionBuilder, ToolPermissionDescriptor};
use crate::tools::todo_state::TodoState;
use crate::tools::{Tool, ToolError, ToolExecutionContext, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Status of a todo item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

impl std::fmt::Display for TodoStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TodoStatus::Pending => write!(f, "pending"),
            TodoStatus::InProgress => write!(f, "in_progress"),
            TodoStatus::Completed => write!(f, "completed"),
        }
    }
}

/// A single todo item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    /// The content/description of the todo (imperative form, e.g., "Run tests")
    pub content: String,
    /// Current status of the todo
    pub status: TodoStatus,
    /// The active form shown during execution (e.g., "Running tests")
    #[serde(rename = "activeForm")]
    pub active_form: String,
}

impl TodoItem {
    pub fn new(content: String, status: TodoStatus, active_form: String) -> Self {
        Self {
            content,
            status,
            active_form,
        }
    }
}

/// Arguments for the todo_write tool
#[derive(Debug, Deserialize)]
struct TodoWriteArgs {
    todos: Vec<TodoItem>,
}

/// Tool for managing a structured task list during coding sessions
pub struct TodoWriteTool {
    todo_state: TodoState,
}

impl Default for TodoWriteTool {
    fn default() -> Self {
        Self::new(TodoState::new())
    }
}

impl TodoWriteTool {
    pub fn new(todo_state: TodoState) -> Self {
        Self { todo_state }
    }
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &'static str {
        "todo_write"
    }

    fn display_name(&self) -> &'static str {
        "TodoWrite"
    }

    fn description(&self) -> &'static str {
        "Create and manage a structured task list for the current coding session.\n\n\
        This tool helps track progress, organize complex tasks, and provide visibility to the user.\n\n\
        When to use:\n\
        - Complex multi-step tasks requiring 3 or more steps\n\
        - User provides multiple tasks (numbered or comma-separated)\n\
        - After receiving new instructions that need tracking\n\
        - When starting work on a task (mark as in_progress)\n\
        - After completing a task (mark as completed)\n\n\
        When NOT to use:\n\
        - Single, straightforward tasks\n\
        - Trivial tasks that need no tracking\n\
        - Purely conversational or informational requests\n\n\
        Task states:\n\
        - pending: Task not yet started\n\
        - in_progress: Currently working on (limit to ONE at a time)\n\
        - completed: Task finished successfully\n\n\
        Important:\n\
        - Mark tasks complete IMMEDIATELY after finishing\n\
        - Only ONE task should be in_progress at a time\n\
        - Provide both 'content' (imperative: \"Run tests\") and 'activeForm' (continuous: \"Running tests\")"
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "The updated todo list",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": {
                                "type": "string",
                                "minLength": 1,
                                "description": "The task description in imperative form (e.g., 'Run tests', 'Fix bug')"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"],
                                "description": "Current status of the task"
                            },
                            "activeForm": {
                                "type": "string",
                                "minLength": 1,
                                "description": "The task description in present continuous form (e.g., 'Running tests', 'Fixing bug')"
                            }
                        },
                        "required": ["content", "status", "activeForm"]
                    }
                }
            },
            "required": ["todos"]
        })
    }

    async fn execute(&self, args: &Value, context: &ToolExecutionContext) -> ToolResult<String> {
        let args: TodoWriteArgs =
            serde_json::from_value(args.clone()).map_err(|e| ToolError::InvalidArguments {
                tool: "todo_write".to_string(),
                message: format!("Invalid todo_write arguments: {}", e),
            })?;

        // Count in_progress items for validation warning
        let in_progress_count = args
            .todos
            .iter()
            .filter(|t| t.status == TodoStatus::InProgress)
            .count();

        // Update the shared todo state
        self.todo_state.update(args.todos.clone()).await;

        // Send event to update the UI
        if let Some(tx) = &context.event_tx {
            let _ = tx.send(AgentEvent::TodoUpdate {
                todos: args.todos.clone(),
            });
        }

        // Build response message
        let total = args.todos.len();
        let completed = args
            .todos
            .iter()
            .filter(|t| t.status == TodoStatus::Completed)
            .count();
        let pending = args
            .todos
            .iter()
            .filter(|t| t.status == TodoStatus::Pending)
            .count();

        let mut response = format!(
            "Todos have been modified successfully. {} total ({} completed, {} pending, {} in progress).",
            total, completed, pending, in_progress_count
        );

        if in_progress_count > 1 {
            response.push_str("\nNote: Multiple tasks are marked as in_progress. Consider having only one task in progress at a time.");
        }

        response.push_str(" Ensure that you continue to use the todo list to track your progress. Please proceed with the current tasks if applicable");

        Ok(response)
    }

    fn describe_permission(&self, target: Option<&str>) -> ToolPermissionDescriptor {
        ToolPermissionBuilder::new(self, target.unwrap_or("*"))
            .into_read_only() // Todo writing is safe, no file system changes
            .build()
            .expect("Failed to build todo_write permission descriptor")
    }

    fn format_call_display(&self, args: &Value) -> String {
        if let Ok(parsed) = serde_json::from_value::<TodoWriteArgs>(args.clone()) {
            let count = parsed.todos.len();
            let in_progress = parsed
                .todos
                .iter()
                .find(|t| t.status == TodoStatus::InProgress);

            if let Some(current) = in_progress {
                format!(
                    "TodoWrite({} items, current: {})",
                    count, current.active_form
                )
            } else {
                format!("TodoWrite({} items)", count)
            }
        } else {
            "TodoWrite(...)".to_string()
        }
    }

    fn result_summary(&self, _result: &str) -> String {
        "Todo list updated".to_string()
    }

    fn is_hidden(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_write_tool_name() {
        let tool = TodoWriteTool::new(TodoState::new());
        assert_eq!(tool.name(), "todo_write");
    }

    #[test]
    fn test_todo_write_tool_display_name() {
        let tool = TodoWriteTool::new(TodoState::new());
        assert_eq!(tool.display_name(), "TodoWrite");
    }

    #[test]
    fn test_todo_status_serialization() {
        assert_eq!(
            serde_json::to_string(&TodoStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&TodoStatus::InProgress).unwrap(),
            "\"in_progress\""
        );
        assert_eq!(
            serde_json::to_string(&TodoStatus::Completed).unwrap(),
            "\"completed\""
        );
    }

    #[test]
    fn test_todo_item_deserialization() {
        let json = r#"{
            "content": "Run tests",
            "status": "in_progress",
            "activeForm": "Running tests"
        }"#;

        let item: TodoItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.content, "Run tests");
        assert_eq!(item.status, TodoStatus::InProgress);
        assert_eq!(item.active_form, "Running tests");
    }

    #[test]
    fn test_parameter_schema() {
        let tool = TodoWriteTool::new(TodoState::new());
        let schema = tool.parameter_schema();

        assert_eq!(schema.get("type").and_then(|v| v.as_str()), Some("object"));
        let props = schema.get("properties");
        assert!(props.is_some());
        assert!(props.unwrap().get("todos").is_some());
    }

    #[test]
    fn test_format_call_display() {
        let tool = TodoWriteTool::new(TodoState::new());
        let args = json!({
            "todos": [
                {
                    "content": "Run tests",
                    "status": "in_progress",
                    "activeForm": "Running tests"
                },
                {
                    "content": "Fix bugs",
                    "status": "pending",
                    "activeForm": "Fixing bugs"
                }
            ]
        });

        let display = tool.format_call_display(&args);
        assert!(display.contains("2 items"));
        assert!(display.contains("Running tests"));
    }

    #[test]
    fn test_format_call_display_no_in_progress() {
        let tool = TodoWriteTool::new(TodoState::new());
        let args = json!({
            "todos": [
                {
                    "content": "Task 1",
                    "status": "pending",
                    "activeForm": "Doing task 1"
                }
            ]
        });

        let display = tool.format_call_display(&args);
        assert!(display.contains("1 items"));
        assert!(!display.contains("current:"));
    }

    #[tokio::test]
    async fn test_execute_success() {
        let tool = TodoWriteTool::new(TodoState::new());
        let args = json!({
            "todos": [
                {
                    "content": "Run tests",
                    "status": "in_progress",
                    "activeForm": "Running tests"
                }
            ]
        });

        let context = ToolExecutionContext {
            tool_call_id: "test".to_string(),
            event_tx: None,
            parent_conversation_id: None,
        };

        let result = tool.execute(&args, &context).await;
        assert!(result.is_ok());
        let result_str = result.unwrap();
        assert!(result_str.contains("Todos have been modified successfully"));
    }

    #[tokio::test]
    async fn test_execute_empty_todos() {
        let tool = TodoWriteTool::new(TodoState::new());
        let args = json!({
            "todos": []
        });

        let context = ToolExecutionContext {
            tool_call_id: "test".to_string(),
            event_tx: None,
            parent_conversation_id: None,
        };

        let result = tool.execute(&args, &context).await;
        assert!(result.is_ok()); // Empty todos now allowed - just clears the list
    }

    #[test]
    fn test_permission_descriptor() {
        let tool = TodoWriteTool::new(TodoState::new());
        let perm = tool.describe_permission(Some("*"));

        assert_eq!(perm.kind(), "todo_write");
        assert!(perm.is_read_only());
        assert!(!perm.is_destructive());
    }
}
