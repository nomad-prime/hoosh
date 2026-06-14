// Drive a single hoosh agent turn inside the shell-mode REPL.
//
// Mirrors the per-turn pattern used by tagged_mode and tui/actions.rs:
//   - lock conversation, add user message, unlock
//   - construct Agent from persistent session resources
//   - spawn handle_turn task, pump events until FinalResponse / Error / Exit
//   - render assistant output to the terminal

use anyhow::Result;
use std::io;
use tokio::sync::mpsc::UnboundedSender;

use crate::agent::{Agent, AgentEvent};
use crate::console::console;
use crate::terminal_spinner::TerminalSpinner;
use crate::tui::app_loop::EventLoopContext;

pub struct TurnDriver {
    ctx: EventLoopContext,
    permission_response_tx: UnboundedSender<crate::agent::PermissionResponse>,
    approval_response_tx: UnboundedSender<crate::agent::ApprovalResponse>,
}

impl TurnDriver {
    pub fn new(
        ctx: EventLoopContext,
        permission_response_tx: UnboundedSender<crate::agent::PermissionResponse>,
        approval_response_tx: UnboundedSender<crate::agent::ApprovalResponse>,
    ) -> Self {
        Self {
            ctx,
            permission_response_tx,
            approval_response_tx,
        }
    }

    pub async fn run_turn(&mut self, user_input: String) -> Result<()> {
        let conversation = self.ctx.conversation_state.conversation.clone();

        {
            let mut conv = conversation.lock().await;
            conv.add_user_message_with_attachments(user_input, Vec::new());
        }

        let agent = Agent::new(
            self.ctx.system_resources.backend.clone(),
            self.ctx.system_resources.tool_registry.clone(),
            self.ctx.system_resources.tool_executor.clone(),
        )
        .with_event_sender(self.ctx.channels.event_tx.clone())
        .with_context_manager(self.ctx.conversation_state.context_manager.clone())
        .with_system_reminder(self.ctx.system_resources.system_reminder.clone());

        let conversation_clone = conversation.clone();
        let mut agent_handle = tokio::spawn(async move {
            let mut conv = conversation_clone.lock().await;
            agent.handle_turn(&mut conv).await
        });

        let mut spinner = TerminalSpinner::new("Processing");
        console().newline();
        spinner.start();

        let event_rx = &mut self.ctx.channels.event_rx;
        let mut response_content = String::new();
        let mut interrupted = false;

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    spinner.stop();
                    console().newline();
                    console().warning("Interrupted");
                    interrupted = true;
                    break;
                }
                Some(event) = event_rx.recv() => {
                    match event {
                        AgentEvent::Thinking => {
                            spinner.update_message("Thinking");
                        }
                        AgentEvent::AssistantThought(thought) => {
                            spinner.stop();
                            console().newline();
                            console().markdown(&thought);
                            console().newline();
                            spinner.start();
                        }
                        AgentEvent::ToolExecutionStarted { tool_name, .. } => {
                            spinner.update_message(format!("Executing: {}", tool_name));
                        }
                        AgentEvent::ToolPermissionRequest { descriptor, request_id } => {
                            spinner.stop();
                            if let Some((allowed, scope)) = prompt_permission(&descriptor)? {
                                let _ = self.permission_response_tx.send(crate::agent::PermissionResponse {
                                    request_id,
                                    allowed,
                                    scope,
                                });
                            }
                            console().newline();
                            spinner.start();
                        }
                        AgentEvent::ToolPreview { preview } => {
                            spinner.stop();
                            console().newline();
                            console().plain(&preview);
                            console().newline();
                            spinner.start();
                        }
                        AgentEvent::ApprovalRequest { tool_call_id, .. } => {
                            spinner.stop();
                            if let Some((approved, rejection_reason)) = prompt_approval()? {
                                let _ = self.approval_response_tx.send(crate::agent::ApprovalResponse {
                                    tool_call_id,
                                    approved,
                                    rejection_reason,
                                });
                            }
                            console().newline();
                            spinner.start();
                        }
                        AgentEvent::FinalResponse(content) => {
                            response_content = content;
                            spinner.stop();
                            break;
                        }
                        AgentEvent::Error(err) => {
                            spinner.stop();
                            console().newline();
                            console().error(&err);
                            break;
                        }
                        AgentEvent::Exit => {
                            spinner.stop();
                            break;
                        }
                        _ => {}
                    }
                }
                result = &mut agent_handle => {
                    spinner.stop();
                    if let Err(e) = result {
                        console().newline();
                        console().error(&format!("Agent execution failed: {}", e));
                    }
                    break;
                }
            }
        }

        if !interrupted && !response_content.is_empty() {
            console().newline();
            console().markdown(&response_content);
            console().newline();
        }

        Ok(())
    }
}

fn prompt_permission(
    descriptor: &crate::permissions::ToolPermissionDescriptor,
) -> Result<Option<(bool, Option<crate::permissions::PermissionScope>)>> {
    use std::io::BufRead;

    console().newline();
    console().warning(&format!(
        "Permission required: {} {}",
        descriptor.kind(),
        descriptor.target()
    ));
    console().plain("  y = yes (once), n = no, a = always for this, t = trust project");
    console().prompt("Allow? (y/n/a/t): ");

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    match input.as_str() {
        "y" | "yes" => Ok(Some((true, None))),
        "n" | "no" => Ok(Some((false, None))),
        "a" | "always" => {
            let target = descriptor.target().to_string();
            Ok(Some((
                true,
                Some(crate::permissions::PermissionScope::Specific(target)),
            )))
        }
        "t" | "trust" => {
            if let Ok(current_dir) = std::env::current_dir() {
                Ok(Some((
                    true,
                    Some(crate::permissions::PermissionScope::ProjectWide(
                        current_dir,
                    )),
                )))
            } else {
                Ok(Some((false, None)))
            }
        }
        _ => Ok(Some((false, None))),
    }
}

fn prompt_approval() -> Result<Option<(bool, Option<String>)>> {
    use std::io::BufRead;

    console().newline();
    console().prompt("Approve? (y/n): ");

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    match input.as_str() {
        "y" | "yes" => Ok(Some((true, None))),
        "n" | "no" => Ok(Some((false, Some("User rejected".to_string())))),
        _ => Ok(Some((false, Some("Invalid input".to_string())))),
    }
}
