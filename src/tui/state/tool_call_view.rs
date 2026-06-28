use super::active_tool_call::{ActiveToolCall, ToolCallStatus};
use crate::tools::{CategoryPhrasing, ToolRender, phrasing};

/// The set of tool calls executing in the current turn, plus whether the user
/// has expanded a collapsed batch.
#[derive(Default)]
pub struct ToolCallView {
    pub active: Vec<ActiveToolCall>,
    pub expanded: bool,
}

impl ToolCallView {
    pub fn clear(&mut self) {
        self.active.clear();
        self.expanded = false;
    }

    /// True when a batch of 2+ standard tool calls should render as a single
    /// aggregate line. Subagents and any awaiting/errored call opt the whole
    /// batch out so their individual rows and stats are preserved.
    pub fn collapsed(&self) -> bool {
        if self.active.len() < 2 || self.expanded {
            return false;
        }
        self.active.iter().all(|tc| {
            tc.render == ToolRender::Standard
                && !matches!(
                    tc.status,
                    ToolCallStatus::AwaitingApproval | ToolCallStatus::Error(_)
                )
        })
    }
}

const EXPLORATION_PHRASINGS: [CategoryPhrasing; 4] = [
    phrasing::READ,
    phrasing::LIST,
    phrasing::FIND,
    phrasing::SEARCH,
];

pub(crate) fn is_exploration_batch(calls: &[ActiveToolCall]) -> bool {
    !calls.is_empty()
        && calls.iter().all(|tc| {
            matches!(tc.status, ToolCallStatus::Completed)
                && tc.render == ToolRender::Standard
                && EXPLORATION_PHRASINGS.contains(&tc.phrasing)
        })
}
