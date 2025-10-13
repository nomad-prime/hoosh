#[derive(Debug, Clone)]
pub enum AgentEvent {
    Thinking,
    AssistantThought(String),
    ToolCalls(Vec<String>), // Display names for each tool call
    ToolResult { #[allow(dead_code)] tool_name: String, summary: String },
    ToolExecutionComplete,
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
