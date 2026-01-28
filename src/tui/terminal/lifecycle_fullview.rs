use crate::tui::terminal::Terminal;
use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::{TerminalOptions, Viewport};
use std::io;

pub type HooshTerminal = Terminal<CrosstermBackend<io::Stdout>>;

pub fn init_terminal_fullview() -> Result<HooshTerminal> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableBracketedPaste)?;
    stdout.execute(EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fullscreen,
        },
    )?;
    Ok(terminal)
}

pub fn restore_terminal_fullview(mut terminal: HooshTerminal) -> Result<()> {
    let mut stdout = io::stdout();
    stdout.execute(DisableMouseCapture)?;
    stdout.execute(DisableBracketedPaste)?;
    stdout.execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    terminal.show_cursor()?;
    Ok(())
}
