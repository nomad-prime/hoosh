use super::clipboard::ClipboardManager;
use super::events::AgentState;
use crate::completion::Completer;
use crate::conversations::AgentEvent;
use crate::history::PromptHistory;
use crate::permissions::OperationType;
use rand::Rng;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::collections::VecDeque;
use tui_textarea::TextArea;

#[derive(Clone)]
pub enum MessageLine {
    Plain(String),
    Styled(Line<'static>),
}

pub struct CompletionState {
    pub candidates: Vec<String>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    #[allow(dead_code)]
    pub trigger_position: usize,
    pub query: String,
    pub completer_index: usize,
}

pub struct PermissionDialogState {
    pub operation: OperationType,
    pub request_id: String,
    pub selected_index: usize,
    pub options: Vec<PermissionOption>,
}

pub struct ApprovalDialogState {
    pub tool_call_id: String,
    pub tool_name: String,
    pub selected_index: usize,
    pub scroll_offset: usize,
}

impl ApprovalDialogState {
    pub fn new(tool_call_id: String, tool_name: String) -> Self {
        Self {
            tool_call_id,
            tool_name,
            selected_index: 0, // 0 = Approve, 1 = Reject
            scroll_offset: 0,
        }
    }
}

pub struct InitialPermissionDialogState {
    pub project_path: std::path::PathBuf,
    pub selected_index: usize,
}

impl InitialPermissionDialogState {
    pub fn new(project_path: std::path::PathBuf) -> Self {
        Self {
            project_path,
            selected_index: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub enum InitialPermissionChoice {
    ReadOnly,
    EnableWriteEdit,
    Deny,
}

#[derive(Clone)]
pub enum PermissionOption {
    YesOnce,
    No,
    TrustProject(std::path::PathBuf),
}

impl CompletionState {
    pub fn new(trigger_position: usize, completer_index: usize) -> Self {
        Self {
            candidates: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            trigger_position,
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

pub struct AppState {
    pub input: TextArea<'static>,
    pub messages: VecDeque<MessageLine>,
    pub pending_messages: VecDeque<MessageLine>,
    pub agent_state: AgentState,
    pub should_quit: bool,
    pub should_cancel_task: bool,
    pub max_messages: usize,
    pub completion_state: Option<CompletionState>,
    pub completers: Vec<Box<dyn Completer>>,
    pub permission_dialog_state: Option<PermissionDialogState>,
    pub approval_dialog_state: Option<ApprovalDialogState>,
    pub initial_permission_dialog_state: Option<InitialPermissionDialogState>,
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
            max_messages: 1000,
            completion_state: None,
            completers: Vec::new(),
            permission_dialog_state: None,
            approval_dialog_state: None,
            initial_permission_dialog_state: None,
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

    pub fn is_showing_permission_dialog(&self) -> bool {
        self.permission_dialog_state.is_some()
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

    pub fn show_permission_dialog(&mut self, operation: OperationType, request_id: String) {
        let options = if let Ok(current_dir) = std::env::current_dir() {
            vec![
                PermissionOption::YesOnce,
                PermissionOption::TrustProject(current_dir),
                PermissionOption::No,
            ]
        } else {
            vec![PermissionOption::YesOnce, PermissionOption::No]
        };

        self.permission_dialog_state = Some(PermissionDialogState {
            operation,
            request_id,
            selected_index: 0,
            options,
        });
    }

    pub fn select_next_permission_option(&mut self) {
        if let Some(dialog) = &mut self.permission_dialog_state
            && !dialog.options.is_empty()
        {
            dialog.selected_index = (dialog.selected_index + 1) % dialog.options.len();
        }
    }

    pub fn select_prev_permission_option(&mut self) {
        if let Some(dialog) = &mut self.permission_dialog_state
            && !dialog.options.is_empty()
        {
            if dialog.selected_index == 0 {
                dialog.selected_index = dialog.options.len() - 1;
            } else {
                dialog.selected_index -= 1;
            }
        }
    }

    pub fn hide_permission_dialog(&mut self) {
        self.permission_dialog_state = None;
    }

    pub fn show_initial_permission_dialog(&mut self, project_path: std::path::PathBuf) {
        self.initial_permission_dialog_state =
            Some(InitialPermissionDialogState::new(project_path));
    }

    pub fn is_showing_initial_permission_dialog(&self) -> bool {
        self.initial_permission_dialog_state.is_some()
    }

    pub fn select_next_initial_permission_option(&mut self) {
        if let Some(dialog) = &mut self.initial_permission_dialog_state {
            dialog.selected_index = (dialog.selected_index + 1) % 3;
        }
    }

    pub fn select_prev_initial_permission_option(&mut self) {
        if let Some(dialog) = &mut self.initial_permission_dialog_state {
            if dialog.selected_index == 0 {
                dialog.selected_index = 2;
            } else {
                dialog.selected_index -= 1;
            }
        }
    }

    pub fn get_selected_initial_permission_choice(&self) -> Option<InitialPermissionChoice> {
        self.initial_permission_dialog_state
            .as_ref()
            .map(|dialog| match dialog.selected_index {
                0 => InitialPermissionChoice::ReadOnly,
                1 => InitialPermissionChoice::EnableWriteEdit,
                2 => InitialPermissionChoice::Deny,
                _ => InitialPermissionChoice::ReadOnly,
            })
    }

    pub fn hide_initial_permission_dialog(&mut self) {
        self.initial_permission_dialog_state = None;
    }

    pub fn start_completion(&mut self, trigger_position: usize, completer_index: usize) {
        self.completion_state = Some(CompletionState::new(trigger_position, completer_index));
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
        self.messages.push_back(msg_line.clone());
        if self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
        self.pending_messages.push_back(msg_line);
    }

    pub fn add_debug_message(&mut self, message: String) {
        let styled_line = Line::from(Span::styled(
            format!("  [DEBUG] {}", message),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::ITALIC),
        ));
        self.add_styled_line(styled_line);
    }

    pub fn add_styled_line(&mut self, line: Line<'static>) {
        let msg_line = MessageLine::Styled(line);
        self.messages.push_back(msg_line.clone());
        if self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
        self.pending_messages.push_back(msg_line);
    }

    pub fn has_pending_messages(&self) -> bool {
        !self.pending_messages.is_empty()
    }

    pub fn drain_pending_messages(&mut self) -> Vec<MessageLine> {
        self.pending_messages.drain(..).collect()
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
            AgentEvent::ToolCalls(tool_call_displays) => {
                self.agent_state = AgentState::ExecutingTools;
                let mut rng = rand::thread_rng();
                self.current_executing_spinner = rng.gen_range(0..7);
                for display_name in tool_call_displays {
                    self.add_tool_call(&display_name);
                }
            }
            AgentEvent::ToolPreview {
                tool_name: _,
                preview,
            } => {
                self.add_tool_preview(&preview);
            }
            AgentEvent::ToolResult { summary, .. } => {
                self.add_status_message(&summary);
            }
            AgentEvent::ToolExecutionComplete => {
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
            AgentEvent::PermissionRequest { .. } => {}
            AgentEvent::ApprovalRequest { .. } => {}
            AgentEvent::UserRejection => {
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
            AgentEvent::AgentSwitched { .. } => {
                // The event is handled in the event loop, but we might want to show a message
                // or update UI elements here if needed
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
            AgentEvent::ContextCompressionTriggered {
                original_message_count,
                token_pressure,
                ..
            } => {
                self.add_status_message(&format!(
                    "Compressing context ({} messages, {:.0}% token pressure)",
                    original_message_count,
                    token_pressure * 100.0
                ));
            }
            AgentEvent::ContextCompressionComplete { summary_length } => {
                self.add_status_message(&format!(
                    "Context compressed (summarized {} messages)",
                    summary_length
                ));
            }
            AgentEvent::ContextCompressionError { error } => {
                self.add_message(format!("Context compression error: {}", error));
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
        }
    }

    pub fn add_thought(&mut self, content: &str) {
        if !content.is_empty() {
            self.add_message(format!("\n• {}", content));
        }
    }

    pub fn add_tool_call(&mut self, name: &str) {
        self.add_message(format!("\n● {}", name));
    }

    pub fn add_status_message(&mut self, message: &str) {
        self.add_message(format!("  ⎿  {}", message));
    }

    pub fn add_error(&mut self, error: &str) {
        self.add_message(format!("  Error: {}", error));
    }

    pub fn add_final_response(&mut self, content: &str) {
        let indented_content = content
            .lines()
            .map(|line| format!("  {}", line))
            .collect::<Vec<_>>()
            .join("\n");
        self.add_message(indented_content);
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
                .fg(Color::Red)
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
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
