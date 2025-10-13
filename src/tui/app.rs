use std::collections::VecDeque;
use tui_textarea::TextArea;

use super::events::{AgentEvent, AgentState};

pub struct AppState {
    pub input: TextArea<'static>,
    pub messages: VecDeque<String>,
    pub agent_state: AgentState,
    pub should_quit: bool,
    pub max_messages: usize,
    pub scroll_offset: u16,
    pub viewport_height: u16,
    pub initial_scroll_done: bool,
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
        }
    }

    pub fn add_message(&mut self, message: String) {
        self.messages.push_back(message);
        if self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
        // Auto-scroll to bottom when new message arrives
        self.scroll_to_bottom();
    }

    pub fn total_lines(&self) -> u16 {
        self.messages
            .iter()
            .map(|msg| msg.lines().count() as u16)
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
                    self.add_message(format!(" • {}", content));
                }
            }
            AgentEvent::ToolCalls(tool_call_displays) => {
                self.agent_state = AgentState::ExecutingTools;
                for display_name in tool_call_displays {
                    self.add_message(format!(" ● {}", display_name));
                }
            }
            AgentEvent::ToolResult { summary, .. } => {
                self.add_message(format!("   ⎿ {}", summary));
                self.add_message(String::new());
            }
            AgentEvent::ToolExecutionComplete => {
                self.add_message("\n".to_string());
            }
            AgentEvent::FinalResponse(content) => {
                self.agent_state = AgentState::Idle;
                self.add_message(content);
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
