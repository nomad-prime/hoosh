use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::agents::AgentManager;
use crate::backends::LlmBackend;
use crate::commands::{CommandContext, CommandRegistry, CommandResult};
use crate::conversations::{AgentEvent, Conversation, ConversationHandler};
use crate::parser::MessageParser;
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;

pub fn execute_command(
    input: String,
    command_registry: Arc<CommandRegistry>,
    conversation: Arc<Mutex<Conversation>>,
    tool_registry: Arc<ToolRegistry>,
    agent_manager: Arc<AgentManager>,
    working_dir: String,
    event_tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
    permission_manager: Arc<crate::permissions::PermissionManager>,
) {
    tokio::spawn(async move {
        let mut context = CommandContext::new()
            .with_conversation(conversation)
            .with_tool_registry(tool_registry)
            .with_agent_manager(agent_manager)
            .with_command_registry(Arc::clone(&command_registry))
            .with_working_directory(working_dir)
            .with_permission_manager(permission_manager);

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
    parser: Arc<MessageParser>,
    conversation: Arc<Mutex<Conversation>>,
    backend: Arc<dyn LlmBackend>,
    tool_registry: Arc<ToolRegistry>,
    tool_executor: Arc<ToolExecutor>,
    event_tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let expanded_input = match parser.expand_message(&input).await {
            Ok(expanded) => expanded,
            Err(_) => input,
        };

        {
            let mut conv = conversation.lock().await;
            conv.add_user_message(expanded_input);
        }

        let mut conv = conversation.lock().await;
        let handler = ConversationHandler::new(backend, tool_registry, tool_executor)
            .with_event_sender(event_tx);

        if let Err(e) = handler.handle_turn(&mut conv).await {
            eprintln!("Error handling turn: {}", e);
        }
    })
}
