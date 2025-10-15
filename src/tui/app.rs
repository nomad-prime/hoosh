use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use std::collections::VecDeque;
use tui_textarea::TextArea;

use super::completion::Completer;
use super::events::AgentState;
use super::history::PromptHistory;
use crate::conversations::AgentEvent;
use crate::permissions::OperationType;

#[derive(Clone)]
pub enum MessageLine {
    Plain(String),
    Styled(Line<'static>),
}

pub struct CompletionState {
    pub candidates: Vec<String>,
    pub selected_index: usize,
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

#[derive(Clone)]
pub enum PermissionOption {
    YesOnce,
    No,
    AlwaysForFile,
    AlwaysForDirectory(String),
    AlwaysForType,
}

impl CompletionState {
    pub fn new(trigger_position: usize, completer_index: usize) -> Self {
        Self {
            candidates: Vec::new(),
            selected_index: 0,
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
        }
    }

    pub fn select_prev(&mut self) {
        if !self.candidates.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.candidates.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }
}

pub struct AppState {
    pub input: TextArea<'static>,
    pub messages: VecDeque<MessageLine>, // Keep for reference, but won't render
    pub pending_messages: VecDeque<MessageLine>, // Messages to insert with insert_before
    pub agent_state: AgentState,
    pub should_quit: bool,
    pub max_messages: usize,
    pub completion_state: Option<CompletionState>,
    pub completers: Vec<Box<dyn Completer>>,
    pub permission_dialog_state: Option<PermissionDialogState>,
    pub animation_frame: usize,
    pub prompt_history: PromptHistory,
}

impl AppState {
    pub fn new() -> Self {
        let mut input = TextArea::default();
        input.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        input.set_cursor_line_style(Style::default());

        Self {
            input,
            messages: VecDeque::new(),
            pending_messages: VecDeque::new(),
            agent_state: AgentState::Idle,
            should_quit: false,
            max_messages: 1000,
            completion_state: None,
            completers: Vec::new(),
            permission_dialog_state: None,
            animation_frame: 0,
            prompt_history: PromptHistory::new(1000),
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

    pub fn show_permission_dialog(&mut self, operation: OperationType, request_id: String) {
        // Build the list of options based on the operation type
        let mut options = vec![
            PermissionOption::YesOnce,
            PermissionOption::No,
            PermissionOption::AlwaysForFile,
        ];

        // Add directory option if applicable
        if let Some(dir) = operation.parent_directory() {
            options.push(PermissionOption::AlwaysForDirectory(dir));
        }

        options.push(PermissionOption::AlwaysForType);

        self.permission_dialog_state = Some(PermissionDialogState {
            operation,
            request_id,
            selected_index: 0,
            options,
        });
    }

    pub fn select_next_permission_option(&mut self) {
        if let Some(dialog) = &mut self.permission_dialog_state {
            if !dialog.options.is_empty() {
                dialog.selected_index = (dialog.selected_index + 1) % dialog.options.len();
            }
        }
    }

    pub fn select_prev_permission_option(&mut self) {
        if let Some(dialog) = &mut self.permission_dialog_state {
            if !dialog.options.is_empty() {
                if dialog.selected_index == 0 {
                    dialog.selected_index = dialog.options.len() - 1;
                } else {
                    dialog.selected_index -= 1;
                }
            }
        }
    }

    pub fn hide_permission_dialog(&mut self) {
        self.permission_dialog_state = None;
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
        // Add to messages for history
        self.messages.push_back(msg_line.clone());
        if self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
        // Queue for insertion above viewport
        self.pending_messages.push_back(msg_line);
    }

    pub fn add_styled_line(&mut self, line: Line<'static>) {
        let msg_line = MessageLine::Styled(line);
        // Add to messages for history
        self.messages.push_back(msg_line.clone());
        if self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
        // Queue for insertion above viewport
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
            }
            AgentEvent::AssistantThought(content) => {
                if !content.is_empty() {
                    self.add_message(format!("• {}", content));
                }
            }
            AgentEvent::ToolCalls(tool_call_displays) => {
                self.agent_state = AgentState::ExecutingTools;
                for display_name in tool_call_displays {
                    self.add_message(format!("● {}", display_name));
                }
            }
            AgentEvent::ToolResult { summary, .. } => {
                self.add_message(format!(" ⎿ {}", summary));
            }
            AgentEvent::ToolExecutionComplete => {
                self.add_message("\n".to_string());
            }
            AgentEvent::FinalResponse(content) => {
                self.agent_state = AgentState::Idle;
                let indented_content = content
                    .lines()
                    .map(|line| format!("  {}", line))
                    .collect::<Vec<_>>()
                    .join("\n");
                self.add_message(indented_content);
                self.add_message("\n".to_string());
            }
            AgentEvent::Error(error) => {
                self.agent_state = AgentState::Idle;
                self.add_message(format!("❌ Error: {}", error));
            }
            AgentEvent::MaxStepsReached(max_steps) => {
                self.agent_state = AgentState::Idle;
                self.add_message(format!(
                    "⚠️ Maximum conversation steps ({}) reached, stopping.",
                    max_steps
                ));
            }
            AgentEvent::PermissionRequest { .. } => {
                // Permission requests are handled separately in the TUI event loop
                // This variant should not reach here, but we include it for exhaustiveness
            }
            AgentEvent::Exit => {
                // Exit is handled in the event loop
                // This variant should not reach here, but we include it for exhaustiveness
            }
            AgentEvent::ClearConversation => {
                // ClearConversation is handled in the event loop
                // This variant should not reach here, but we include it for exhaustiveness
            }
        }
    }

    pub fn get_input_text(&self) -> String {
        self.input.lines().join("\n")
    }

    pub fn clear_input(&mut self) {
        self.input = TextArea::default();
        self.input.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        // Remove the underline from the cursor line
        self.input.set_cursor_line_style(Style::default());
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
