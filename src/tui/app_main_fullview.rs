use crate::session::AgentSession;
use crate::tui::app_loop_fullview::run_event_loop;
use crate::tui::terminal::lifecycle_fullview::{init_terminal_fullview, restore_terminal_fullview};

pub async fn run_with_session_fullview(mut session: AgentSession) -> anyhow::Result<()> {
    let mut terminal = init_terminal_fullview()?;

    let terminal = match terminal.clear() {
        Ok(_) => terminal,
        Err(e) => {
            restore_terminal_fullview(terminal)?;
            return Err(e.into());
        }
    };

    let terminal =
        run_event_loop(terminal, &mut session.app_state, session.event_loop_context).await?;

    let _ = session.app_state.prompt_history.save();
    restore_terminal_fullview(terminal)?;

    Ok(())
}
