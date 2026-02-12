use super::clipboard::ClipboardManager;
use super::events::AgentState;
use super::input::{PasteDetector, TextArea, TextAttachment};
use crate::agent::AgentEvent;
use crate::completion::Completer;
use crate::history::PromptHistory;
use crate::permissions::ToolPermissionDescriptor;
use crate::tools::todo_write::{TodoItem, TodoStatus};
use crate::tui::palette;
use anyhow::Result;
use rand::Rng;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ScrollbarState;
use std::collections::VecDeque;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    Normal,
    Expanded,
    AttachmentList,
    AttachmentView,
}

#[derive(Clone)]
pub enum MessageLine {
    Plain(String),
    Styled(Line<'static>),
    Markdown(String),
}

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

pub struct CompletionState {
    pub candidates: Vec<String>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub query: String,
    pub completer_index: usize,
}

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

impl CompletionState {
    pub fn new(completer_index: usize) -> Self {
        Self {
            candidates: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            query: String::new(),
            completer_index,
        }
    }

    pub fn selected_item(&self) -> Option<&str> {
        self.candidates.get(self.selected_index).map(|s| s.as_str())
    }

    pub fn select_next(&mut self) {
        if !self.candidates.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.candidates.len();
            self.update_scroll_offset(10);
        }
    }

    pub fn select_prev(&mut self) {
        if !self.candidates.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.candidates.len() - 1;
            } else {
                self.selected_index -= 1;
            }
            self.update_scroll_offset(10);
        }
    }

    fn update_scroll_offset(&mut self, visible_items: usize) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + visible_items {
            self.scroll_offset = self.selected_index.saturating_sub(visible_items - 1);
        }
    }
}

pub struct AttachmentViewState {
    pub attachment_id: usize,
    pub editor: TextArea,
    pub is_modified: bool,
}

pub struct AppState {
    pub input: TextArea,
    pub messages: VecDeque<MessageLine>,
    pub pending_messages: VecDeque<MessageLine>,
    pub agent_state: AgentState,
    pub should_quit: bool,
    pub should_cancel_task: bool,
    pub max_messages: usize,
    pub completion_state: Option<CompletionState>,
    pub completers: Vec<Box<dyn Completer>>,
    pub tool_permission_dialog_state: Option<ToolPermissionDialogState>,
    pub approval_dialog_state: Option<ApprovalDialogState>,
    pub autopilot_enabled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub animation_frame: usize,
    pub prompt_history: PromptHistory,
    pub current_thinking_spinner: usize,
    pub current_executing_spinner: usize,
    pub clipboard: ClipboardManager,
    pub current_retry_status: Option<String>,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_cost: f64,
    pub active_tool_calls: Vec<ActiveToolCall>,
    pub todos: Vec<TodoItem>,
    pub vertical_scroll: usize,
    pub vertical_scroll_state: ScrollbarState,
    pub vertical_scroll_content_length: usize,
    pub vertical_scroll_viewport_length: usize,
    pub attachments: Vec<TextAttachment>,
    pub next_attachment_id: usize,
    pub input_mode: InputMode,
    pub attachment_view: Option<AttachmentViewState>,
    pub paste_detector: PasteDetector,
}

impl AppState {
    pub fn new() -> Self {
        let mut input = TextArea::default();
        input.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        input.set_cursor_line_style(Style::default());

        // Initialize with random spinner indices
        let mut rng = rand::thread_rng();
        let current_thinking_spinner = rng.gen_range(0..7);
        let current_executing_spinner = rng.gen_range(0..7);

        Self {
            input,
            messages: VecDeque::new(),
            pending_messages: VecDeque::new(),
            agent_state: AgentState::Idle,
            should_quit: false,
            should_cancel_task: false,
            max_messages: 100_000,
            completion_state: None,
            completers: Vec::new(),
            tool_permission_dialog_state: None,
            approval_dialog_state: None,
            autopilot_enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            animation_frame: 0,
            prompt_history: PromptHistory::new(1000),
            current_thinking_spinner,
            current_executing_spinner,
            clipboard: ClipboardManager::new(),
            current_retry_status: None,
            input_tokens: 0,
            output_tokens: 0,
            total_cost: 0.0,
            active_tool_calls: Vec::new(),
            todos: Vec::new(),
            vertical_scroll: 0,
            vertical_scroll_state: ScrollbarState::default(),
            vertical_scroll_content_length: 0,
            vertical_scroll_viewport_length: 0,
            attachments: Vec::new(),
            next_attachment_id: 1,
            input_mode: InputMode::Normal,
            attachment_view: None,
            paste_detector: PasteDetector::new(),
        }
    }

