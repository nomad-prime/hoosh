use super::tool_detail::{BashDetail, SubagentDetail, ToolDetail};
use crate::tools::{CategoryPhrasing, ToolRender};
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct ActiveToolCall {
    pub tool_call_id: String,
    pub display_name: String,
    pub render: ToolRender,
    pub phrasing: CategoryPhrasing,
    pub status: ToolCallStatus,
    pub preview: Option<String>,
    pub result_summary: Option<String>,
    pub subagent: Option<SubagentDetail>,
    pub bash: Option<BashDetail>,
    pub start_time: Instant,
    pub budget_pct: Option<f32>,
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
    pub fn new(
        tool_call_id: String,
        display_name: String,
        render: ToolRender,
        phrasing: CategoryPhrasing,
    ) -> Self {
        Self {
            tool_call_id,
            display_name,
            render,
            phrasing,
            status: ToolCallStatus::Starting,
            preview: None,
            result_summary: None,
            subagent: None,
            bash: None,
            start_time: Instant::now(),
            budget_pct: None,
        }
    }

    pub fn detail(&self) -> Option<&dyn ToolDetail> {
        if let Some(detail) = &self.subagent {
            Some(detail)
        } else {
            self.bash.as_ref().map(|detail| detail as &dyn ToolDetail)
        }
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
