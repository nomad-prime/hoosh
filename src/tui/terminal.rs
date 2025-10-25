use crate::tui::app::AppState;
use crate::tui::minimal_terminal::Terminal;
use crate::tui::ui::{calculate_desired_height, render_ui};
use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::Backend;
use ratatui::backend::CrosstermBackend;
use ratatui::{TerminalOptions, Viewport};
use std::io;

pub type Tui = Terminal<CrosstermBackend<io::Stdout>>;

/// Initializes the terminal using MinimalTerminal.
/// It starts with a 1-line inline viewport which will be resized on the first draw.
pub fn init_terminal() -> Result<Tui> {
    // Tui now refers to MinimalTerminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnableBracketedPaste)?;

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::with_options(
        // Use MinimalTerminal here
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(1), // Start minimal
        },
    )?;
    Ok(terminal)
}

/// Restores the terminal to its normal state.
pub fn restore_terminal(mut terminal: Tui) -> Result<()> {
    // Tui is MinimalTerminal
    let mut stdout = io::stdout();
    stdout.execute(DisableBracketedPaste)?;
    disable_raw_mode()?;
    // Use MinimalTerminal's show_cursor
    terminal.show_cursor()?;
    Ok(())
}

/// Dynamically resizes the inline viewport based on UI needs and draws the frame.
/// This function contains the core logic adapted from codex-rs.
pub fn draw_dynamic_ui(terminal: &mut Tui, app: &mut AppState) -> Result<()> {
    // Tui is MinimalTerminal
    // 1. Calculate the required height for the current UI state.
    let height = calculate_desired_height(app);

    // 2. Get the current size of the physical terminal window.
    let backend_size = terminal.backend().size()?; // Use backend().size() for physical size

    // 3. Get the current viewport area from our MinimalTerminal.
    // Use the public viewport_area field from MinimalTerminal
    let current_viewport = terminal.get_viewport_area();

    // 4. Determine the target viewport area based on desired height.
    let mut target_viewport = current_viewport;
    target_viewport.height = height.min(backend_size.height); // Cap height by screen size
    target_viewport.width = backend_size.width; // Use full width

    // 5. Check if the target viewport would overflow the bottom of the screen.
    if target_viewport.bottom() > backend_size.height {
        let overflow = target_viewport.bottom() - backend_size.height;

        // 6. If it overflows, scroll the region *above* the current viewport up.
        // This makes space at the bottom by pushing history/previous lines up.
        // Requires the "scrolling-regions" feature flag in ratatui.
        terminal
            .backend_mut()
            .scroll_region_up(0..current_viewport.top(), overflow)?; // Scroll above current top

        // 7. Anchor the target viewport to the bottom of the screen.
        target_viewport.y = backend_size.height - target_viewport.height;
    }

    // 8. If the viewport needs to change (size or position)...
    if target_viewport != current_viewport {
        // ...clear the *current* area and then set the *new* target area.
        // Use the public clear() and set_viewport_area() from MinimalTerminal
        terminal.clear()?;
        terminal.set_viewport_area(target_viewport);
    }

    // 9. Draw the UI content into the (now correctly sized and positioned) viewport.
    // Use MinimalTerminal's draw method
    terminal.draw(|frame| {
        render_ui(frame, app);
    })?;

    Ok(())
}