    pub fn tick_animation(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);
    }

    pub fn register_completer(&mut self, completer: Box<dyn Completer>) {
        self.completers.push(completer);
    }

    pub fn find_completer_for_key(&self, key: char) -> Option<usize> {
        self.completers.iter().position(|c| c.trigger_key() == key)
    }

    pub fn is_completing(&self) -> bool {
        self.completion_state.is_some()
    }

    pub fn is_showing_tool_permission_dialog(&self) -> bool {
        self.tool_permission_dialog_state.is_some()
    }

    pub fn is_showing_approval_dialog(&self) -> bool {
        self.approval_dialog_state.is_some()
    }

    pub fn toggle_autopilot(&mut self) {
        let current = self
            .autopilot_enabled
            .load(std::sync::atomic::Ordering::Relaxed);
        self.autopilot_enabled
            .store(!current, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn show_approval_dialog(&mut self, tool_call_id: String, tool_name: String) {
        self.approval_dialog_state = Some(ApprovalDialogState::new(tool_call_id, tool_name));
    }

    pub fn hide_approval_dialog(&mut self) {
        self.approval_dialog_state = None;
    }

    pub fn select_next_approval_option(&mut self) {
        if let Some(dialog) = &mut self.approval_dialog_state {
            dialog.selected_index = (dialog.selected_index + 1) % 2; // 0 = Approve, 1 = Reject
        }
    }

    pub fn select_prev_approval_option(&mut self) {
        if let Some(dialog) = &mut self.approval_dialog_state {
            dialog.selected_index = (dialog.selected_index + 1) % 2; // Same as next for 2 options
        }
    }

    pub fn show_tool_permission_dialog(
        &mut self,
        descriptor: ToolPermissionDescriptor,
        request_id: String,
    ) {
        let options = if let Ok(current_dir) = std::env::current_dir() {
            vec![
                PermissionOption::YesOnce,
                PermissionOption::TrustProject(current_dir),
                PermissionOption::No,
            ]
        } else {
            vec![PermissionOption::YesOnce, PermissionOption::No]
        };

        self.tool_permission_dialog_state = Some(ToolPermissionDialogState {
            descriptor,
            request_id,
            selected_index: 0,
            options,
        });
    }

    pub fn select_next_tool_permission_option(&mut self) {
        if let Some(dialog) = &mut self.tool_permission_dialog_state
            && !dialog.options.is_empty()
        {
            dialog.selected_index = (dialog.selected_index + 1) % dialog.options.len();
        }
    }

    pub fn select_prev_tool_permission_option(&mut self) {
        if let Some(dialog) = &mut self.tool_permission_dialog_state
            && !dialog.options.is_empty()
        {
            if dialog.selected_index == 0 {
                dialog.selected_index = dialog.options.len() - 1;
            } else {
                dialog.selected_index -= 1;
            }
        }
    }

    pub fn hide_tool_permission_dialog(&mut self) {
        self.tool_permission_dialog_state = None;
    }

    pub fn start_completion(&mut self, completer_index: usize) {
        self.completion_state = Some(CompletionState::new(completer_index));
    }

    pub fn cancel_completion(&mut self) {
        self.completion_state = None;
    }

    pub fn update_completion_query(&mut self, query: String) {
        if let Some(state) = &mut self.completion_state {
            state.query = query;
            state.selected_index = 0;
        }
    }

    pub fn set_completion_candidates(&mut self, candidates: Vec<String>) {
        if let Some(state) = &mut self.completion_state {
            state.candidates = candidates;
            state.selected_index = 0;
            state.scroll_offset = 0;
        }
    }

    pub fn select_next_completion(&mut self) {
        if let Some(state) = &mut self.completion_state {
            state.select_next();
        }
    }

    pub fn select_prev_completion(&mut self) {
        if let Some(state) = &mut self.completion_state {
            state.select_prev();
        }
    }

    pub fn apply_completion(&mut self) -> Option<String> {
        if let Some(state) = &self.completion_state {
            let selected = state.selected_item()?.to_string();
            self.completion_state = None;
            Some(selected)
        } else {
            None
        }
    }

    pub fn add_message(&mut self, message: String) {
        let msg_line = MessageLine::Plain(message);
        self.add_message_line(msg_line);
    }

    pub fn add_debug_message(&mut self, message: String) {
        let styled_line = Line::from(Span::styled(
            format!("  [DEBUG] {}", message),
            Style::default()
                .fg(palette::WARNING)
                .add_modifier(Modifier::ITALIC),
        ));
        self.add_styled_line(styled_line);
    }

    pub fn add_styled_line(&mut self, line: Line<'static>) {
        let msg_line = MessageLine::Styled(line);
        self.add_message_line(msg_line);
    }

    pub fn has_pending_messages(&self) -> bool {
        !self.pending_messages.is_empty()
    }

    pub fn drain_pending_messages(&mut self) -> Vec<MessageLine> {
        self.pending_messages.drain(..).collect()
    }

    pub fn add_active_tool_call(&mut self, tool_call_id: String, display_name: String) {
        self.active_tool_calls.push(ActiveToolCall {
            tool_call_id,
            display_name,
            status: ToolCallStatus::Starting,
            preview: None,
            result_summary: None,
            subagent_steps: Vec::new(),
            is_subagent_task: false,
            bash_output_lines: Vec::new(),
            is_bash_streaming: false,
            start_time: Instant::now(),
            budget_pct: None,
            total_tool_uses: None,
            total_tokens: None,
        });
    }

    pub fn update_tool_call_status(&mut self, tool_call_id: &str, status: ToolCallStatus) {
        if let Some(tool_call) = self.get_active_tool_call_mut(tool_call_id) {
            tool_call.status = status;
        }
    }

    pub fn set_tool_call_result(&mut self, tool_call_id: &str, summary: String) {
        if let Some(tool_call) = self.get_active_tool_call_mut(tool_call_id) {
            tool_call.result_summary = Some(summary);
        }
    }

    pub fn get_active_tool_call_mut(&mut self, tool_call_id: &str) -> Option<&mut ActiveToolCall> {
        self.active_tool_calls
            .iter_mut()
            .find(|tc| tc.tool_call_id == tool_call_id)
    }

    pub fn complete_active_tool_calls(&mut self) {
        let tool_calls = self.active_tool_calls.clone();

        for tool_call in &tool_calls {
            self.add_message(format!("\n● {}", tool_call.display_name));

            if let Some(summary) = &tool_call.result_summary {
                self.add_message(format!("  ⎿  {}", summary));
            }

            // Preview is now displayed immediately when ToolPreview event is received,
            // so we don't display it again here

            if let ToolCallStatus::Error(err) = &tool_call.status {
                self.add_error(err);
            }
        }

        self.active_tool_calls.clear();
    }

    pub fn complete_single_tool_call(&mut self, tool_call_id: &str) {
        if let Some(index) = self
            .active_tool_calls
            .iter()
            .position(|tc| tc.tool_call_id == tool_call_id)
        {
            let tool_call = self.active_tool_calls.remove(index);

            self.add_message(format!("\n● {}", tool_call.display_name));

            // For subagent tasks, show completion stats
            if tool_call.is_subagent_task {
                if let (Some(tool_uses), Some(tokens)) =
                    (tool_call.total_tool_uses, tool_call.total_tokens)
                {
                    let tokens_formatted = if tokens >= 1000 {
                        format!("{:.1}k", tokens as f64 / 1000.0)
                    } else {
                        tokens.to_string()
                    };

                    let completion_text = format!(
                        "Done ({} tool uses · {} tokens · {})",
                        tool_uses,
                        tokens_formatted,
                        tool_call.elapsed_time()
                    );
                    self.add_message(format!("  ⎿ {}", completion_text));
                }
            } else if let Some(summary) = &tool_call.result_summary {
                // For regular tools, show the result summary
                self.add_message(format!("  ⎿  {}", summary));
            }

            if let ToolCallStatus::Error(err) = &tool_call.status {
                self.add_error(err);
            }
        }
    }

    pub fn clear_active_tool_calls(&mut self) {
        self.active_tool_calls.clear();
    }

    pub fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Thinking => {
                self.agent_state = AgentState::Thinking;
                let mut rng = rand::thread_rng();
                self.current_thinking_spinner = rng.gen_range(0..7);
            }
            AgentEvent::AssistantThought(content) => {
                self.add_thought(&content);
            }
            AgentEvent::ToolCalls(tool_call_info) => {
                self.agent_state = AgentState::ExecutingTools;
                let mut rng = rand::thread_rng();
                self.current_executing_spinner = rng.gen_range(0..7);
                for (tool_call_id, display_name) in tool_call_info {
                    self.add_active_tool_call(tool_call_id, display_name);
                }
            }
            AgentEvent::ToolExecutionStarted { tool_call_id, .. } => {
                self.update_tool_call_status(&tool_call_id, ToolCallStatus::Executing);
            }
            AgentEvent::ToolPreview { preview, .. } => {
                self.add_message(format!("\n{}", preview));
            }
            AgentEvent::ToolResult {
                tool_call_id,
                summary,
                ..
            } => {
                self.set_tool_call_result(&tool_call_id, summary);
            }
            AgentEvent::ToolExecutionCompleted { tool_call_id, .. } => {
                self.update_tool_call_status(&tool_call_id, ToolCallStatus::Completed);
            }
            AgentEvent::AllToolsComplete => {
                self.complete_active_tool_calls();
                self.agent_state = AgentState::Thinking;
            }
            AgentEvent::FinalResponse(content) => {
                self.agent_state = AgentState::Idle;
                self.add_final_response(&content);
            }
            AgentEvent::Error(error) => {
                self.agent_state = AgentState::Idle;
                self.add_error(&error);
            }
            AgentEvent::MaxStepsReached(max_steps) => {
                self.agent_state = AgentState::Idle;
                self.add_message(format!(
                    "   Maximum conversation steps ({}) reached, stopping.",
                    max_steps
                ));
            }
            AgentEvent::ToolPermissionRequest { .. } => {}
            AgentEvent::ApprovalRequest { .. } => {}
            AgentEvent::UserRejection(rejected_tool_calls) => {
                for rtc in &rejected_tool_calls {
                    self.add_tool_call(rtc);
                    self.add_status_message("Rejected, tell me what to do instead");
                }
                self.clear_active_tool_calls();

                self.agent_state = AgentState::Idle;
            }
            AgentEvent::PermissionDenied(rejected_tool_calls) => {
                for rtc in &rejected_tool_calls {
                    self.add_tool_call(rtc);
                    self.add_status_message("Permission denied, tell me what to do instead");
                }
                self.clear_active_tool_calls();

                self.agent_state = AgentState::Idle;
            }
            AgentEvent::Exit => {}
            AgentEvent::ClearConversation => {}
            AgentEvent::DebugMessage(_) => {}
            AgentEvent::RetryEvent {
                message,
                is_success,
                attempt,
                max_attempts,
                ..
            } => {
                if is_success {
                    self.current_retry_status = None;
                    self.add_status_message(&message);
                } else if attempt < max_attempts {
                    self.current_retry_status = Some(message.clone());
                } else {
                    self.current_retry_status = None;
                    self.agent_state = AgentState::Idle;
                    self.add_retry_failure(&message);
                }
            }
            AgentEvent::Summarizing { .. } => {
                self.agent_state = AgentState::Summarizing;
            }
            AgentEvent::SummaryComplete { .. } => {
                self.agent_state = AgentState::Idle;
            }
            AgentEvent::SummaryError { error } => {
                self.agent_state = AgentState::Idle;
                self.add_message(format!("Error summarizing conversation: {}", error));
            }
            AgentEvent::TokenPressureWarning {
                current_pressure,
                threshold,
            } => {
                if current_pressure > threshold {
                    self.add_status_message(&format!(
                        "High token pressure: {:.0}% (threshold: {:.0}%)",
                        current_pressure * 100.0,
                        threshold * 100.0
                    ));
                }
            }
            AgentEvent::TokenUsage {
                input_tokens,
                output_tokens,
                cost,
            } => {
                self.input_tokens = input_tokens;
                self.output_tokens = output_tokens;
                if let Some(call_cost) = cost {
                    self.total_cost += call_cost;
                }
            }
            AgentEvent::SubagentStepProgress {
                tool_call_id,
                step_number,
                action_type,
                description,
                budget_pct,
                ..
            } => {
                if let Some(tool_call) = self.get_active_tool_call_mut(&tool_call_id) {
                    tool_call.is_subagent_task = true;
                    let step = SubagentStepSummary {
                        step_number,
                        action_type,
                        description,
                    };
                    tool_call.budget_pct = Some(budget_pct);
                    tool_call.add_subagent_step(step);
                }
            }
            AgentEvent::SubagentTaskComplete {
                tool_call_id,
                total_tool_uses,
                total_input_tokens,
                total_output_tokens,
                ..
            } => {
                if let Some(tool_call) = self.get_active_tool_call_mut(&tool_call_id) {
                    tool_call.total_tool_uses = Some(total_tool_uses);
                    tool_call.total_tokens = Some(total_input_tokens + total_output_tokens);
                }
            }
            AgentEvent::BashOutputChunk {
                tool_call_id,
                output_line,
                stream_type,
                line_number,
                ..
            } => {
                if let Some(tool_call) = self.get_active_tool_call_mut(&tool_call_id) {
                    let bash_line = BashOutputLine {
                        line_number,
                        content: output_line,
                        stream_type,
                    };
                    tool_call.add_bash_output_line(bash_line);
                }
            }
            AgentEvent::StepStarted { .. } => {
                // This event is used internally for step tracking, no UI update needed
            }
            AgentEvent::TodoUpdate { todos } => {
                // If all todos are completed, auto-clear after updating
                let all_completed =
                    !todos.is_empty() && todos.iter().all(|t| t.status == TodoStatus::Completed);

                if all_completed {
                    // Clear the list when all items are done
                    self.todos = Vec::new();
                } else {
                    self.todos = todos;
                }
            }
        }
    }

    pub fn add_thought(&mut self, content: &str) {
        if !content.is_empty() {
            let msg_line = MessageLine::Markdown(content.to_string());

            self.add_message("\n".to_string());
            self.add_message_line(msg_line);
        }
    }

    pub fn add_message_line(&mut self, msg_line: MessageLine) {
        self.messages.push_back(msg_line.clone());
        if self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
        self.pending_messages.push_back(msg_line);
    }

    pub fn add_tool_call(&mut self, name: &str) {
        self.add_message(format!("\n● {}", name));
    }

    pub fn add_status_message(&mut self, message: &str) {
        self.add_message(format!("  ⎿  {}", message));
    }

    pub fn add_error(&mut self, error: &str) {
        let styled_line = Line::from(Span::styled(
            format!("  ⎿  {}", error),
            Style::default()
                .fg(palette::DESTRUCTIVE)
                .add_modifier(Modifier::ITALIC),
        ));
        self.add_styled_line(styled_line);
    }

    pub fn add_final_response(&mut self, content: &str) {
        // Add blank line before response
        self.add_message("\n".to_string());

        let msg_line = MessageLine::Markdown(content.to_string());
        self.add_message_line(msg_line);
    }

    pub fn add_user_input(&mut self, input: &str) {
        self.add_message(format!("\n> {}", input));
    }

    pub fn add_tool_preview(&mut self, preview: &str) {
        self.add_message(format!("\n{}", preview));
    }

    pub fn add_retry_failure(&mut self, message: &str) {
        let styled_line = Line::from(Span::styled(
            format!("  ⎿  {}", message),
            Style::default()
                .fg(palette::DESTRUCTIVE)
                .add_modifier(Modifier::ITALIC),
        ));
        self.add_styled_line(styled_line);
    }

    pub fn get_input_text(&self) -> String {
        self.input.lines().join("\n")
    }

    pub fn clear_input(&mut self) {
        self.input = TextArea::default();
        self.input
            .set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        // Remove the underline from the cursor line
        self.input.set_cursor_line_style(Style::default());
    }

    pub fn create_attachment(&mut self, content: String) -> Result<usize> {
        let size_bytes = content.len();

        if size_bytes > 5_000_000 {
            anyhow::bail!("Paste rejected: exceeds 5MB limit");
        }

        if content.chars().count() <= 200 {
            anyhow::bail!("Content too small for attachment (must be >200 chars)");
        }

        let id = self.next_attachment_id;
        self.next_attachment_id += 1;

        let attachment = TextAttachment::new(id, content);
        self.attachments.push(attachment);

        Ok(id)
    }

    pub fn delete_attachment(&mut self, id: usize) -> Result<()> {
        let index = self
            .attachments
            .iter()
            .position(|a| a.id == id)
            .ok_or_else(|| anyhow::anyhow!("Attachment not found: {}", id))?;

        self.attachments.remove(index);
        Ok(())
    }

    pub fn clear_attachments(&mut self) {
        self.attachments.clear();
        self.next_attachment_id = 1;
    }

    pub fn get_attachment(&self, id: usize) -> Option<&TextAttachment> {
        self.attachments.iter().find(|a| a.id == id)
    }

    pub fn get_attachment_mut(&mut self, id: usize) -> Option<&mut TextAttachment> {
        self.attachments.iter_mut().find(|a| a.id == id)
    }

    pub fn expand_attachments(&self, input: &str) -> String {
        let mut expanded = input.to_string();
        for attachment in &self.attachments {
            let token = format!("[pasted text-{}]", attachment.id);
            expanded = expanded.replace(&token, &attachment.content);
        }
        expanded
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "app_state_tests.rs"]
mod tests;

