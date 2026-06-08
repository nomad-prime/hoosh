use crate::permissions::ToolPermissionDescriptor;
use crate::tools::todo_write::TodoItem;

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Thinking,
    AssistantThought(String),
    ToolCalls(Vec<(String, String)>),
    ToolPreview {
        preview: String,
    },
    ToolResult {
        tool_call_id: String,
        tool_name: String,
        summary: String,
    },
    ToolExecutionStarted {
        tool_call_id: String,
        tool_name: String,
    },
    ToolExecutionCompleted {
        tool_call_id: String,
        tool_name: String,
    },
    AllToolsComplete,
    FinalResponse(String),
    Error(String),
    MaxStepsReached(usize),
    ToolPermissionRequest {
        descriptor: ToolPermissionDescriptor,
        request_id: String,
    },
    ApprovalRequest {
        tool_call_id: String,
        tool_name: String,
    },
    UserRejection(Vec<String>),
    PermissionDenied(Vec<String>),
    Exit,
    ClearConversation,
    DebugMessage(String),
    RetryEvent {
        operation_name: String,
        attempt: u32,
        max_attempts: u32,
        message: String,
        is_success: bool,
    },
    TokenPressureWarning {
        current_pressure: f32,
        threshold: f32,
    },
    TokenUsage {
        input_tokens: usize,
        output_tokens: usize,
        cost: Option<f64>,
    },
    SubagentStepProgress {
        tool_call_id: String,
        step_number: usize,
        action_type: String,
        description: String,
        timestamp: std::time::SystemTime,
        budget_pct: f32,
    },
    SubagentTaskComplete {
        tool_call_id: String,
        total_steps: usize,
        total_tool_uses: usize,
        total_input_tokens: usize,
        total_output_tokens: usize,
    },
    BashOutputChunk {
        tool_call_id: String,
        output_line: String,
        stream_type: String,
        line_number: usize,
        timestamp: std::time::SystemTime,
    },
    StepStarted {
        step: usize,
    },
    TodoUpdate {
        todos: Vec<TodoItem>,
    },
    /// Request that the event loop swap the active backend and/or model.
    /// Handled on the main task because it needs `&mut EventLoopContext`.
    /// `save = true` also writes the new selection through to the config file.
    SwitchBackend {
        backend: Option<String>,
        model: Option<String>,
        save: bool,
    },
}
