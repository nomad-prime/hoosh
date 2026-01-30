// Tagged mode runner - terminal-native output without TUI
//
// This module implements the "tagged" terminal mode, which provides:
// - Non-hijacking terminal interaction (returns control to shell)
// - Terminal-native output (no TUI, uses stdout/stderr)
// - Session file persistence for context across @hoosh invocations
// - Simple text-based prompts for permissions

use anyhow::{Context, Result};
use std::io::{self, Write};

use crate::agent::{Agent, AgentEvent};
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
pub async fn run_tagged_mode(session: AgentSession, message: Option<String>) -> Result<()> {
    let AgentSession {
        event_loop_context, ..
    } = session;

    // Get terminal PID for session file lookup
    let terminal_pid = get_terminal_pid()?;

    // Load existing session file if exists
    let mut session_file = match SessionFile::load(terminal_pid)? {
        Some(mut file) => {
            file.touch();
            file
        }
        None => {
            SessionFile::new(terminal_pid)
        }
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
                    eprintln!("⚠️  Warning: Failed to deserialize message: {}", e);
                }
            }
        }
    }

    // Get input: either from CLI argument or prompt via stdin
    let input = if let Some(msg) = message {
        // Non-interactive mode: use provided message
        msg
    } else {
        // Interactive mode: prompt for input
        eprint!("🤖 > ");
        io::stderr().flush()?;

        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .context("Failed to read input")?;

        line.trim().to_string()
    };

    if input.is_empty() {
        eprintln!("No input provided");
        return Ok(());
    }

    // Expand message with parser
    let expanded_input = event_loop_context
        .system_resources
        .parser
        .expand_message(&input)
        .await
        .unwrap_or_else(|_| input.to_string());

    // Add user message to conversation
    {
        let mut conv = conversation.lock().await;
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

    // Start spinner
    let mut spinner = TerminalSpinner::new("Processing");
    spinner.start();

    // Execute agent in background
    let conversation_clone = conversation.clone();
    let mut agent_handle = tokio::spawn(async move {
        let mut conv = conversation_clone.lock().await;
        agent.handle_turn(&mut conv).await
    });

    // Listen for events and display output
    let mut event_rx = event_loop_context.channels.event_rx;
    let mut response_content = String::new();

    loop {
        tokio::select! {
            Some(event) = event_rx.recv() => {
                match event {
                    AgentEvent::Thinking => {
                        spinner.update_message("Thinking");
                    }
                    AgentEvent::AssistantThought(thought) => {
                        spinner.update_message(format!("Thinking: {}", thought));
                    }
                    AgentEvent::ToolExecutionStarted { tool_name, .. } => {
                        spinner.update_message(format!("Executing: {}", tool_name));
                    }
                    AgentEvent::FinalResponse(content) => {
                        response_content = content;
                        spinner.stop();
                        break;
                    }
                    AgentEvent::Error(err) => {
                        spinner.stop();
                        eprintln!("\n❌ Error: {}", err);
                        return Ok(());
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
                    eprintln!("\n❌ Agent execution failed: {}", e);
                    return Err(e.into());
                }
                break;
            }
        }
    }

    // Display response
    eprintln!(); // Clear spinner line
    println!("{}", response_content);

    // Save session file with updated messages
    {
        let conv = conversation.lock().await;
        session_file.messages = conv
            .messages
            .iter()
            .filter_map(|msg| serde_json::to_value(msg).ok())
            .collect();
    }

    if let Err(e) = session_file.save() {
        eprintln!("⚠️  Warning: Failed to save session: {}", e);
    }

    Ok(())
}
