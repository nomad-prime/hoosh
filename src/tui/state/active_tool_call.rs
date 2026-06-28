use crate::tools::{CategoryPhrasing, ToolRender};
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct SubagentStepSummary {
    pub step_number: usize,
    pub action_type: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct BashOutputLine {
    pub line_number: usize,
    pub content: String,
    pub stream_type: String, // "stdout" or "stderr"
}

#[derive(Clone, Debug)]
pub struct ActiveToolCall {
    pub tool_call_id: String,
    pub display_name: String,
    pub render: ToolRender,
    pub phrasing: CategoryPhrasing,
    pub status: ToolCallStatus,
    pub preview: Option<String>,
    pub result_summary: Option<String>,
    pub subagent_steps: Vec<SubagentStepSummary>,
    pub is_subagent_task: bool,
    pub bash_output_lines: Vec<BashOutputLine>,
    pub is_bash_streaming: bool,
    pub start_time: Instant,
    pub budget_pct: Option<f32>,
    pub total_tool_uses: Option<usize>,
    pub total_tokens: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ToolCallStatus {
    Starting,
    AwaitingApproval,
    Executing,
    Completed,
    Error(String),
}

impl ActiveToolCall {
    pub fn add_subagent_step(&mut self, step: SubagentStepSummary) {
        self.subagent_steps.push(step);
    }

    pub fn add_bash_output_line(&mut self, line: BashOutputLine) {
        self.bash_output_lines.push(line);
        self.is_bash_streaming = true;
    }

    pub fn elapsed_time(&self) -> String {
        let elapsed = self.start_time.elapsed();
        let total_secs = elapsed.as_secs();

        if total_secs < 60 {
            format!("{}s", total_secs)
        } else {
            let mins = total_secs / 60;
            let secs = total_secs % 60;
            format!("{}m{}s", mins, secs)
        }
    }
}
