use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::{Agent, AgentEvent, Conversation, FileMention, PendingToolCall};
use crate::backends::LlmBackend;
use crate::commands::{CommandContext, CommandResult};
use crate::context_management::ContextManager;
use crate::tool_executor::ToolExecutor;
use crate::tools::{ToolRegistry, ToolRender};
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
        let mut expanded =
            parser
                .expand(&input)
                .await
                .unwrap_or_else(|_| crate::parser::ExpandedMessage {
                    text: input,
                    ..Default::default()
                });
        expanded.attachments.extend(image_attachments);

        emit_mention_events(&expanded.mentions, &tool_registry, &event_tx);

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

        conv.add_user_message_with_file_mentions(
            expanded.text,
            expanded.attachments,
            expanded.mentions,
        );

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

fn emit_mention_events(
    mentions: &[FileMention],
    tool_registry: &ToolRegistry,
    event_tx: &mpsc::UnboundedSender<AgentEvent>,
) {
    if mentions.is_empty() {
        return;
    }

    let mut pending = Vec::new();
    let mut results = Vec::new();

    for mention in mentions {
        let id = format!("mention_{}", uuid::Uuid::new_v4());
        let tool_name = mention.tool_name();
        let args = mention.tool_args();
        let tool = tool_registry.get_tool(tool_name);

        let display_name = tool
            .map(|t| t.format_call_display(&args))
            .unwrap_or_else(|| mention.display_name().to_string());
        let render = tool
            .map(|t| t.render_strategy())
            .unwrap_or(ToolRender::Standard);
        let phrasing = tool
            .map(|t| t.phrasing())
            .unwrap_or(crate::tools::phrasing::GENERIC);
        let summary = match mention.result() {
            Ok(output) => tool
                .map(|t| t.result_summary(output))
                .unwrap_or_else(|| "Done".to_string()),
            Err(err) => format!("Error: {}", err),
        };

        pending.push(PendingToolCall {
            id: id.clone(),
            display_name,
            render,
            phrasing,
        });
        results.push((id, tool_name.to_string(), summary));
    }

    let _ = event_tx.send(AgentEvent::ToolCalls(pending));
    for (id, tool_name, summary) in results {
        let _ = event_tx.send(AgentEvent::ToolExecutionStarted {
            tool_call_id: id.clone(),
            tool_name: tool_name.clone(),
        });
        let _ = event_tx.send(AgentEvent::ToolResult {
            tool_call_id: id.clone(),
            tool_name: tool_name.clone(),
            summary,
        });
        let _ = event_tx.send(AgentEvent::ToolExecutionCompleted {
            tool_call_id: id,
            tool_name,
        });
    }
    let _ = event_tx.send(AgentEvent::AllToolsComplete);
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