#[cfg(test)]
mod attachment_tests {
    use super::*;

    #[test]
    fn test_create_attachment_success() {
        let mut state = AppState::new();
        let content = "a".repeat(201);

        let result = state.create_attachment(content.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
        assert_eq!(state.attachments.len(), 1);
        assert_eq!(state.next_attachment_id, 2);
    }

    #[test]
    fn test_create_attachment_sequential_ids() {
        let mut state = AppState::new();

        let id1 = state.create_attachment("a".repeat(201)).unwrap();
        let id2 = state.create_attachment("b".repeat(201)).unwrap();
        let id3 = state.create_attachment("c".repeat(201)).unwrap();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
        assert_eq!(state.attachments.len(), 3);
        assert_eq!(state.next_attachment_id, 4);
    }

    #[test]
    fn test_create_attachment_rejects_too_small() {
        let mut state = AppState::new();
        let small_content = "a".repeat(200);

        let result = state.create_attachment(small_content);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("too small for attachment"));
        assert_eq!(state.attachments.len(), 0);
    }

    #[test]
    fn test_create_attachment_rejects_exceeds_5mb() {
        let mut state = AppState::new();
        let huge_content = "a".repeat(5_000_001);

        let result = state.create_attachment(huge_content);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds 5MB"));
        assert_eq!(state.attachments.len(), 0);
    }

