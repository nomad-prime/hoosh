1. **History ("Tape"):** They use `terminal.insert_before()` to print static history, which gives them native
   scrollback. (This is what your `MessageRenderer` does).
2. **UI ("Canvas"):** They use `terminal.draw()`, but it's not a *fixed* viewport.
3. **The "Magic" (`tui.rs:532`):** Before *every draw*, they calculate the `desired_height` of their UI. If that height
   would overflow the bottom of the screen, they manually call `backend_mut().scroll_region_up()` to "push" the entire
   shell history up by *exactly* enough lines to make their UI fit at the bottom.

This gives them the best of both worlds: native scrollback *and* a dynamic, app-like UI that "grows" and "shrinks."

Here is how to implement this *exact* logic in your own project.

-----

### Step 1: Modify `terminal.rs`

First, you need to change `init_terminal` to start with a minimal viewport. Then, add a new `draw_ui` function that
contains the "magic" resizing logic you found.

```rust
// In terminal.rs

use crate::tui::app::AppState;
use crate::tui::ui::{calculate_desired_height, render_ui}; // We will create these in Step 2
use anyhow::Result;
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::ExecutableCommand;
use ratatui::{
    backend::CrosstermBackend,
    terminal::{Viewport, Terminal},
};
use std::io;

pub type Tui = Terminal<CrosstermBackend<io::Stdout>>;

// We need a small, constant height to start with.
const INITIAL_VIEWPORT_HEIGHT: u16 = 1;

pub fn init_terminal() -> Result<Tui> { // Removed viewport_height parameter
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnableBracketedPaste)?;

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            // Start with a minimal 1-line viewport. It will be resized on the first draw.
            viewport: Viewport::Inline(INITIAL_VIEWPORT_HEIGHT),
        },
    )?;
    Ok(terminal)
}

pub fn restore_terminal(mut terminal: Tui) -> Result<()> {
    // ... (your existing restore_terminal function is fine)
    let mut stdout = io.stdout();
    stdout.execute(DisableBracketedPaste)?;
    disable_raw_mode()?;
    terminal.show_cursor()?;
    Ok(())
}

/// This is our new draw function, containing the 'codex-rs' logic.
pub fn draw_ui(terminal: &mut Tui, app: &mut AppState) -> Result<()> {
    // 1. Calculate the total height our UI needs *right now*.
    let height = calculate_desired_height(app);

    // 2. This is the magic snippet from tui.rs:532
    let size = terminal.size()?;
    let mut area = terminal.viewport_area();

    // Clamp height to terminal size
    area.height = height.min(size.height);
    area.width = size.width;

    // If the new, resized area would overflow the bottom...
    if area.bottom() > size.height {
        // ...scroll the *entire terminal* up by the difference.
        terminal
            .backend_mut()
            .scroll_region_up(0..area.top(), area.bottom() - size.height)?;
        // Anchor the area to the bottom
        area.y = size.height - area.height;
    }

    // If the viewport area has changed, clear it and set the new one.
    if area != terminal.viewport_area() {
        terminal.clear()?;
        terminal.set_viewport_area(area);
    }

    // 3. Now, render the UI inside the perfectly-sized and positioned viewport.
    terminal.draw(|frame| {
        render_ui(frame, app);
    })?;

    Ok(())
}

```

-----

### Step 2: Modify `ui.rs`

Next, you need to split your `render` function into two parts:

1. `calculate_desired_height`: This *only* calculates the total lines needed.
2. `render_ui`: This *only* does the rendering inside the given frame.

<!-- end list -->

