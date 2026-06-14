// Shell mode: hoosh owns the prompt; bash runs as the fast path.
//
// Each line the user types is first executed via `bash -c` inside a PTY.
// Exit code routes what happens next:
//   - 0                     → continue
//   - 127 (cmd not found)   → route input to the hoosh agent as prose
//   - other non-zero        → offer a one-key '? for hoosh' hint
//
// This is a separate REPL from inline/fullview/tagged modes. It builds an
// `AgentSession` the same way tagged mode does and drives `Agent::handle_turn`
// per hoosh dispatch. No new agent surface, no edits to existing modes.

mod dispatcher;
mod hoosh_turn;
mod pty_runner;
mod ring_buffer;

pub use dispatcher::{DispatchAction, classify};

use anyhow::{Context, Result};
use std::io::{self, BufRead, Write};

use crate::console::console;
use crate::session::AgentSession;

const OUTPUT_TAIL_BYTES: usize = 4096;

pub async fn run_shell_mode(session: AgentSession) -> Result<()> {
    let AgentSession {
        event_loop_context, ..
    } = session;

    let permission_response_tx = event_loop_context
        .tagged_mode_channels
        .permission_response_tx
        .clone();
    let approval_response_tx = event_loop_context
        .tagged_mode_channels
        .approval_response_tx
        .clone();

    let mut ctx = hoosh_turn::TurnDriver::new(
        event_loop_context,
        permission_response_tx,
        approval_response_tx,
    );

    let mut pty = pty_runner::PtySession::spawn().context("Failed to start shell session")?;

    console()
        .plain("hoosh shell — your $SHELL runs commands; hoosh handles prose. Ctrl-D to exit.");

    loop {
        print_prompt()?;
        let line = match read_line()? {
            Some(s) => s,
            None => {
                console().newline();
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "exit" || trimmed == "quit" {
            break;
        }

        if let Ok((cols, rows)) = crossterm::terminal::size() {
            pty.resize(cols, rows);
        }

        let outcome = pty
            .run_command(trimmed, OUTPUT_TAIL_BYTES)
            .context("Failed to run command in shell")?;

        match classify(outcome.exit_code) {
            DispatchAction::Continue => {}
            DispatchAction::RouteToAgent => {
                ctx.run_turn(prose_prompt(trimmed)).await?;
            }
            DispatchAction::OfferHint => {
                if prompt_hint_keypress()? {
                    let prompt = failure_prompt(trimmed, outcome.exit_code, &outcome.output_tail);
                    ctx.run_turn(prompt).await?;
                }
            }
        }
    }

    Ok(())
}

fn print_prompt() -> Result<()> {
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "~".to_string());
    let mut stdout = io::stdout();
    write!(stdout, "{} ❯ ", cwd)?;
    stdout.flush()?;
    Ok(())
}

fn read_line() -> Result<Option<String>> {
    let stdin = io::stdin();
    let mut line = String::new();
    let n = stdin.lock().read_line(&mut line)?;
    if n == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim_end_matches('\n').to_string()))
}

fn prompt_hint_keypress() -> Result<bool> {
    use crossterm::event::{Event, KeyCode, read};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    let mut stdout = io::stdout();
    write!(stdout, "  ⎿ press ? for hoosh, any other key to dismiss ")?;
    stdout.flush()?;

    enable_raw_mode()?;
    let pressed = loop {
        match read()? {
            Event::Key(k) => break k.code,
            _ => continue,
        }
    };
    disable_raw_mode()?;

    writeln!(stdout)?;
    stdout.flush()?;

    Ok(matches!(pressed, KeyCode::Char('?')))
}

fn prose_prompt(line: &str) -> String {
    format!(
        "The user typed the following at a hoosh shell prompt and it was not a recognized shell command (exit 127). Treat it as a natural-language request and help. If acting in this directory is appropriate, do so.\n\nUser input:\n{}",
        line
    )
}

fn failure_prompt(line: &str, exit_code: Option<i32>, output_tail: &str) -> String {
    let exit_str = exit_code
        .map(|c| c.to_string())
        .unwrap_or_else(|| "signal".to_string());
    format!(
        "The user ran the following shell command in hoosh shell and it failed (exit {}). They asked for help.\n\nCommand:\n{}\n\nLast output (stdout+stderr tail):\n{}",
        exit_str,
        line,
        output_tail.trim()
    )
}
