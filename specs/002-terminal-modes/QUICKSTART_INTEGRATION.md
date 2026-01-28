# Quick Start: Completing Terminal Display Modes

**Goal**: Get from current state (42% complete) to fully working feature

**Time Estimate**: 15-21 hours

**Prerequisites**: Rust 2024, ratatui 0.29, basic TUI knowledge

---

## Step 1: Setup (5 minutes)

```bash
# Clone and switch to feature branch
cd ~/Projects/hoosh
git checkout 002-terminal-modes

# Verify everything compiles
cargo check
# Should output: Finished `dev` profile [unoptimized + debuginfo]

# Run existing tests
cargo test
# Should show: 22 tests passing

# Review current state
cat specs/002-terminal-modes/IMPLEMENTATION_STATUS.md
cat specs/002-terminal-modes/REVIEW_SUMMARY.md
```

---

## Step 2: Fullview Mode (MVP) - 4-6 hours

### Task 1: Create Scroll Input Handler (2 hours)

**File**: `src/tui/handlers/scroll_handler.rs`

```rust
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use crate::tui::app_state::AppState;

pub fn handle_scroll_event(app_state: &mut AppState, event: &Event) -> Result<bool> {
    let scroll_state = match &mut app_state.scroll_state {
        Some(state) => state,
        None => return Ok(false), // Not in fullview mode
    };

    match event {
        Event::Key(KeyEvent { code, modifiers, .. }) => {
            match code {
                KeyCode::Down => scroll_state.scroll_down(1),
                KeyCode::Up => scroll_state.scroll_up(1),
                KeyCode::Char('j') if !modifiers.contains(KeyModifiers::CONTROL) => {
                    scroll_state.scroll_down(1);
                }
                KeyCode::Char('k') if !modifiers.contains(KeyModifiers::CONTROL) => {
                    scroll_state.scroll_up(1);
                }
                KeyCode::PageDown => scroll_state.page_down(),
                KeyCode::PageUp => scroll_state.page_up(),
                KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                    scroll_state.scroll_down(scroll_state.viewport_height / 2);
                }
                KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                    scroll_state.scroll_up(scroll_state.viewport_height / 2);
                }
                KeyCode::Home => scroll_state.offset = 0,
                KeyCode::End => scroll_state.scroll_to_bottom(),
                _ => return Ok(false),
            }
            Ok(true)
        }
        Event::Mouse(mouse_event) => {
            match mouse_event.kind {
                MouseEventKind::ScrollUp => scroll_state.scroll_up(3),
                MouseEventKind::ScrollDown => scroll_state.scroll_down(3),
                _ => return Ok(false),
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
```

**Add to** `src/tui/handlers/mod.rs`:
```rust
pub mod scroll_handler;
pub use scroll_handler::handle_scroll_event;
```

### Task 2: Integrate Mode Selection (1 hour)

**File**: `src/session.rs` (find session initialization)

```rust
use crate::terminal_mode::{TerminalMode, select_terminal_mode};
use crate::terminal_capabilities::TerminalCapabilities;

// In session initialization function:
let terminal_mode = select_terminal_mode(
    cli.mode.clone(),
    config.terminal_mode.clone(),
);

// Detect capabilities and warn if needed
let capabilities = TerminalCapabilities::detect()?;
capabilities.warn_if_vscode_with_inline(terminal_mode);

// Initialize scroll state for fullview mode
let scroll_state = if terminal_mode == TerminalMode::Fullview {
    let (_width, height) = crossterm::terminal::size()?;
    Some(ScrollState::new(height as usize))
} else {
    None
};

// Store in app_state
app_state.scroll_state = scroll_state;
```

### Task 3: Modify Terminal Lifecycle (1 hour)

**File**: `src/tui/terminal/lifecycle.rs`

Find terminal initialization and modify:

