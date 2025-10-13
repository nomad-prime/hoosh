use ratatui::text::Line;
use std::collections::VecDeque;
use tui_textarea::TextArea;

use super::completion::Completer;
use super::events::{AgentEvent, AgentState};

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
    pub messages: VecDeque<MessageLine>,
    pub agent_state: AgentState,
    pub should_quit: bool,
    pub max_messages: usize,
    pub scroll_offset: u16,
    pub viewport_height: u16,
    pub initial_scroll_done: bool,
    pub completion_state: Option<CompletionState>,
    pub completers: Vec<Box<dyn Completer>>,
}

impl AppState {
    pub fn new() -> Self {
        let mut input = TextArea::default();
        input.set_placeholder_text("Type your message here...");

        Self {
            input,
            messages: VecDeque::new(),
            agent_state: AgentState::Idle,
            should_quit: false,
            max_messages: 1000,
            scroll_offset: 0,
            viewport_height: 0,
            initial_scroll_done: false,
            completion_state: None,
            completers: Vec::new(),
        }
    }

    pub fn register_completer(&mut self, completer: Box<dyn Completer>) {
        self.completers.push(completer);
    }

    pub fn find_completer_for_key(&self, key: char) -> Option<usize> {
        self.completers
            .iter()
            .position(|c| c.trigger_key() == key)
    }

    pub fn is_completing(&self) -> bool {
        self.completion_state.is_some()
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
        self.messages.push_back(MessageLine::Plain(message));
        if self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
        // Auto-scroll to bottom when new message arrives
        self.scroll_to_bottom();
    }

    pub fn add_styled_line(&mut self, line: Line<'static>) {
        self.messages.push_back(MessageLine::Styled(line));
        if self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
        // Auto-scroll to bottom when new message arrives
        self.scroll_to_bottom();
    }

    pub fn total_lines(&self) -> u16 {
        self.messages
            .iter()
            .map(|msg| match msg {
                MessageLine::Plain(s) => s.lines().count() as u16,
                MessageLine::Styled(_) => 1,
            })
            .sum()
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn max_scroll_offset(&self) -> u16 {
        let total_lines = self.total_lines();
        total_lines.saturating_sub(self.viewport_height)
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.max_scroll_offset();
    }

    pub fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Thinking => {
                self.agent_state = AgentState::Thinking;
                self.add_message("🔄 Thinking...".to_string());
                self.add_message("\n".to_string());
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
        }
    }

    pub fn get_input_text(&self) -> String {
        self.input.lines().join("\n")
    }

    pub fn clear_input(&mut self) {
        self.input = TextArea::default();
        self.input.set_placeholder_text("Type your message here...");
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
