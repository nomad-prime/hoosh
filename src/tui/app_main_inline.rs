use crate::session::AgentSession;
use crate::tui::app_loop_inline::run_event_loop;
use crate::tui::terminal::lifecycle_inline::{init_terminal_inline, restore_terminal_inline};

pub async fn run_with_session_inline(mut session: AgentSession) -> anyhow::Result<()> {
    let mut terminal = init_terminal_inline()?;

    let terminal = match terminal.clear() {
        Ok(_) => terminal,
        Err(e) => {
            restore_terminal_inline(terminal)?;
            return Err(e.into());
        }
    };

    let terminal =
        run_event_loop(terminal, &mut session.app_state, session.event_loop_context).await?;

    let _ = session.app_state.prompt_history.save();
    restore_terminal_inline(terminal)?;

    Ok(())
}
