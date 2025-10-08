use crate::conversations::ToolCall;

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Thinking,
    AssistantThought(String),
    ToolCalls(Vec<ToolCall>),
    ToolResult { #[allow(dead_code)] tool_name: String, summary: String },
    FinalResponse(String),
    Error(String),
    MaxStepsReached(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Idle,
    Thinking,
    ExecutingTools,
}