```rust
// In ui.rs

use ratatui::{
    layout::{Constraint, Layout},
    Frame,
};

use super::app::AppState;
use super::components::{
    approval_dialog::ApprovalDialogWidget, completion_popup::CompletionPopupWidget,
    input::InputWidget, mode_indicator::ModeIndicatorWidget,
    permission_dialog::PermissionDialogWidget, status::StatusWidget,
};

/// NEW FUNCTION
/// Calculates the total vertical height needed for the entire UI.
pub fn calculate_desired_height(app: &AppState) -> u16 {
    // This is your exact dialog height logic, extracted from your original render fn
    let dialog_height = if app.is_showing_permission_dialog() {
        15 // You can refine this by asking the widget itself
    } else if app.is_showing_approval_dialog() {
        10 // You can refine this
    } else if app.is_completing() {
        12 // You can refine this
    } else {
        0
    };

    let status_height = 1;
    let input_height = 3;
    let mode_height = 1; // This will be 0 if a dialog is showing

    let base_ui_height = status_height + input_height +
        if dialog_height > 0 { 0 } else { mode_height };

    // Total height is the base UI + the dynamic dialog height
    base_ui_height + dialog_height
}

/// RENAMED from 'render' to 'render_ui'
/// This function is now only responsible for drawing, not for calculating height.
pub fn render_ui(frame: &mut Frame, app: &mut AppState) {
    let viewport_area = frame.area();

    // This logic is now simpler. We just use the height calculated before.
    let dialog_height = if app.is_showing_permission_dialog() {
        15
    } else if app.is_showing_approval_dialog() {
        10
    } else if app.is_completing() {
        12
    } else {
        0
    };

    let vertical = Layout::vertical([
        Constraint::Min(0), // This will be the status area
        Constraint::Length(3), // input_area
        Constraint::Length(1), // mode_area
        Constraint::Length(dialog_height), // dialog_area
    ]);

    // We adjust the layout constraints to match the calculated height.
    // The spacer is no longer needed, as the frame is already the perfect size.
    let [status_area, input_area, mode_area_or_dialog, dialog_area] = if dialog_height > 0 {
        let vertical = Layout::vertical([
            Constraint::Length(1), // status_area
            Constraint::Length(3), // input_area
            Constraint::Length(dialog_height), // dialog_area
        ]);
        let [status, input, dialog] = vertical.areas(viewport_area);
        [status, input, dialog, dialog] // mode_area is unused, dialog is last
    } else {
        let vertical = Layout::vertical([
            Constraint::Length(1), // status_area
            Constraint::Length(3), // input_area
            Constraint::Length(1), // mode_area
        ]);
        let [status, input, mode] = vertical.areas(viewport_area);
        [status, input, mode, mode] // dialog_area is unused, mode is last
    };


    // --- This is your existing render logic, slightly adapted ---

    frame.render_widget(StatusWidget::new(app), status_area);
    frame.render_widget(InputWidget::new(&app.input), input_area);

    if !app.is_completing()
        && !app.is_showing_permission_dialog()
        && !app.is_showing_approval_dialog()
    {
        frame.render_widget(ModeIndicatorWidget::new(app), mode_area_or_dialog);
    }

    if app.is_completing() {
        frame.render_widget(CompletionPopupWidget::new(app, input_area), dialog_area);
    }

    if app.is_showing_permission_dialog() {
        frame.render_widget(PermissionDialogWidget::new(app, mode_area_or_dialog), dialog_area);
    }

    if app.is_showing_approval_dialog() {
        frame.render_widget(ApprovalDialogWidget::new(app, mode_area_or_dialog), dialog_area);
    }
}
```

*Note:* I had to refactor your `ui.rs` layout logic slightly to remove the `Constraint::Min(0)` spacer, as the `frame`
itself is now perfectly sized by our `draw_ui` function.

-----

### Step 3: Modify Your Main Loop (e.g., in `main.rs`)

Finally, in your main loop, you just need to change one line.

```rust
// In your main.rs (or wherever your main loop is)

// ... setup code ...
// let mut terminal = init_terminal()?;
// let mut app = AppState::new();

loop {
// ... your event handling logic (key presses, etc.) ...

// --- THIS IS THE CHANGE ---

// BEFORE:
// terminal.draw(|frame| crate::tui::ui::render(frame, &mut app))?;

// AFTER:
crate::tui::terminal::draw_ui( & mut terminal, & mut app) ?;

// ... check for app.should_quit ...
}

// ... restore_terminal(terminal) ...
```

And that's it. You have now implemented the `codex-rs` dynamic inline rendering model. Your UI will now "grow" and "
shrink" as needed, pushing the shell history up, which is exactly the behavior you wanted.
