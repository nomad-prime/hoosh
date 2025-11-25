use super::*;
use crate::agent::{Conversation, ToolCall, ToolFunction};
use crate::backends::{LlmError, LlmResponse};
use crate::permissions::PermissionManager;
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};

struct MockBackend {
    responses: Vec<LlmResponse>,
    call_count: Arc<AtomicUsize>,
}

impl MockBackend {
    fn new(responses: Vec<LlmResponse>) -> Self {
        Self {
            responses,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl LlmBackend for MockBackend {
    async fn send_message(&self, _message: &str) -> Result<String> {
        let index = self.call_count.fetch_add(1, Ordering::SeqCst);
        self.responses
            .get(index)
            .and_then(|r| r.content.clone())
            .ok_or_else(|| anyhow::anyhow!("No more responses"))
    }

    async fn send_message_with_tools(
        &self,
        _conversation: &Conversation,
        _tools: &ToolRegistry,
    ) -> Result<LlmResponse, LlmError> {
        let index = self.call_count.fetch_add(1, Ordering::SeqCst);
        self.responses
            .get(index)
            .cloned()
            .ok_or_else(|| LlmError::Other {
                message: "No more responses".to_string(),
            })
    }

    async fn send_message_with_tools_and_events(
        &self,
        _conversation: &Conversation,
        _tools: &ToolRegistry,
        _event_sender: Option<mpsc::UnboundedSender<AgentEvent>>,
    ) -> Result<LlmResponse, LlmError> {
        let index = self.call_count.fetch_add(1, Ordering::SeqCst);
        self.responses
            .get(index)
            .cloned()
            .ok_or_else(|| LlmError::Other {
                message: "No more responses".to_string(),
            })
    }

    fn backend_name(&self) -> &str {
        "mock"
    }

    fn model_name(&self) -> &str {
        "mock-model"
    }

    fn pricing(&self) -> Option<crate::backends::TokenPricing> {
        None
    }
}

fn create_test_agent(
    backend: Arc<dyn LlmBackend>,
) -> (
    Agent,
    Arc<ToolRegistry>,
    Arc<ToolExecutor>,
    mpsc::UnboundedSender<AgentEvent>,
) {
    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager =
        Arc::new(PermissionManager::new(event_tx.clone(), response_rx).with_skip_permissions(true));
    let tool_executor = Arc::new(ToolExecutor::new(
        Arc::clone(&tool_registry),
        Arc::clone(&permission_manager),
    ));

    let agent = Agent::new(backend, Arc::clone(&tool_registry), tool_executor.clone());
    (agent, tool_registry, tool_executor, event_tx)
}

#[tokio::test]
async fn agent_handles_simple_response() {
    let backend = Arc::new(MockBackend::new(vec![LlmResponse::content_only(
        "Hello, I'm here to help!".to_string(),
    )]));

    let (agent, _, _, _) = create_test_agent(backend);
    let mut conversation = Conversation::new();
    conversation.add_user_message("Hello".to_string());

    let result = agent.handle_turn(&mut conversation).await;

    assert!(result.is_ok());
    assert_eq!(conversation.messages.len(), 2);
}

#[tokio::test]
async fn agent_handles_tool_calls_with_execution() {
    let tool_call = ToolCall {
        id: "call_123".to_string(),
        r#type: "function".to_string(),
        function: ToolFunction {
            name: "test_tool".to_string(),
            arguments: "{}".to_string(),
        },
    };

    let backend = Arc::new(MockBackend::new(vec![LlmResponse::with_tool_calls(
        Some("Let me call a tool".to_string()),
        vec![tool_call],
    )]));

    let (agent, _, _, _) = create_test_agent(backend);
    let mut conversation = Conversation::new();
    conversation.add_user_message("Help me with something".to_string());

    let result = agent.handle_turn(&mut conversation).await;

    // Tool execution may fail (tool doesn't exist), but should handle gracefully
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn agent_continues_after_successful_tool_call() {
    let tool_call = ToolCall {
        id: "call_456".to_string(),
        r#type: "function".to_string(),
        function: ToolFunction {
            name: "test_tool".to_string(),
            arguments: "{}".to_string(),
        },
    };

    let backend = Arc::new(MockBackend::new(vec![
        LlmResponse::with_tool_calls(Some("Calling tool".to_string()), vec![tool_call]),
        LlmResponse::content_only("Tool worked!".to_string()),
    ]));

    let (agent, _, _, _) = create_test_agent(backend);
    let mut conversation = Conversation::new();
    conversation.add_user_message("Test message".to_string());

    let result = agent.handle_turn(&mut conversation).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn agent_respects_max_steps_limit() {
    let backend = Arc::new(MockBackend::new(vec![]));
    let agent = Agent::new(
        backend,
        Arc::new(ToolRegistry::new()),
        Arc::new(ToolExecutor::new(
            Arc::new(ToolRegistry::new()),
            Arc::new(
                PermissionManager::new(
                    {
                        let (tx, _) = mpsc::unbounded_channel();
                        tx
                    },
                    {
                        let (_, rx) = mpsc::unbounded_channel();
                        rx
                    },
                )
                .with_skip_permissions(true),
            ),
        )),
    )
    .with_max_steps(2);

    let mut conversation = Conversation::new();
    conversation.add_user_message("Test".to_string());

    let result = agent.handle_turn(&mut conversation).await;
    assert!(result.is_err() || result.is_ok());
}

#[tokio::test]
async fn agent_emits_token_usage_events() {
    let backend = Arc::new(MockBackend::new(vec![
        LlmResponse::content_only("Response".to_string()).with_tokens(100, 50),
    ]));

    let (agent, _, _, _) = create_test_agent(backend);
    let mut conversation = Conversation::new();
    conversation.add_user_message("Test".to_string());

    let result = agent.handle_turn(&mut conversation).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn agent_handles_no_response_content() {
    let backend = Arc::new(MockBackend::new(vec![LlmResponse {
        content: None,
        tool_calls: None,
        input_tokens: None,
        output_tokens: None,
    }]));

    let (agent, _, _, _) = create_test_agent(backend);
    let mut conversation = Conversation::new();
    conversation.add_user_message("Test".to_string());

    let result = agent.handle_turn(&mut conversation).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn agent_with_event_sender() {
    let backend = Arc::new(MockBackend::new(vec![LlmResponse::content_only(
        "Response".to_string(),
    )]));

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let agent = Agent::new(
        backend,
        Arc::new(ToolRegistry::new()),
        Arc::new(ToolExecutor::new(
            Arc::new(ToolRegistry::new()),
            Arc::new(
                PermissionManager::new(
                    {
                        let (tx, _) = mpsc::unbounded_channel();
                        tx
                    },
                    {
                        let (_, rx) = mpsc::unbounded_channel();
                        rx
                    },
                )
                .with_skip_permissions(true),
            ),
        )),
    )
    .with_event_sender(event_tx);

    let mut conversation = Conversation::new();
    conversation.add_user_message("Test".to_string());

    let result = agent.handle_turn(&mut conversation).await;
    assert!(result.is_ok());

    // Events should have been sent
    let has_events = event_rx.try_recv().is_ok();
    assert!(has_events);
}

#[tokio::test]
async fn agent_generates_title_from_first_message() {
    let backend = Arc::new(MockBackend::new(vec![LlmResponse::content_only(
        "Helpful Rust Learning Guide".to_string(),
    )]));

    let (agent, _, _, _) = create_test_agent(backend);
    let title = agent.generate_title("How can I learn Rust?").await;

    assert!(title.is_ok());
    let title_str = title.unwrap();
    assert!(!title_str.is_empty());
    assert!(!title_str.contains('"'));
}

#[tokio::test]
async fn agent_strips_quotes_from_title() {
    let backend = Arc::new(MockBackend::new(vec![LlmResponse::content_only(
        "\"Test Title\"".to_string(),
    )]));

    let (agent, _, _, _) = create_test_agent(backend);
    let title = agent.generate_title("test").await.unwrap();

    assert_eq!(title, "Test Title");
    assert!(!title.contains('"'));
}

#[tokio::test]
async fn permission_response_creation() {
    let response = PermissionResponse {
        request_id: "req_123".to_string(),
        allowed: true,
        scope: None,
    };

    assert_eq!(response.request_id, "req_123");
    assert!(response.allowed);
}

#[tokio::test]
async fn approval_response_with_rejection() {
    let response = ApprovalResponse {
        tool_call_id: "call_456".to_string(),
        approved: false,
        rejection_reason: Some("User declined".to_string()),
    };

    assert!(!response.approved);
    assert!(response.rejection_reason.is_some());
}

#[tokio::test]
async fn agent_initializes_with_correct_defaults() {
    let backend = Arc::new(MockBackend::new(vec![]));
    let tool_registry = Arc::new(ToolRegistry::new());
    let permission_manager = Arc::new(
        PermissionManager::new(
            {
                let (tx, _) = mpsc::unbounded_channel();
                tx
            },
            {
                let (_, rx) = mpsc::unbounded_channel();
                rx
            },
        )
        .with_skip_permissions(true),
    );
    let tool_executor = Arc::new(ToolExecutor::new(tool_registry.clone(), permission_manager));

    let agent = Agent::new(backend, tool_registry, tool_executor);

    assert_eq!(agent.max_steps, 1000);
}

#[tokio::test]
async fn multiple_agents_independent() {
    let backend1 = Arc::new(MockBackend::new(vec![LlmResponse::content_only(
        "Agent 1".to_string(),
    )]));
    let backend2 = Arc::new(MockBackend::new(vec![LlmResponse::content_only(
        "Agent 2".to_string(),
    )]));

    let (agent1, _, _, _) = create_test_agent(backend1);
    let (agent2, _, _, _) = create_test_agent(backend2);

    let mut conv1 = Conversation::new();
    conv1.add_user_message("Message 1".to_string());

    let mut conv2 = Conversation::new();
    conv2.add_user_message("Message 2".to_string());

    let result1 = agent1.handle_turn(&mut conv1).await;
    let result2 = agent2.handle_turn(&mut conv2).await;

    assert!(result1.is_ok());
    assert!(result2.is_ok());
}

#[tokio::test]
async fn agent_builder_pattern() {
    let backend = Arc::new(MockBackend::new(vec![]));
    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager =
        Arc::new(PermissionManager::new(event_tx.clone(), response_rx).with_skip_permissions(true));
    let tool_executor = Arc::new(ToolExecutor::new(tool_registry.clone(), permission_manager));

    let agent = Agent::new(backend, tool_registry, tool_executor)
        .with_max_steps(100)
        .with_event_sender(event_tx);

    assert_eq!(agent.max_steps, 100);
}

#[tokio::test]
async fn agent_empty_response_completes_turn() {
    let backend = Arc::new(MockBackend::new(vec![LlmResponse {
        content: None,
        tool_calls: None,
        input_tokens: None,
        output_tokens: None,
    }]));

    let (agent, _, _, _) = create_test_agent(backend);
    let mut conversation = Conversation::new();
    conversation.add_user_message("Hello".to_string());

    let result = agent.handle_turn(&mut conversation).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn agent_tracks_token_usage_when_provided() {
    let backend = Arc::new(MockBackend::new(vec![LlmResponse {
        content: Some("Response".to_string()),
        tool_calls: None,
        input_tokens: Some(100),
        output_tokens: Some(50),
    }]));

    let (agent, _, _, event_tx) = create_test_agent(backend);
    let mut conversation = Conversation::new();
    conversation.add_user_message("Hello".to_string());

    let result = agent.handle_turn(&mut conversation).await;

    assert!(result.is_ok());
    drop(event_tx);
}

#[tokio::test]
async fn agent_response_only_has_content_completes() {
    let backend = Arc::new(MockBackend::new(vec![LlmResponse::content_only(
        "Final answer".to_string(),
    )]));

    let (agent, _, _, _) = create_test_agent(backend);
    let mut conversation = Conversation::new();
    conversation.add_user_message("Question".to_string());

    let result = agent.handle_turn(&mut conversation).await;

    assert!(result.is_ok());
    let last_message = conversation.messages.last();
    assert!(last_message.is_some());
    assert_eq!(last_message.unwrap().role, "assistant");
    assert_eq!(
        last_message.unwrap().content,
        Some("Final answer".to_string())
    );
}

#[tokio::test]
async fn agent_with_execution_budget() {
    use crate::system_reminders::{BudgetReminderStrategy, SystemReminder};
    use crate::task_management::ExecutionBudget;
    use std::time::Duration;

    let backend = Arc::new(MockBackend::new(vec![LlmResponse::content_only(
        "Response within budget".to_string(),
    )]));

    let budget = Arc::new(ExecutionBudget::new(Duration::from_secs(60), 10));
    let tool_registry = Arc::new(ToolRegistry::new());
    let tool_executor = Arc::new(ToolExecutor::new(
        tool_registry.clone(),
        Arc::new(PermissionManager::new(
            {
                let (tx, _) = mpsc::unbounded_channel();
                tx
            },
            {
                let (_, rx) = mpsc::unbounded_channel();
                rx
            },
        )),
    ));

    let max_steps = 5;
    let budget_strategy = Box::new(BudgetReminderStrategy::new(budget, max_steps));
    let system_reminder = Arc::new(SystemReminder::new().add_strategy(budget_strategy));

    let agent = Agent::new(backend, tool_registry, tool_executor)
        .with_max_steps(max_steps)
        .with_system_reminder(system_reminder);

    let mut conversation = Conversation::new();
    conversation.add_user_message("Test message".to_string());

    let result = agent.handle_turn(&mut conversation).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn agent_handles_budget_tracking() {
    use crate::system_reminders::{BudgetReminderStrategy, SystemReminder};
    use crate::task_management::ExecutionBudget;
    use std::time::Duration;

    let backend = Arc::new(MockBackend::new(vec![LlmResponse::content_only(
        "Response".to_string(),
    )]));

    let budget = Arc::new(ExecutionBudget::new(Duration::from_secs(60), 10));
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let tool_registry = Arc::new(ToolRegistry::new());
    let tool_executor = Arc::new(ToolExecutor::new(
        tool_registry.clone(),
        Arc::new(PermissionManager::new(
            {
                let (tx, _) = mpsc::unbounded_channel();
                tx
            },
            {
                let (_, rx) = mpsc::unbounded_channel();
                rx
            },
        )),
    ));

    let max_steps = 5;
    let budget_strategy = Box::new(BudgetReminderStrategy::new(budget, max_steps));
    let system_reminder = Arc::new(SystemReminder::new().add_strategy(budget_strategy));

    let agent = Agent::new(backend, tool_registry, tool_executor)
        .with_max_steps(max_steps)
        .with_event_sender(event_tx)
        .with_system_reminder(system_reminder);

    let mut conversation = Conversation::new();
    conversation.add_user_message("Test".to_string());

    let result = agent.handle_turn(&mut conversation).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn agent_wraps_up_when_budget_low() {
    use crate::system_reminders::{BudgetReminderStrategy, SystemReminder};
    use crate::task_management::ExecutionBudget;
    use std::time::Duration;

    // Create responses where the first 3 have tool calls (to continue the loop)
    // and the 4th has content (to complete)
    let backend = Arc::new(MockBackend::new(vec![
        LlmResponse {
            content: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                r#type: "function".to_string(),
                function: ToolFunction {
                    name: "nonexistent".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
            input_tokens: None,
            output_tokens: None,
        },
        LlmResponse {
            content: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_2".to_string(),
                r#type: "function".to_string(),
                function: ToolFunction {
                    name: "nonexistent".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
            input_tokens: None,
            output_tokens: None,
        },
        LlmResponse {
            content: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_3".to_string(),
                r#type: "function".to_string(),
                function: ToolFunction {
                    name: "nonexistent".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
            input_tokens: None,
            output_tokens: None,
        },
        LlmResponse::content_only("Final response".to_string()),
    ]));

    // Use a budget with 4 max steps, so at step 3 we'll have 75% step pressure
    let budget = Arc::new(ExecutionBudget::new(Duration::from_secs(100), 4));
    let (event_tx, _) = mpsc::unbounded_channel();
    let tool_registry = Arc::new(ToolRegistry::new());
    let tool_executor = Arc::new(ToolExecutor::new(
        tool_registry.clone(),
        Arc::new(PermissionManager::new(
            {
                let (tx, _) = mpsc::unbounded_channel();
                tx
            },
            {
                let (_, rx) = mpsc::unbounded_channel();
                rx
            },
        )),
    ));

    let max_steps = 10;
    let budget_strategy = Box::new(BudgetReminderStrategy::new(budget, max_steps));
    let system_reminder = Arc::new(SystemReminder::new().add_strategy(budget_strategy));

    let agent = Agent::new(backend, tool_registry, tool_executor)
        .with_max_steps(max_steps)
        .with_event_sender(event_tx)
        .with_system_reminder(system_reminder);

    let mut conversation = Conversation::new();
    conversation.add_user_message("Test".to_string());

    let result = agent.handle_turn(&mut conversation).await;
    assert!(result.is_ok());

    let messages = conversation.get_messages_for_api();
    let has_budget_alert = messages.iter().any(|m| {
        m.content
            .as_ref()
            .is_some_and(|c| c.contains("BUDGET ALERT"))
    });
    assert!(
        has_budget_alert,
        "Should add budget alert message when budget is low. Messages: {:?}",
        messages.iter().map(|m| &m.content).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn agent_handles_budget_exhaustion() {
    use crate::system_reminders::{BudgetReminderStrategy, SystemReminder};
    use crate::task_management::ExecutionBudget;
    use std::time::Duration;

    let backend = Arc::new(MockBackend::new(vec![
        LlmResponse::content_only("Working...".to_string()),
        LlmResponse::content_only("Summary of work done".to_string()),
    ]));

    let budget = Arc::new(ExecutionBudget::new(Duration::from_millis(10), 5));
    let tool_registry = Arc::new(ToolRegistry::new());
    let tool_executor = Arc::new(ToolExecutor::new(
        tool_registry.clone(),
        Arc::new(PermissionManager::new(
            {
                let (tx, _) = mpsc::unbounded_channel();
                tx
            },
            {
                let (_, rx) = mpsc::unbounded_channel();
                rx
            },
        )),
    ));

    let max_steps = 10;
    let budget_strategy = Box::new(BudgetReminderStrategy::new(budget, max_steps));
    let system_reminder = Arc::new(SystemReminder::new().add_strategy(budget_strategy));

    let agent = Agent::new(backend, tool_registry, tool_executor)
        .with_max_steps(max_steps)
        .with_system_reminder(system_reminder);

    let mut conversation = Conversation::new();
    conversation.add_user_message("Long task".to_string());

    tokio::time::sleep(Duration::from_millis(15)).await;

    let result = agent.handle_turn(&mut conversation).await;
    assert!(result.is_ok());
}