    #[test]
    fn test_create_attachment_calculates_size_chars() {
        let mut state = AppState::new();
        let content = "🦀".repeat(300);

        let id = state.create_attachment(content.clone()).unwrap();
        let attachment = state.get_attachment(id).unwrap();

        assert_eq!(attachment.size_chars, 300);
        assert!(attachment.content.len() > 300);
    }

    #[test]
    fn test_create_attachment_calculates_line_count() {
        let mut state = AppState::new();
        let content = "line1\nline2\nline3\n".to_string() + &"a".repeat(200);

        let id = state.create_attachment(content.clone()).unwrap();
        let attachment = state.get_attachment(id).unwrap();

        assert_eq!(attachment.line_count, 4);
    }

    #[test]
    fn test_create_attachment_line_count_minimum_one() {
        let mut state = AppState::new();
        let content = "a".repeat(201);

        let id = state.create_attachment(content).unwrap();
        let attachment = state.get_attachment(id).unwrap();

        assert_eq!(attachment.line_count, 1);
    }

    #[test]
    fn test_delete_attachment_success() {
        let mut state = AppState::new();
        let id = state.create_attachment("a".repeat(201)).unwrap();

        let result = state.delete_attachment(id);
        assert!(result.is_ok());
        assert_eq!(state.attachments.len(), 0);
    }

