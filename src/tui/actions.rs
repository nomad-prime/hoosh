use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::{Agent, AgentEvent, Conversation};
use crate::backends::LlmBackend;
use crate::commands::{CommandContext, CommandResult};
use crate::context_management::ContextManager;
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;
use crate::tui::app_loop::EventLoopContext;

pub fn execute_command(input: String, event_loop_context: &EventLoopContext) {
    let command_registry = Arc::clone(&event_loop_context.system_resources.command_registry);
    let conversation = Arc::clone(&event_loop_context.conversation_state.conversation);
    let tool_registry = Arc::clone(&event_loop_context.system_resources.tool_registry);
    let agent_manager = Arc::clone(&event_loop_context.system_resources.agent_manager);
    let working_dir = event_loop_context.runtime.working_dir.clone();
    let event_tx = event_loop_context.channels.event_tx.clone();
    let permission_manager = Arc::clone(&event_loop_context.runtime.permission_manager);
    let current_agent_name = event_loop_context
        .conversation_state
        .current_agent_name
        .clone();
    let config = event_loop_context.runtime.config.clone();
    let backend = Arc::clone(&event_loop_context.system_resources.backend);
    let context_manager = Arc::clone(&event_loop_context.conversation_state.context_manager);
    let tool_executor = Arc::clone(&event_loop_context.system_resources.tool_executor);
    let system_reminder = Arc::clone(&event_loop_context.system_resources.system_reminder);

    tokio::spawn(async move {
        let mut context = CommandContext::new()
            .with_conversation(Arc::clone(&conversation))
            .with_tool_registry(Arc::clone(&tool_registry))
            .with_agent_manager(agent_manager)
            .with_command_registry(command_registry.clone())
            .with_working_directory(working_dir)
            .with_permission_manager(permission_manager)
            .with_current_agent_name(current_agent_name)
            .with_event_sender(event_tx.clone())
            .with_config(config)
            .with_backend(Arc::clone(&backend))
            .with_context_manager(Arc::clone(&context_manager));

        match command_registry.execute(&input, &mut context).await {
            Ok(CommandResult::Success(msg)) => {
                let _ = event_tx.send(AgentEvent::FinalResponse(msg));
            }
            Ok(CommandResult::RunAgent) => {
                run_agent_on_conversation(
                    event_tx.clone(),
                    Arc::clone(&conversation),
                    Arc::clone(&backend),
                    Arc::clone(&tool_registry),
                    Arc::clone(&tool_executor),
                    Arc::clone(&context_manager),
                    Arc::clone(&system_reminder),
                )
                .await;
            }
            Ok(CommandResult::Exit) => {
                let _ = event_tx.send(AgentEvent::Exit);
            }
            Ok(CommandResult::ClearConversation) => {
                let _ = event_tx.send(AgentEvent::ClearConversation);
            }
            Err(e) => {
                let _ = event_tx.send(AgentEvent::Error(format!("Command error: {}", e)));
            }
        }
    });
}

pub fn answer(
    input: String,
    image_attachments: Vec<crate::agent::Attachment>,
    event_loop_context: &EventLoopContext,
) -> JoinHandle<()> {
    let parser = Arc::clone(&event_loop_context.system_resources.parser);
    let conversation = Arc::clone(&event_loop_context.conversation_state.conversation);
    let backend = Arc::clone(&event_loop_context.system_resources.backend);
    let tool_registry = Arc::clone(&event_loop_context.system_resources.tool_registry);
    let tool_executor = Arc::clone(&event_loop_context.system_resources.tool_executor);
    let system_reminder = Arc::clone(&event_loop_context.system_resources.system_reminder);
    let event_tx = event_loop_context.channels.event_tx.clone();
    let context_manager = Arc::clone(&event_loop_context.conversation_state.context_manager);
    let memory_manager = event_loop_context
        .runtime
        .memory_mode_manager
        .as_ref()
        .map(Arc::clone);

    tokio::spawn(async move {
        let turn_start = SystemTime::now();
        let (expanded_input, mut attachments) = parser
            .expand_message_with_attachments(&input)
            .await
            .unwrap_or_else(|_| (input, Vec::new()));
        attachments.extend(image_attachments);

        let mut conv = conversation.lock().await;

        if let Some(ref manager) = memory_manager {
            if manager.summary_written_since_last_turn() {
                let n_before = conv.messages.len();
                conv.clear_turn_history();
                let cleared = n_before.saturating_sub(conv.messages.len());
                crate::console::console().debug(&format!(
                    "Memory mode: cleared {} messages from prior turn",
                    cleared
                ));
            }

            let summary = manager.read_summary();
            let content = match summary {
                Some(ref s) => format!("{}\n\n## Session Memory\n\n{}", manager.instructions, s),
                None => manager.instructions.clone(),
            };
            conv.add_system_message(content);
        }

        conv.add_user_message_with_attachments(expanded_input.clone(), attachments);

        let agent = Agent::new(backend, tool_registry, tool_executor)
            .with_event_sender(event_tx.clone())
            .with_context_manager(context_manager)
            .with_system_reminder(system_reminder);

        // Error is already sent as AgentEvent::Error from within handle_turn
        let _ = agent.handle_turn(&mut conv).await;

        if let Some(ref manager) = memory_manager {
            manager.record_turn_end(turn_start);
        }
    })
}

pub async fn run_agent_on_conversation(
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    conversation: Arc<tokio::sync::Mutex<Conversation>>,
    backend: Arc<dyn LlmBackend>,
    tool_registry: Arc<ToolRegistry>,
    tool_executor: Arc<ToolExecutor>,
    context_manager: Arc<ContextManager>,
    system_reminder: Arc<crate::system_reminders::SystemReminder>,
) {
    let agent = Agent::new(backend, tool_registry, tool_executor)
        .with_event_sender(event_tx.clone())
        .with_context_manager(context_manager)
        .with_system_reminder(system_reminder);

    let mut conv = conversation.lock().await;
    let _ = agent.handle_turn(&mut conv).await;
}
