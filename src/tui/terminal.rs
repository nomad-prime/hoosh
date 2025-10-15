use anyhow::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::{backend::CrosstermBackend, Terminal, TerminalOptions, Viewport};
use std::io;

pub type Tui = Terminal<CrosstermBackend<io::Stdout>>;

pub fn init_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let stdout = io::stdout();

    let viewport_height = 18;

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(viewport_height),
        },
    )?;
    Ok(terminal)
}

pub fn restore_terminal(mut terminal: Tui) -> Result<()> {
    disable_raw_mode()?;
    terminal.show_cursor()?;
    Ok(())
}
