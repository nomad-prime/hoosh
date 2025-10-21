use anyhow::Result;
use crossterm::cursor::{Hide, MoveToColumn, MoveUp};
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType};
use crossterm::ExecutableCommand;
use ratatui::{backend::CrosstermBackend, Terminal, TerminalOptions, Viewport};
use std::io::{self, Write};

pub type Tui = Terminal<CrosstermBackend<io::Stdout>>;

pub fn init_terminal(viewport_height: u16) -> Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnableBracketedPaste)?;

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(viewport_height),
        },
    )?;
    Ok(terminal)
}

pub fn resize_terminal(terminal: Tui, new_viewport_height: u16) -> Result<Tui> {
    let old_height = terminal.size()?.height;

    // Hide cursor during transition
    let mut stdout = io::stdout();
    stdout.execute(Hide)?;

    // Clear the old viewport area
    // Move cursor up to the start of the viewport, then clear down
    if old_height > 0 {
        stdout.execute(MoveUp(old_height.saturating_sub(1)))?;
        stdout.execute(MoveToColumn(0))?;

        // Clear each line of the old viewport
        for _ in 0..old_height {
            stdout.execute(Clear(ClearType::CurrentLine))?;
            stdout.write_all(b"\n")?;
        }

        // Move back up to where we started
        stdout.execute(MoveUp(old_height))?;
        stdout.execute(MoveToColumn(0))?;
    }

    stdout.flush()?;

    // Drop the old terminal to release the backend
    drop(terminal);

    // Create a new terminal with the new viewport height
    let backend = CrosstermBackend::new(io::stdout());
    let mut new_terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(new_viewport_height),
        },
    )?;

    // Show cursor again
    new_terminal.show_cursor()?;

    Ok(new_terminal)
}

pub fn restore_terminal(mut terminal: Tui) -> Result<()> {
    let mut stdout = io::stdout();
    stdout.execute(DisableBracketedPaste)?;
    disable_raw_mode()?;
    terminal.show_cursor()?;
    Ok(())
}
