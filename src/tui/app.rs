use std::collections::VecDeque;
use tui_textarea::TextArea;

use super::events::{AgentEvent, AgentState};

pub struct AppState {
    pub input: TextArea<'static>,
    pub messages: VecDeque<String>,
    pub agent_state: AgentState,
    pub should_quit: bool,
    pub max_messages: usize,
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
        }
    }

    pub fn add_message(&mut self, message: String) {
        self.messages.push_back(message);
        if self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
    }

    pub fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Thinking => {
                self.agent_state = AgentState::Thinking;
                self.add_message("🔄 Thinking...".to_string());
            }
            AgentEvent::AssistantThought(content) => {
                if !content.is_empty() {
                    self.add_message(format!("• {}", content));
                }
            }
            AgentEvent::ToolCalls(tool_calls) => {
                self.agent_state = AgentState::ExecutingTools;
                for tool_call in tool_calls {
                    self.add_message(format!("⏺ {}", tool_call.function.name));
                }
            }
            AgentEvent::ToolResult { summary, .. } => {
                self.add_message(format!("  ⎿ {}", summary));
            }
            AgentEvent::FinalResponse(content) => {
                self.agent_state = AgentState::Idle;
                self.add_message(content);
                self.add_message(String::new());
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
