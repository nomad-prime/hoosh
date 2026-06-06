// Tagged mode runner - terminal-native output without TUI
//
// This module implements the "tagged" terminal mode, which provides:
// - Non-hijacking terminal interaction (returns control to shell)
// - Terminal-native output (no TUI, uses stdout/stderr)
// - Session file persistence for context across @hoosh invocations
// - Simple text-based prompts for permissions

use anyhow::{Context, Result};
use std::io::{self, IsTerminal, Read};
use std::time::SystemTime;

use crate::agent::{Agent, AgentEvent};
use crate::console::console;
use crate::output_format::OutputFormat;
use crate::session::AgentSession;
use crate::session_files::store::{SessionFile, get_terminal_pid};
use crate::terminal_spinner::TerminalSpinner;

/// Run agent in tagged mode (terminal-native, no TUI)
///
/// Tagged mode characteristics:
/// - No TUI initialization - direct stdout/stderr output
/// - Braille spinner for progress indication
/// - Text-based permission prompts
/// - Session file loaded/saved for context persistence
/// - Returns control to shell immediately after completion
///
/// # Arguments
/// * `session` - Initialized agent session
/// * `message` - Optional message text. If None, prompts for input via stdin (interactive mode)
/// * `permission_response_tx` - Channel to send permission responses
/// * `approval_response_tx` - Channel to send approval responses
pub async fn run_tagged_mode(
    session: AgentSession,
    message: Option<String>,
    output_format: OutputFormat,
    permission_response_tx: tokio::sync::mpsc::UnboundedSender<crate::agent::PermissionResponse>,
    approval_response_tx: tokio::sync::mpsc::UnboundedSender<crate::agent::ApprovalResponse>,
) -> Result<()> {
    let AgentSession {
        event_loop_context, ..
    } = session;
    let json_mode = output_format == OutputFormat::Json;

    let backend_name = event_loop_context
        .system_resources
        .backend
        .backend_name()
        .to_string();
    let model_name = event_loop_context
        .system_resources
        .backend
        .model_name()
        .to_string();

    // Get terminal PID for session file lookup
    let terminal_pid = get_terminal_pid()?;

    // Load existing session file if exists
    let mut session_file = match SessionFile::load(terminal_pid)? {
        Some(mut file) => {
            file.touch();
            file
        }
        None => SessionFile::new(terminal_pid),
    };

    // Load existing messages into conversation
    let conversation = event_loop_context.conversation_state.conversation.clone();
    {
        let mut conv = conversation.lock().await;
        for msg_value in &session_file.messages {
            match serde_json::from_value(msg_value.clone()) {
                Ok(msg) => {
                    conv.messages.push(msg);
                }
                Err(e) => {
                    console().warning(&format!("Failed to deserialize message: {}", e));
                }
            }
        }
    }

    // Get input: handle piped stdin, CLI argument, or interactive prompt
    let stdin = io::stdin();
    let stdin_is_piped = !stdin.is_terminal();

    // Read piped input if available
    let piped_content = if stdin_is_piped {
        let mut buffer = String::new();
        stdin
            .lock()
            .read_to_string(&mut buffer)
            .context("Failed to read piped input")?;
        Some(buffer)
    } else {
        None
    };

    // Combine piped input with message argument
    let input = match (piped_content, message) {
        (Some(piped), Some(msg)) => {
            // Both piped input and message: combine them
            format!("{}\n\n{}", piped, msg)
        }
        (Some(piped), None) => {
            // Only piped input
            piped
        }
        (None, Some(msg)) => {
            // Only message argument
            msg
        }
        (None, None) => {
            // Interactive mode: prompt for input
            console().prompt("🤖 > ");

            let mut line = String::new();
            io::stdin()
                .read_line(&mut line)
                .context("Failed to read input")?;

            line.trim().to_string()
        }
    };

    if input.trim().is_empty() {
        console().error("No input provided");
        return Ok(());
    }

    // Expand message with parser
    let expanded_input = event_loop_context
        .system_resources
        .parser
        .expand_message(&input)
        .await
        .unwrap_or_else(|_| input.to_string());

    let memory_manager = event_loop_context
        .runtime
        .memory_mode_manager
        .as_ref()
        .map(std::sync::Arc::clone);

    // Add user message to conversation (with optional memory injection)
    let turn_start = SystemTime::now();
    {
        let mut conv = conversation.lock().await;

        if let Some(ref manager) = memory_manager {
            if manager.summary_written_since_last_turn() {
                let n_before = conv.messages.len();
                conv.clear_turn_history();
                let cleared = n_before.saturating_sub(conv.messages.len());
                console().debug(&format!(
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

        conv.add_user_message(expanded_input.clone());
    }

    // Create agent
    let agent = Agent::new(
        event_loop_context.system_resources.backend.clone(),
        event_loop_context.system_resources.tool_registry.clone(),
        event_loop_context.system_resources.tool_executor.clone(),
    )
    .with_event_sender(event_loop_context.channels.event_tx.clone())
    .with_context_manager(
        event_loop_context
            .conversation_state
            .context_manager
            .clone(),
    )
    .with_system_reminder(event_loop_context.system_resources.system_reminder.clone());

    // Start spinner (text mode only)
    let mut spinner = TerminalSpinner::new("Processing");
    if !json_mode {
        console().newline();
        spinner.start();
    }

    // Execute agent in background
    let conversation_clone = conversation.clone();
    let mut agent_handle = tokio::spawn(async move {
        let mut conv = conversation_clone.lock().await;
        agent.handle_turn(&mut conv).await
    });

    // Listen for events and display output
    let mut event_rx = event_loop_context.channels.event_rx;
    let mut response_content = String::new();
    let mut interrupted = false;
    let mut total_input_tokens: usize = 0;
    let mut total_output_tokens: usize = 0;
    let mut error_message: Option<String> = None;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                if !json_mode {
                    spinner.stop();
                    console().newline();
                    console().warning("Interrupted - saving partial context...");
                }
                interrupted = true;
                break;
            }
            Some(event) = event_rx.recv() => {
                match event {
                    AgentEvent::Thinking => {
                        if !json_mode { spinner.update_message("Thinking"); }
                    }
                    AgentEvent::AssistantThought(thought) => {
                        if !json_mode {
                            spinner.stop();
                            console().newline();
                            console().markdown(&thought);
                            console().newline();
                            spinner.start();
                        }
                    }
                    AgentEvent::ToolExecutionStarted { tool_name, .. } => {
                        if !json_mode {
                            spinner.update_message(format!("Executing: {}", tool_name));
                        }
                    }
                    AgentEvent::ToolPermissionRequest { descriptor, request_id } => {
                        if json_mode {
                            let _ = permission_response_tx.send(crate::agent::PermissionResponse {
                                request_id,
                                allowed: false,
                                scope: None,
                            });
                        } else {
                            spinner.stop();
                            if let Some((allowed, scope)) = prompt_permission(&descriptor)? {
                                let response = crate::agent::PermissionResponse {
                                    request_id,
                                    allowed,
                                    scope,
                                };
                                let _ = permission_response_tx.send(response);
                            }
                            console().newline();
                            spinner.start();
                        }
                    }
                    AgentEvent::ToolPreview { preview } => {
                        if !json_mode {
                            spinner.stop();
                            console().newline();
                            console().plain(&preview);
                            console().newline();
                            spinner.start();
                        }
                    }
                    AgentEvent::ApprovalRequest { tool_call_id, .. } => {
                        if json_mode {
                            let _ = approval_response_tx.send(crate::agent::ApprovalResponse {
                                tool_call_id,
                                approved: false,
                                rejection_reason: Some("approval not supported in --output-format json".to_string()),
                            });
                        } else {
                            spinner.stop();
                            if let Some((approved, rejection_reason)) = prompt_approval()? {
                                let response = crate::agent::ApprovalResponse {
                                    tool_call_id,
                                    approved,
                                    rejection_reason,
                                };
                                let _ = approval_response_tx.send(response);
                            }
                            console().newline();
                            spinner.start();
                        }
                    }
                    AgentEvent::TokenUsage { input_tokens, output_tokens, .. } => {
                        total_input_tokens += input_tokens;
                        total_output_tokens += output_tokens;
                    }
                    AgentEvent::FinalResponse(content) => {
                        response_content = content;
                        if !json_mode { spinner.stop(); }
                        break;
                    }
                    AgentEvent::Error(err) => {
                        if !json_mode {
                            spinner.stop();
                            console().newline();
                            console().error(&err);
                            return Ok(());
                        }
                        error_message = Some(err);
                        break;
                    }
                    AgentEvent::Exit => {
                        if !json_mode { spinner.stop(); }
                        break;
                    }
                    _ => {}
                }
            }
            result = &mut agent_handle => {
                if !json_mode { spinner.stop(); }
                if let Err(e) = result {
                    if json_mode {
                        error_message = Some(format!("Agent execution failed: {}", e));
                        break;
                    } else {
                        console().newline();
                        console().error(&format!("Agent execution failed: {}", e));
                        return Err(e.into());
                    }
                }
                break;
            }
        }
    }

    if let Some(ref manager) = memory_manager {
        manager.record_turn_end(turn_start);
    }

    // Display response (text mode only; JSON mode emits at the end)
    if !json_mode && !interrupted {
        console().newline();
        console().markdown(&response_content);
    }

    // Save session file with updated messages (including partial state on interruption)
    let storage_enabled = event_loop_context
        .runtime
        .config
        .conversation_storage
        .unwrap_or(false);
    let conv_id_for_json = {
        let conv = conversation.lock().await;
        session_file.messages = conv
            .messages
            .iter()
            .filter_map(|msg| serde_json::to_value(msg).ok())
            .collect();
        conv.id().to_string()
    };

    if let Err(e) = session_file.save() {
        if !json_mode {
            console().warning(&format!("Failed to save session: {}", e));
        }
    } else if interrupted && !json_mode {
        console().success("Partial context saved");
    }

    if json_mode {
        let session_id_value = if storage_enabled {
            serde_json::Value::String(conv_id_for_json)
        } else {
            serde_json::Value::Null
        };
        let mut out = serde_json::json!({
            "result": response_content,
            "session_id": session_id_value,
            "backend": backend_name,
            "model": model_name,
            "input_tokens": total_input_tokens,
            "output_tokens": total_output_tokens,
            "interrupted": interrupted,
        });
        if let Some(err) = error_message {
            out["error"] = serde_json::Value::String(err);
        }
        println!(
            "{}",
            serde_json::to_string(&out).unwrap_or_else(|_| "{}".to_string())
        );
    }

    Ok(())
}

/// Prompt user for permission via CLI (text-based, Linux-style)
fn prompt_permission(
    descriptor: &crate::permissions::ToolPermissionDescriptor,
) -> Result<Option<(bool, Option<crate::permissions::PermissionScope>)>> {
    use crate::console::console;

    console().newline();
    console().warning(&format!(
        "Permission required: {} {}",
        descriptor.kind(),
        descriptor.target()
    ));
    console().plain("  y = yes (once), n = no, a = always for this, t = trust project");
    console().prompt("Allow? (y/n/a/t): ");

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    match input.as_str() {
        "y" | "yes" => {
            console().success("Allowed (once)");
            Ok(Some((true, None)))
        }
        "n" | "no" => {
            console().plain("Denied");
            Ok(Some((false, None)))
        }
        "a" | "always" => {
            let target = descriptor.target().to_string();
            console().success(&format!("Allowed (always for {})", target));
            Ok(Some((
                true,
                Some(crate::permissions::PermissionScope::Specific(target)),
            )))
        }
        "t" | "trust" => {
            if let Ok(current_dir) = std::env::current_dir() {
                console().success(&format!(
                    "Trusted (project-wide: {})",
                    current_dir.display()
                ));
                Ok(Some((
                    true,
                    Some(crate::permissions::PermissionScope::ProjectWide(
                        current_dir,
                    )),
                )))
            } else {
                console().error("Could not determine current directory");
                Ok(Some((false, None)))
            }
        }
        _ => {
            console().error("Invalid input, denying permission");
            Ok(Some((false, None)))
        }
    }
}

fn prompt_approval() -> Result<Option<(bool, Option<String>)>> {
    use crate::console::console;

    console().newline();
    console().prompt("Approve? (y/n): ");

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    match input.as_str() {
        "y" | "yes" => {
            console().success("Approved");
            Ok(Some((true, None)))
        }
        "n" | "no" => {
            console().plain("Rejected");
            Ok(Some((false, Some("User rejected".to_string()))))
        }
        _ => {
            console().error("Invalid input, rejecting");
            Ok(Some((false, Some("Invalid input".to_string()))))
        }
    }
}
