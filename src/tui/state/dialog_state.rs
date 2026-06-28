use crate::permissions::ToolPermissionDescriptor;

pub struct ToolPermissionDialogState {
    pub descriptor: ToolPermissionDescriptor,
    pub request_id: String,
    pub selected_index: usize,
    pub options: Vec<PermissionOption>,
}

pub struct ApprovalDialogState {
    pub tool_call_id: String,
    pub tool_name: String,
    pub selected_index: usize,
}

impl ApprovalDialogState {
    pub fn new(tool_call_id: String, tool_name: String) -> Self {
        Self {
            tool_call_id,
            tool_name,
            selected_index: 0, // 0 = Approve, 1 = Reject
        }
    }
}

#[derive(Clone)]
pub enum PermissionOption {
    YesOnce,
    No,
    TrustProject(std::path::PathBuf),
}

/// The two modal dialogs the agent loop can raise: tool approval and the
/// richer tool-permission prompt. At most one is shown at a time.
#[derive(Default)]
pub struct DialogState {
    pub approval: Option<ApprovalDialogState>,
    pub permission: Option<ToolPermissionDialogState>,
}
