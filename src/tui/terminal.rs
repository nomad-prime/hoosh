use anyhow::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::{backend::CrosstermBackend, Terminal, TerminalOptions, Viewport};
use std::io;

pub type Tui = Terminal<CrosstermBackend<io::Stdout>>;

pub fn init_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let stdout = io::stdout();

    // Use inline viewport for input area only (like Ink's Static pattern)
    // Messages will be inserted above using terminal.insert_before()
    // This allows native terminal scrolling and text selection
    let viewport_height = 6; // Space for status + input + borders

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
