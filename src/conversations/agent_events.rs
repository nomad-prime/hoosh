use crate::permissions::ToolPermissionDescriptor;

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Thinking,
    AssistantThought(String),
    ToolCalls(Vec<String>),
    ToolPreview {
        tool_name: String,
        preview: String,
    },
    ToolResult {
        #[allow(dead_code)]
        tool_name: String,
        summary: String,
    },
    ToolExecutionComplete,
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
    UserRejection,
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
    AgentSwitched {
        new_agent_name: String,
    },
    ContextCompressionTriggered {
        original_message_count: usize,
        compressed_message_count: usize,
        token_pressure: f32,
    },
    ContextCompressionComplete {
        summary_length: usize,
    },
    ContextCompressionError {
        error: String,
    },
    TokenPressureWarning {
        current_pressure: f32,
        threshold: f32,
    },
    Summarizing {
        message_count: usize,
    },
    SummaryComplete {
        message_count: usize,
        summary: String,
    },
    SummaryError {
        error: String,
    },
    TokenUsage {
        input_tokens: usize,
        output_tokens: usize,
        cost: Option<f64>,
    },
}
