use crate::session::AgentSession;
use crate::tui::app_loop::run_event_loop;
use crate::tui::terminal::{init_terminal, restore_terminal};

pub async fn run_with_session(mut session: AgentSession) -> anyhow::Result<()> {
    let mut terminal = init_terminal()?;

    // Clear terminal
    let terminal = match terminal.clear() {
        Ok(_) => terminal,
        Err(e) => {
            restore_terminal(terminal)?;
            return Err(e.into());
        }
    };

    // Run the event loop
    let terminal =
        run_event_loop(terminal, &mut session.app_state, session.event_loop_context).await?;

    // Save history and restore terminal
    let _ = session.app_state.prompt_history.save();
    restore_terminal(terminal)?;

    Ok(())
}