```rust
// Change from Viewport::Inline to mode-based
let viewport = match terminal_mode {
    TerminalMode::Inline => Viewport::Inline(1),
    TerminalMode::Fullview => Viewport::Fullscreen,
    TerminalMode::Tagged => return Err(anyhow!("Tagged mode doesn't use TUI")),
};
```

### Task 4: Update Render Loop (1-2 hours)

**File**: `src/tui/app_loop.rs` (find render_frame or similar)

```rust
// In render function:
if let Some(scroll_state) = &app_state.scroll_state {
    // Fullview mode: render only visible portion
    let start_line = scroll_state.offset;
    let end_line = (scroll_state.offset + scroll_state.viewport_height)
        .min(scroll_state.content_height);

    // Render messages in range [start_line, end_line]
    // Update content_height after rendering
} else {
    // Inline mode: render all messages as before
}
```

### Task 5: Add Scroll Handler to Event Loop (30 min)

**File**: `src/tui/app_loop.rs` or wherever input events are processed

```rust
use crate::tui::handlers::handle_scroll_event;

// In event handling loop, add before other handlers:
if handle_scroll_event(&mut app_state, &event)? {
    continue; // Event was handled, skip other handlers
}
```

### Task 6: Handle Terminal Resize (30 min)

**File**: Terminal lifecycle or event handler

```rust
use crossterm::event::Event;

match event {
    Event::Resize(width, height) => {
        if let Some(scroll_state) = &mut app_state.scroll_state {
            scroll_state.update_viewport_height(height as usize);
        }
    }
    _ => {}
}
```

**Test Fullview Mode:**
```bash
cargo run -- --mode fullview
# Try scrolling with arrow keys, j/k, mouse wheel
# Test in VSCode terminal
```

---

## Step 3: Tagged Mode - 6-8 hours

### Task 1: Add `hoosh setup` Command (1 hour)

**File**: `src/cli/setup.rs` (modify existing)

```rust
use crate::cli::shell_setup::{detect_shell, install_shell_alias};

pub async fn handle_setup() -> Result<()> {
    // Existing setup wizard code stays...

    // After wizard or as separate step:
    println!("\n=== Shell Integration ===");
    println!("Install @hoosh alias for tagged mode? (y/n)");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().eq_ignore_ascii_case("y") {
        match detect_shell() {
            Ok(shell_type) => {
                install_shell_alias(shell_type)?;
            }
            Err(e) => {
                eprintln!("Could not detect shell: {}", e);
                eprintln!("Please manually add @hoosh alias to your shell config");
            }
        }
    }

    Ok(())
}
```

### Task 2: Create Terminal Spinner (2 hours)

**File**: `src/terminal_spinner.rs`

```rust
use std::io::{self, Write};

pub struct TerminalSpinner {
    frames: &'static [&'static str],
    current_frame: usize,
    message: String,
}

impl TerminalSpinner {
    pub fn new(message: String) -> Self {
        Self {
            frames: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"],
            current_frame: 0,
            message,
        }
    }

    pub fn tick(&mut self) {
        self.current_frame = (self.current_frame + 1) % self.frames.len();
        let frame = self.frames[self.current_frame];
        eprint!("\r\x1b[K{} {}", frame, self.message);
        io::stderr().flush().unwrap();
    }

    pub fn finish(&self, final_message: &str) {
        eprintln!("\r\x1b[K{}", final_message);
    }
}
```

### Task 3: Create Text Prompts (1 hour)

**File**: `src/text_prompts.rs`

```rust
use std::io::{self, BufRead, Write};

pub fn prompt_yes_no(question: &str) -> bool {
    eprintln!("\n┌─────────────────────────────────");
    eprintln!("│ {}", question);
    eprintln!("└─────────────────────────────────");
    eprint!("[y/N]: ");
    io::stderr().flush().unwrap();

    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).unwrap();

    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}
```

### Task 4: Integrate Session Files (2 hours)

**File**: `src/session.rs`

