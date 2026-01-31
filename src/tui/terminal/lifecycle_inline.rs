use crate::tui::terminal::Terminal;
use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::Backend;
use ratatui::backend::CrosstermBackend;
use ratatui::{TerminalOptions, Viewport};
use std::io;

pub type HooshTerminal = Terminal<CrosstermBackend<io::Stdout>>;

pub fn init_terminal_inline() -> Result<HooshTerminal> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnableBracketedPaste)?;

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(1),
        },
    )?;
    Ok(terminal)
}

pub fn restore_terminal_inline(mut terminal: HooshTerminal) -> Result<()> {
    let mut stdout = io::stdout();
    stdout.execute(DisableBracketedPaste)?;
    disable_raw_mode()?;
    terminal.show_cursor()?;
    Ok(())
}

pub fn resize_terminal_inline(terminal: &mut HooshTerminal, height: u16) -> Result<()> {
    let backend_size = terminal.backend().size()?;
    let current_viewport = terminal.get_viewport_area();

    let mut target_viewport = current_viewport;
    target_viewport.height = height.min(backend_size.height);
    target_viewport.width = backend_size.width;

    if target_viewport.bottom() > backend_size.height {
        let overflow = target_viewport.bottom() - backend_size.height;

        terminal
            .backend_mut()
            .scroll_region_up(0..current_viewport.top(), overflow)?;

        target_viewport.y = backend_size.height - target_viewport.height;
    }

    if target_viewport != current_viewport {
        terminal.clear()?;
        terminal.set_viewport_area(target_viewport);
    }

    Ok(())
}
