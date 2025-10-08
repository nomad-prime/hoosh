use anyhow::Result;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::backends::{LlmBackend, LlmResponse};
use crate::console::console;
use crate::conversations::{Conversation, ToolCall};
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;

#[derive(Debug, Clone)]
pub enum ConversationEvent {
    Thinking,
    AssistantThought(String),
    ToolCalls(Vec<ToolCall>),
    ToolResult { tool_name: String, summary: String },
    FinalResponse(String),
    Error(String),
    MaxStepsReached(usize),
}

pub struct ConversationHandler<'a> {
    backend: &'a Box<dyn LlmBackend>,
    tool_registry: &'a ToolRegistry,
    tool_executor: &'a ToolExecutor,
    max_steps: usize,
    event_sender: Option<mpsc::UnboundedSender<ConversationEvent>>,
}

impl<'a> ConversationHandler<'a> {
    pub fn new(
        backend: &'a Box<dyn LlmBackend>,
        tool_registry: &'a ToolRegistry,
        tool_executor: &'a ToolExecutor,
    ) -> Self {
        Self {
            backend,
            tool_registry,
            tool_executor,
            max_steps: 30,
            event_sender: None,
        }
    }

    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    pub fn with_event_sender(mut self, sender: mpsc::UnboundedSender<ConversationEvent>) -> Self {
        self.event_sender = Some(sender);
        self
    }

    fn send_event(&self, event: ConversationEvent) {
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(event);
        }
    }

    pub async fn handle_turn(&self, conversation: &mut Conversation) -> Result<()> {
        self.send_event(ConversationEvent::Thinking);
        console().thinking();

        for step in 0..self.max_steps {
            let response = self
                .backend
                .send_message_with_tools(conversation, self.tool_registry)
                .await?;

            match self.process_response(conversation, response, step).await? {
                TurnStatus::Continue => continue,
                TurnStatus::Complete => return Ok(()),
            }
        }

        self.send_event(ConversationEvent::MaxStepsReached(self.max_steps));
        console().max_steps_reached(self.max_steps);
        Ok(())
    }

    async fn process_response(
        &self,
        conversation: &mut Conversation,
        response: LlmResponse,
        step: usize,
    ) -> Result<TurnStatus> {
        if let Some(ref tool_calls) = response.tool_calls {
            if !tool_calls.is_empty() {
                return self.handle_tool_calls(conversation, response, step).await;
            }
        }

        if let Some(content) = response.content {
            self.send_event(ConversationEvent::FinalResponse(content.clone()));
            self.display_final_response(&content);
            conversation.add_assistant_message(Some(content), None);
            return Ok(TurnStatus::Complete);
        }

        self.send_event(ConversationEvent::Error("No response received".to_string()));
        console().warning("No response received.");
        Ok(TurnStatus::Complete)
    }

    async fn handle_tool_calls(
        &self,
        conversation: &mut Conversation,
        response: LlmResponse,
        _step: usize,
    ) -> Result<TurnStatus> {
        let tool_calls = response.tool_calls.clone().unwrap();

        conversation.add_assistant_message(response.content.clone(), Some(tool_calls.clone()));

        if let Some(ref content) = response.content {
            self.send_event(ConversationEvent::AssistantThought(content.clone()));
            console().assistant_thought(content);
        }

        self.send_event(ConversationEvent::ToolCalls(tool_calls.clone()));

        // Display tool calls and execute them one by one to maintain proper spacing
        for tool_call in tool_calls.iter() {
            self.display_tool_call(tool_call);
        }

        let tool_results = self.tool_executor.execute_tool_calls(&tool_calls).await;

        self.display_tool_results(&tool_results);

        for tool_result in tool_results {
            conversation.add_tool_result(tool_result);
        }

        Ok(TurnStatus::Continue)
    }

    fn display_tool_call(&self, tool_call: &ToolCall) {
        let args_summary = self.format_tool_args(&tool_call.function.arguments);
        console().tool_call(&tool_call.function.name, &args_summary);
    }

    fn format_tool_args(&self, args_json: &str) -> String {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(args_json) {
            let parts: Vec<String> = map
                .iter()
                .map(|(k, v)| {
                    let val_str = match v {
                        Value::String(s) => format!("\"{}\"", s),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        _ => v.to_string(),
                    };
                    format!("{}: {}", k, val_str)
                })
                .collect();
            format!("{})", parts.join(", "))
        } else {
            ")".to_string()
        }
    }

    fn display_tool_results(&self, tool_results: &[crate::conversations::ToolResult]) {
        for tool_result in tool_results {
            let summary = if let Ok(ref result) = tool_result.result {
                // Get the tool from registry to use its summary method
                if let Some(tool) = self.tool_registry.get_tool(&tool_result.tool_name) {
                    tool.result_summary(result)
                } else {
                    // Fallback if tool not found
                    "Completed successfully".to_string()
                }
            } else if let Err(ref error) = tool_result.result {
                format!("Error: {}", error)
            } else {
                continue;
            };

            self.send_event(ConversationEvent::ToolResult {
                tool_name: tool_result.tool_name.clone(),
                summary: summary.clone(),
            });
            console().tool_result_summary(&summary);
        }
        console().newline();
    }


    fn display_final_response(&self, content: &str) {
        console().plain(content);
    }
}