```rust
use crate::session_files::{SessionFile, get_terminal_pid, cleanup_stale_sessions};

// At start of tagged mode invocation:
cleanup_stale_sessions()?;

let pid = get_terminal_pid()?;
let mut session = SessionFile::load(pid)?
    .unwrap_or_else(|| SessionFile::new(pid));

// Use session.messages for conversation history

// After response:
session.messages.push(/* new messages */);
session.save()?;
```

### Task 5: SIGINT Handler (1-2 hours)

```rust
use tokio::signal;

// In main or session initialization:
tokio::select! {
    result = run_agent() => result?,
    _ = signal::ctrl_c() => {
        if mode == TerminalMode::Tagged {
            // Save partial session
            session.save()?;
        }
        Ok(())
    }
}
```

**Test Tagged Mode:**
```bash
cargo run -- setup
source ~/.bashrc
@hoosh "hello"
@hoosh "what did I say?"  # Should remember context
```

---

## Step 4: Inline Mode - 2-3 hours

### Task 1: Ensure Default Mode (30 min)

**File**: `src/session.rs`

```rust
// Make sure inline is default when no mode specified
let mode = select_terminal_mode(cli.mode.clone(), config.terminal_mode.clone());
// Should return TerminalMode::Inline by default
```

### Task 2: Verify Existing Rendering (1 hour)

**File**: Existing render code

```rust
// In render loop:
match app_state.scroll_state {
    Some(_) => { /* fullview rendering */ }
    None => {
        // Existing inline rendering - VERIFY UNCHANGED
        // Run through existing message rendering path
    }
}
```

### Task 3: Integration Testing (1 hour)

```bash
# Test inline mode (default)
cargo run
# Verify no regressions

# Test in different terminals
# iTerm2, Alacritty, Terminal.app
```

---

## Step 5: Polish - 3-4 hours

### Task 1: Update README.md (1 hour)

Add section about terminal modes:
```markdown
## Terminal Display Modes

Hoosh supports three terminal display modes...
[Copy from examples/usage-examples.md]
```

### Task 2: Run Clippy and Fix (1 hour)

```bash
cargo clippy --fix
cargo fmt
```

### Task 3: Manual Validation (1-2 hours)

Run through all scenarios in `quickstart.md`:
- [ ] Fullview in VSCode
- [ ] Tagged mode workflow
- [ ] Inline mode compatibility

---

## Testing Checklist

```bash
# Unit tests
cargo test

# Build release
cargo build --release

# Manual tests
./target/release/hoosh --mode inline
./target/release/hoosh --mode fullview
./target/release/hoosh setup
@hoosh "test"

# Verify in VSCode
code .
# Open terminal in VSCode
./target/release/hoosh --mode fullview
```

---

## Common Issues & Solutions

### Issue: Scroll not working

**Check**: Is scroll_state Some(_)?
**Fix**: Ensure mode == Fullview and scroll_state initialized

### Issue: @hoosh not found

**Check**: Did shell config get updated?
**Fix**: Run `source ~/.bashrc` or restart terminal

### Issue: Session not persisting

**Check**: Is session_context_enabled true?
**Fix**: Set in config.toml

---

## Success Criteria

- [ ] All three modes work independently
- [ ] Tests passing (aim for >30 total)
- [ ] No regressions in existing functionality
- [ ] Documentation complete
- [ ] Code quality: cargo clippy clean

---

## Resources

- Implementation Status: `specs/002-terminal-modes/IMPLEMENTATION_STATUS.md`
- Review Summary: `specs/002-terminal-modes/REVIEW_SUMMARY.md`
- Original Plan: `specs/002-terminal-modes/plan.md`
- Data Model: `specs/002-terminal-modes/data-model.md`
- Research: `specs/002-terminal-modes/research.md`

---

## Need Help?

1. Review IMPLEMENTATION_STATUS.md for detailed context
2. Check research.md for technical decisions
3. Look at existing tests for patterns
4. Reference examples/ directory for configuration

**Good luck! The hard part (architecture) is done. Integration is straightforward! 🚀**
