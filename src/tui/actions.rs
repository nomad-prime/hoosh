use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::agents::AgentManager;
use crate::backends::LlmBackend;
use crate::commands::{CommandContext, CommandRegistry, CommandResult};
use crate::config::AppConfig;
use crate::conversations::{AgentEvent, Conversation, ConversationHandler, MessageSummarizer};
use crate::parser::MessageParser;
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;

pub struct CommandExecutionContext {
    pub input: String,
    pub command_registry: Arc<CommandRegistry>,
    pub conversation: Arc<Mutex<Conversation>>,
    pub tool_registry: Arc<ToolRegistry>,
    pub agent_manager: Arc<AgentManager>,
    pub working_dir: String,
    pub event_tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
    pub permission_manager: Arc<crate::permissions::PermissionManager>,
    pub summarizer: Arc<MessageSummarizer>,
    pub current_agent_name: String,
    pub config: AppConfig,
    pub backend: Arc<dyn LlmBackend>,
}

pub fn execute_command(ctx: CommandExecutionContext) {
    tokio::spawn(async move {
        let mut context = CommandContext::new()
            .with_conversation(ctx.conversation)
            .with_tool_registry(ctx.tool_registry)
            .with_agent_manager(ctx.agent_manager)
            .with_command_registry(Arc::clone(&ctx.command_registry))
            .with_working_directory(ctx.working_dir)
            .with_permission_manager(ctx.permission_manager)
            .with_summarizer(ctx.summarizer)
            .with_current_agent_name(ctx.current_agent_name)
            .with_event_sender(ctx.event_tx.clone())
            .with_config(ctx.config)
            .with_backend(ctx.backend);

        match ctx.command_registry.execute(&ctx.input, &mut context).await {
            Ok(CommandResult::Success(msg)) => {
                let _ = ctx.event_tx.send(AgentEvent::FinalResponse(msg));
            }
            Ok(CommandResult::Exit) => {
                let _ = ctx.event_tx.send(AgentEvent::Exit);
            }
            Ok(CommandResult::ClearConversation) => {
                let _ = ctx.event_tx.send(AgentEvent::ClearConversation);
            }
            Err(e) => {
                let _ = ctx
                    .event_tx
                    .send(AgentEvent::Error(format!("Command error: {}", e)));
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
        let expanded_input = parser
            .expand_message(&input)
            .await
            .unwrap_or_else(|_| input);

        {
            let mut conv = conversation.lock().await;
            conv.add_user_message(expanded_input);
        }

        let mut conv = conversation.lock().await;
        let handler = ConversationHandler::new(backend, tool_registry, tool_executor)
            .with_event_sender(event_tx);

        let _ = handler.handle_turn(&mut conv).await;
    })
}