    #[test]
    fn test_delete_attachment_not_found() {
        let mut state = AppState::new();

        let result = state.delete_attachment(999);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_delete_attachment_correct_id() {
        let mut state = AppState::new();
        let id1 = state.create_attachment("a".repeat(201)).unwrap();
        let id2 = state.create_attachment("b".repeat(201)).unwrap();
        let id3 = state.create_attachment("c".repeat(201)).unwrap();

        state.delete_attachment(id2).unwrap();

        assert_eq!(state.attachments.len(), 2);
        assert!(state.get_attachment(id1).is_some());
        assert!(state.get_attachment(id2).is_none());
        assert!(state.get_attachment(id3).is_some());
    }

    #[test]
    fn test_clear_attachments() {
        let mut state = AppState::new();
        state.create_attachment("a".repeat(201)).unwrap();
        state.create_attachment("b".repeat(201)).unwrap();
        state.create_attachment("c".repeat(201)).unwrap();

        state.clear_attachments();

        assert_eq!(state.attachments.len(), 0);
        assert_eq!(state.next_attachment_id, 1);
    }

    #[test]
    fn test_clear_attachments_resets_id_counter() {
        let mut state = AppState::new();
        state.create_attachment("a".repeat(201)).unwrap();
        state.clear_attachments();

        let new_id = state.create_attachment("b".repeat(201)).unwrap();
        assert_eq!(new_id, 1);
    }

    #[test]
    fn test_get_attachment_found() {
        let mut state = AppState::new();
        let content = "test content".repeat(20);
        let id = state.create_attachment(content.clone()).unwrap();

        let attachment = state.get_attachment(id);
        assert!(attachment.is_some());
        assert_eq!(attachment.unwrap().content, content);
    }

    #[test]
    fn test_get_attachment_not_found() {
        let state = AppState::new();
        let attachment = state.get_attachment(999);
        assert!(attachment.is_none());
    }

    #[test]
    fn test_get_attachment_mut() {
        let mut state = AppState::new();
        let id = state.create_attachment("a".repeat(201)).unwrap();

        let new_content = "b".repeat(300);
        if let Some(attachment) = state.get_attachment_mut(id) {
            attachment.update_content(new_content.clone());
        }

        let attachment = state.get_attachment(id).unwrap();
        assert_eq!(attachment.content, new_content);
        assert_eq!(attachment.size_chars, 300);
    }
}
