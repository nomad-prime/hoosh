use std::sync::Arc;
use tokio::sync::RwLock;

use super::todo_write::{TodoItem, TodoStatus};

/// Shared todo state that can be accessed across the application
#[derive(Clone)]
pub struct TodoState {
    todos: Arc<RwLock<Vec<TodoItem>>>,
}

impl Default for TodoState {
    fn default() -> Self {
        Self::new()
    }
}

impl TodoState {
    pub fn new() -> Self {
        Self {
            todos: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Update the todo list
    pub async fn update(&self, todos: Vec<TodoItem>) {
        let mut state = self.todos.write().await;
        *state = todos;
    }

    /// Get a copy of the current todos
    pub async fn get(&self) -> Vec<TodoItem> {
        let state = self.todos.read().await;
        state.clone()
    }

    /// Check if there are any todos
    pub async fn is_empty(&self) -> bool {
        let state = self.todos.read().await;
        state.is_empty()
    }

    /// Check if all todos are completed
    pub async fn all_completed(&self) -> bool {
        let todos = self.get().await;
        !todos.is_empty() && todos.iter().all(|t| t.status == TodoStatus::Completed)
    }

    /// Clear the todo list
    pub async fn clear(&self) {
        let mut state = self.todos.write().await;
        state.clear();
    }

    /// Format the current todo state as a system reminder for the LLM
    pub async fn format_for_llm(&self) -> Option<String> {
        let todos = self.get().await;

        if todos.is_empty() {
            return Some(
                "<system-reminder>\n\
                This is a reminder that your todo list is currently empty. DO NOT mention this to the user explicitly because they are already aware. \
                If you are working on tasks that would benefit from a todo list please use the todo_write tool to create one. \
                If not, please feel free to ignore. Again do not mention this message to the user.\n\
                </system-reminder>".to_string()
            );
        }

        // Check if all todos are completed
        let all_completed = todos.iter().all(|t| t.status == TodoStatus::Completed);

        let mut lines = Vec::new();
        lines.push("<system-reminder>".to_string());

        if all_completed {
            lines.push("All tasks in your todo list have been completed! You can now clear the todo list by calling todo_write with an empty array, \
or leave it as-is if you want to keep the record. DO NOT mention this reminder to the user.".to_string());
        } else {
            lines.push("The todo_write tool hasn't been used recently. If you're working on tasks that would benefit from tracking progress, \
consider using the todo_write tool to track progress. Also consider cleaning up the todo list if has become stale and no longer matches \
what you are working on. Only use it if it's relevant to the current work. This is just a gentle reminder - ignore if not applicable. \
Make sure that you NEVER mention this reminder to the user".to_string());
        }

        lines.push(String::new());
        lines.push(String::new());
        lines.push("Here are the existing contents of your todo list:".to_string());
        lines.push(String::new());

        lines.push("[".to_string());
        for (index, todo) in todos.iter().enumerate() {
            let status_str = match todo.status {
                TodoStatus::Pending => "pending",
                TodoStatus::InProgress => "in_progress",
                TodoStatus::Completed => "completed",
            };
            lines.push(format!("{}. [{}] {}", index + 1, status_str, todo.content));
        }
        lines.push("]".to_string());
        lines.push("</system-reminder>".to_string());

        Some(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_todo_state_new_is_empty() {
        let state = TodoState::new();
        assert!(state.is_empty().await);
    }

    #[tokio::test]
    async fn test_todo_state_update_and_get() {
        let state = TodoState::new();
        let todos = vec![TodoItem::new(
            "Test task".to_string(),
            TodoStatus::Pending,
            "Testing task".to_string(),
        )];

        state.update(todos.clone()).await;

        let retrieved = state.get().await;
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].content, "Test task");
    }

    #[tokio::test]
    async fn test_format_for_llm_empty() {
        let state = TodoState::new();
        let formatted = state.format_for_llm().await;

        assert!(formatted.is_some());
        let text = formatted.unwrap();
        assert!(text.contains("todo list is currently empty"));
    }

    #[tokio::test]
    async fn test_format_for_llm_with_todos() {
        let state = TodoState::new();
        let todos = vec![
            TodoItem::new(
                "First task".to_string(),
                TodoStatus::Completed,
                "Doing first task".to_string(),
            ),
            TodoItem::new(
                "Second task".to_string(),
                TodoStatus::InProgress,
                "Doing second task".to_string(),
            ),
        ];

        state.update(todos).await;
        let formatted = state.format_for_llm().await;

        assert!(formatted.is_some());
        let text = formatted.unwrap();
        assert!(text.contains("[completed]"));
        assert!(text.contains("[in_progress]"));
        assert!(text.contains("First task"));
        assert!(text.contains("Second task"));
    }

    #[tokio::test]
    async fn test_all_completed() {
        let state = TodoState::new();

        // Empty list is not "all completed"
        assert!(!state.all_completed().await);

        // Mixed status
        let todos = vec![
            TodoItem::new(
                "First task".to_string(),
                TodoStatus::Completed,
                "Doing first task".to_string(),
            ),
            TodoItem::new(
                "Second task".to_string(),
                TodoStatus::Pending,
                "Doing second task".to_string(),
            ),
        ];
        state.update(todos).await;
        assert!(!state.all_completed().await);

        // All completed
        let todos = vec![
            TodoItem::new(
                "First task".to_string(),
                TodoStatus::Completed,
                "Doing first task".to_string(),
            ),
            TodoItem::new(
                "Second task".to_string(),
                TodoStatus::Completed,
                "Doing second task".to_string(),
            ),
        ];
        state.update(todos).await;
        assert!(state.all_completed().await);
    }

    #[tokio::test]
    async fn test_format_for_llm_all_completed() {
        let state = TodoState::new();
        let todos = vec![
            TodoItem::new(
                "First task".to_string(),
                TodoStatus::Completed,
                "Doing first task".to_string(),
            ),
            TodoItem::new(
                "Second task".to_string(),
                TodoStatus::Completed,
                "Doing second task".to_string(),
            ),
        ];

        state.update(todos).await;
        let formatted = state.format_for_llm().await;

        assert!(formatted.is_some());
        let text = formatted.unwrap();
        assert!(text.contains("All tasks in your todo list have been completed"));
        assert!(text.contains("clear the todo list"));
    }

    #[tokio::test]
    async fn test_clear() {
        let state = TodoState::new();
        let todos = vec![TodoItem::new(
            "Task".to_string(),
            TodoStatus::Pending,
            "Doing task".to_string(),
        )];

        state.update(todos).await;
        assert!(!state.is_empty().await);

        state.clear().await;
        assert!(state.is_empty().await);
    }
}
