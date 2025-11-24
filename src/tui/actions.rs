use std::sync::Arc;
use tokio::task::JoinHandle;

use crate::agent::{Agent, AgentEvent};
use crate::commands::{CommandContext, CommandResult};
use crate::system_reminders::{PeriodicCoreReminderStrategy, SystemReminder};
use crate::tui::app_loop::EventLoopContext;

pub fn execute_command(input: String, event_loop_context: &crate::tui::app_loop::EventLoopContext) {
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
    let context_manager = Arc::clone(&event_loop_context.conversation_state.context_manager);

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
            .with_backend(backend)
            .with_context_manager(context_manager);

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

pub fn answer(input: String, event_loop_context: &EventLoopContext) -> JoinHandle<()> {
    let parser = Arc::clone(&event_loop_context.system_resources.parser);
    let conversation = Arc::clone(&event_loop_context.conversation_state.conversation);
    let backend = Arc::clone(&event_loop_context.system_resources.backend);
    let tool_registry = Arc::clone(&event_loop_context.system_resources.tool_registry);
    let tool_executor = Arc::clone(&event_loop_context.system_resources.tool_executor);
    let event_tx = event_loop_context.channels.event_tx.clone();
    let context_manager = Arc::clone(&event_loop_context.conversation_state.context_manager);
    let todo_state = event_loop_context.runtime.todo_state.clone();

    tokio::spawn(async move {
        let expanded_input = parser.expand_message(&input).await.unwrap_or(input);

        {
            let mut conv = conversation.lock().await;
            conv.add_user_message(expanded_input.clone());

            // Inject todo state as a system reminder in the user message
            if let Some(todo_reminder) = todo_state.format_for_llm().await {
                // Add the todo reminder as part of the user's message context
                let last_idx = conv.messages.len() - 1;
                if let Some(msg) = conv.messages.get_mut(last_idx)
                    && let Some(content) = &mut msg.content
                {
                    content.push_str("\n\n");
                    content.push_str(&todo_reminder);
                }
            }
        }

        let mut conv = conversation.lock().await;

        // Create system reminders
        let core_instructions =
            "Focus on completing the task efficiently. Remember to check your progress regularly."
                .to_string();
        let reminder_strategy = Box::new(PeriodicCoreReminderStrategy::new(10, core_instructions));
        let system_reminder = Arc::new(SystemReminder::new().add_strategy(reminder_strategy));

        let agent = Agent::new(backend, tool_registry, tool_executor)
            .with_event_sender(event_tx.clone())
            .with_context_manager(context_manager)
            .with_system_reminder(system_reminder);

        // Error is already sent as AgentEvent::Error from within handle_turn
        let _ = agent.handle_turn(&mut conv).await;
    })
}
