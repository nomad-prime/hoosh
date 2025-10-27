use std::sync::Arc;
use tokio::task::JoinHandle;

use crate::commands::{CommandContext, CommandResult};
use crate::conversations::{AgentEvent, ConversationHandler};

pub fn execute_command(
    input: String,
    event_loop_context: &crate::tui::event_loop::EventLoopContext,
) {
    let command_registry = Arc::clone(&event_loop_context.system_resources.command_registry);
    let conversation = Arc::clone(&event_loop_context.conversation_state.conversation);
    let tool_registry = Arc::clone(&event_loop_context.system_resources.tool_registry);
    let agent_manager = Arc::clone(&event_loop_context.system_resources.agent_manager);
    let working_dir = event_loop_context.runtime.working_dir.clone();
    let event_tx = event_loop_context.channels.event_tx.clone();
    let permission_manager = Arc::clone(&event_loop_context.runtime.permission_manager);
    let summarizer = Arc::clone(&event_loop_context.conversation_state.summarizer);
    let current_agent_name = event_loop_context
        .conversation_state
        .current_agent_name
        .clone();
    let config = event_loop_context.runtime.config.clone();
    let backend = Arc::clone(&event_loop_context.system_resources.backend);

    tokio::spawn(async move {
        let mut context = CommandContext::new()
            .with_conversation(conversation)
            .with_tool_registry(tool_registry)
            .with_agent_manager(agent_manager)
            .with_command_registry(command_registry.clone())
            .with_working_directory(working_dir)
            .with_permission_manager(permission_manager)
            .with_summarizer(summarizer)
            .with_current_agent_name(current_agent_name)
            .with_event_sender(event_tx.clone())
            .with_config(config)
            .with_backend(backend);

        match command_registry.execute(&input, &mut context).await {
            Ok(CommandResult::Success(msg)) => {
                let _ = event_tx.send(AgentEvent::FinalResponse(msg));
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

pub fn start_agent_conversation(
    input: String,
    event_loop_context: &crate::tui::event_loop::EventLoopContext,
) -> JoinHandle<()> {
    let parser = Arc::clone(&event_loop_context.system_resources.parser);
    let conversation = Arc::clone(&event_loop_context.conversation_state.conversation);
    let backend = Arc::clone(&event_loop_context.system_resources.backend);
    let tool_registry = Arc::clone(&event_loop_context.system_resources.tool_registry);
    let tool_executor = Arc::clone(&event_loop_context.system_resources.tool_executor);
    let event_tx = event_loop_context.channels.event_tx.clone();
    let context_manager = Arc::clone(&event_loop_context.conversation_state.context_manager);

    tokio::spawn(async move {
        let expanded_input = parser.expand_message(&input).await.unwrap_or(input);

        {
            let mut conv = conversation.lock().await;
            conv.add_user_message(expanded_input);
        }

        let mut conv = conversation.lock().await;
        let handler = ConversationHandler::new(backend, tool_registry, tool_executor)
            .with_event_sender(event_tx)
            .with_context_manager(context_manager);

        let _ = handler.handle_turn(&mut conv).await;
    })
}