enum TurnStatus {
    Continue,
    Complete,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::LlmResponse;
    use crate::conversations::{ToolCall, ToolFunction};
    use crate::permissions::PermissionManager;
    use async_trait::async_trait;

    struct MockBackend {
        responses: Vec<LlmResponse>,
        current_index: std::sync::Mutex<usize>,
    }

    impl MockBackend {
        fn new(responses: Vec<LlmResponse>) -> Self {
            Self {
                responses,
                current_index: std::sync::Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl LlmBackend for MockBackend {
        async fn send_message(&self, _message: &str) -> Result<String> {
            unimplemented!()
        }

        async fn stream_message(&self, _message: &str) -> Result<crate::backends::StreamResponse> {
            unimplemented!()
        }

        async fn send_message_with_tools(
            &self,
            _conversation: &Conversation,
            _tools: &ToolRegistry,
        ) -> Result<LlmResponse> {
            let mut index = self.current_index.lock().unwrap();
            let response = self.responses.get(*index).cloned();
            *index += 1;
            response.ok_or_else(|| anyhow::anyhow!("No more responses"))
        }

        async fn stream_message_with_tools(
            &self,
            _conversation: &Conversation,
            _tools: &ToolRegistry,
        ) -> Result<crate::backends::StreamResponse> {
            unimplemented!()
        }

        fn backend_name(&self) -> &'static str {
            "mock"
        }
    }

    #[tokio::test]
    async fn test_conversation_handler_simple_response() {
        crate::console::init_console(crate::console::VerbosityLevel::Quiet);

        let mock_backend: Box<dyn LlmBackend> = Box::new(MockBackend::new(vec![
            LlmResponse::content_only("Hello, how can I help?".to_string()),
        ]));

        let tool_registry = ToolRegistry::new();
        let permission_manager = PermissionManager::new().with_skip_permissions(true);
        let tool_executor = ToolExecutor::new(tool_registry.clone(), permission_manager);

        let handler = ConversationHandler::new(&mock_backend, &tool_registry, &tool_executor);

        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let result = handler.handle_turn(&mut conversation).await;
        assert!(result.is_ok());
        assert_eq!(conversation.messages.len(), 2);
    }

    #[tokio::test]
    async fn test_conversation_handler_with_tool_call() {
        crate::console::init_console(crate::console::VerbosityLevel::Quiet);

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: r#"{"path": "test.txt"}"#.to_string(),
            },
        };

        let mock_backend: Box<dyn LlmBackend> = Box::new(MockBackend::new(vec![
            LlmResponse::with_tool_calls(Some("Reading file".to_string()), vec![tool_call]),
            LlmResponse::content_only("File read successfully".to_string()),
        ]));

        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();

        let tool_registry = ToolExecutor::create_tool_registry_with_working_dir(
            temp_dir.path().to_path_buf(),
        );
        let permission_manager = PermissionManager::new().with_skip_permissions(true);
        let tool_executor = ToolExecutor::new(tool_registry.clone(), permission_manager);

        let handler = ConversationHandler::new(&mock_backend, &tool_registry, &tool_executor);

        let mut conversation = Conversation::new();
        conversation.add_user_message("Read test.txt".to_string());

        let result = handler.handle_turn(&mut conversation).await;
        assert!(result.is_ok());
    }
}
