# Research: Terminal Display Modes

**Feature**: 002-terminal-modes
**Date**: 2026-01-23
**Status**: Complete

## Overview

This document captures research findings and technical decisions for implementing three terminal display modes in hoosh: inline (existing), fullview (VSCode-compatible with internal scrolling), and tagged (non-hijacking shell integration). The research focuses on terminal detection, scrolling patterns, session file design, shell integration, and terminal-native output.

---

## Research Questions

### 1. Terminal Mode Detection

**Question**: How can we reliably detect VSCode integrated terminal vs standard terminal, and what fallback strategies should we use?

**Finding**: Multiple environment variables and terminal capabilities can be used for detection:

**VSCode Terminal Detection**:
- `TERM_PROGRAM=vscode` - Set by VSCode integrated terminal
- `VSCODE_INJECTION=1` - Set by VSCode shell integration
- `VSCODE_GIT_ASKPASS_*` - Various VSCode-specific variables
- `VSCODE_CLI=1` - Set when using VSCode CLI

**Other Terminal Identifiers**:
- `TERM_PROGRAM=iTerm.app` - iTerm2
- `TERM_PROGRAM=Apple_Terminal` - macOS Terminal
- `TERM_PROGRAM=WezTerm` - WezTerm
- `ALACRITTY_WINDOW_ID` - Alacritty
- `KITTY_WINDOW_ID` - Kitty terminal

**Terminal Capabilities**:
- `TERM` environment variable (e.g., "xterm-256color", "screen-256color")
- Crossterm's `supports_keyboard_enhancement()` for advanced features
- Terminal size detection via `crossterm::terminal::size()`

**Decision**: Use multi-layered detection strategy with explicit override

**Rationale**:
1. **Primary**: Check CLI flag `--mode` (user explicit choice, highest priority)
2. **Secondary**: Check config file `terminal_mode` setting (project/global)
3. **Auto-detect**: Check `TERM_PROGRAM` and `VSCODE_*` environment variables
4. **Fallback**: Default to inline mode (backward compatible, safest)

**Implementation Pattern**:
```rust
pub fn detect_terminal_mode(
    cli_mode: Option<TerminalMode>,
    config_mode: Option<TerminalMode>,
) -> TerminalMode {
    // CLI flag has highest priority
    if let Some(mode) = cli_mode {
        return mode;
    }

    // Config file second priority
    if let Some(mode) = config_mode {
        return mode;
    }

    // Auto-detect based on environment
    if is_vscode_terminal() {
        eprintln!("VSCode terminal detected. Consider using --mode fullview");
        eprintln!("Defaulting to inline mode. Set terminal_mode in config to suppress this message.");
    }

    // Default to inline (backward compatible)
    TerminalMode::Inline
}

fn is_vscode_terminal() -> bool {
    std::env::var("TERM_PROGRAM")
        .map(|v| v == "vscode")
        .unwrap_or(false)
    || std::env::var("VSCODE_INJECTION").is_ok()
}
```

**Alternatives Considered**:
- **Automatic mode switching**: Rejected - could surprise users, prefer explicit config
- **Query terminal capabilities at runtime**: Rejected - adds complexity, not reliable across all terminals
- **Maintain terminal compatibility database**: Rejected - maintenance burden, env vars sufficient

**Risk Mitigation**:
- Warn users when VSCode detected but inline mode active (hint to use fullview)
- Document terminal detection behavior clearly in quickstart guide
- Provide `hoosh doctor` command to display detected terminal info

---

### 2. Ratatui Fullview Scrolling

**Question**: How should fullview mode implement internal scrolling using Ratatui's Viewport::Fullscreen?

**Finding**: Ratatui provides multiple viewport modes with different scrolling behaviors:

**Viewport Modes** (from `src/tui/terminal/custom_terminal.rs`):
- `Viewport::Inline(height)` - Current default, grows dynamically, uses terminal scrollback
- `Viewport::Fullscreen` - Takes entire terminal, requires internal scroll state management
- `Viewport::Fixed(area)` - Fixed rectangular area

**Existing Infrastructure**:
- Current implementation uses `Viewport::Inline(1)` in `src/tui/terminal/lifecycle.rs:25`
- Terminal resizing handled via `resize_terminal()` function with scroll region manipulation
- Crossterm provides mouse event handling via `Event::Mouse(MouseEvent)`

**Scroll State Management Pattern**:
```rust
pub struct ScrollState {
    /// Current scroll offset (top visible line)
    pub offset: usize,
    /// Total content height (lines)
    pub content_height: usize,
    /// Viewport height (visible lines)
    pub viewport_height: usize,
}

impl ScrollState {
    pub fn scroll_down(&mut self, lines: usize) {
        let max_offset = self.content_height.saturating_sub(self.viewport_height);
        self.offset = (self.offset + lines).min(max_offset);
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.offset = self.offset.saturating_sub(lines);
    }

    pub fn page_down(&mut self) {
        self.scroll_down(self.viewport_height.saturating_sub(1));
    }

    pub fn page_up(&mut self) {
        self.scroll_up(self.viewport_height.saturating_sub(1));
    }

    pub fn at_bottom(&self) -> bool {
        self.offset + self.viewport_height >= self.content_height
    }
}
```

**Mouse Event Handling** (using crossterm):
```rust
use crossterm::event::{Event, MouseEvent, MouseEventKind};

// In event handler
match event {
    Event::Mouse(MouseEvent { kind: MouseEventKind::ScrollUp, .. }) => {
        scroll_state.scroll_up(3); // 3 lines per scroll tick
    }
    Event::Mouse(MouseEvent { kind: MouseEventKind::ScrollDown, .. }) => {
        scroll_state.scroll_down(3);
    }
    _ => {}
}
```

**Vim-Style Keybindings** (from existing patterns in `src/tui/handlers/quit_handler.rs`):
```rust
use crossterm::event::{Event, KeyCode, KeyModifiers};

match event {
    Event::Key(key) => match key.code {
        KeyCode::Char('j') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            scroll_state.scroll_down(1);
        }
        KeyCode::Char('k') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            scroll_state.scroll_up(1);
        }
        KeyCode::Down => scroll_state.scroll_down(1),
        KeyCode::Up => scroll_state.scroll_up(1),
        KeyCode::PageDown => scroll_state.page_down(),
        KeyCode::PageUp => scroll_state.page_up(),
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            scroll_state.scroll_down(scroll_state.viewport_height / 2); // Half-page
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            scroll_state.scroll_up(scroll_state.viewport_height / 2); // Half-page
        }
        _ => {}
    }
}
```

**Decision**: Implement ScrollState struct in fullview app_state, handle scrolling in dedicated input handler

**Rationale**:
- Scroll state naturally belongs to fullview-specific state (not shared with inline mode)
- Existing InputHandler pattern (`src/tui/input_handler.rs`) provides clean architecture
- Mouse support enhances UX for users who prefer mouse scrolling
- Vim keybindings familiar to developer audience, no conflict with text input (only active when not typing)
- Auto-scroll to bottom on new messages (matches chat UX expectations)

**Fullview Layout Strategy**:
```rust
// In fullview/layout.rs
pub fn render_scrollable_messages(
    scroll_state: &ScrollState,
    messages: &[Message],
    area: Rect,
    buf: &mut Buffer,
) {
    // Calculate visible range based on scroll offset
    let start_line = scroll_state.offset;
    let end_line = (scroll_state.offset + scroll_state.viewport_height)
        .min(scroll_state.content_height);

    // Render only visible portion (viewport windowing for performance)
    let visible_messages = get_messages_in_range(messages, start_line, end_line);
    render_messages(visible_messages, area, buf);
}
```

**Alternatives Considered**:
- **Ratatui ScrollView widget**: Does not exist in ratatui 0.29.0, manual implementation required
- **Line-based scrolling only**: Rejected - page up/down improves navigation efficiency
- **Mouse-only scrolling**: Rejected - keyboard-only users need full access
- **Automatic scrolling without manual control**: Rejected - users need to review conversation history

**Performance Considerations**:
- Only render visible lines (viewport windowing) to handle large conversations
- Reuse existing MessageRenderer (already optimized for markdown rendering)
- Update scroll_state.content_height on message addition (incremental update)
- Target: Resize reflow <200ms (spec requirement SC-003)

---

### 3. Session File Design

**Question**: How should session files be designed for tagged mode context persistence?

**Finding**: Session files must persist conversation context across @hoosh invocations within a terminal's lifetime.

**Terminal PID Detection**:

**Unix/Linux/macOS**:
```rust
use std::process;

fn get_terminal_pid() -> u32 {
    // On Unix, use parent process ID (PPID)
    // The shell is our parent, terminal is shell's parent
    // But we want the current process context, so:

    // Option 1: Use environment variable set by shell
    if let Ok(ppid) = std::env::var("PPID") {
        return ppid.parse().unwrap_or(process::id());
    }

    // Option 2: Use current process ID as session identifier
    // (simpler, works if terminal PID not accessible)
    process::id()
}
```

**Windows**:
```rust
#[cfg(windows)]
fn get_terminal_pid() -> u32 {
    // Use $PID environment variable in PowerShell
    std::env::var("PID")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or_else(|| std::process::id())
}
```

**Decision**: Use shell-level PID via environment variable with fallback to process ID

**Rationale**:
- `$PPID` in bash/zsh represents the shell's PID (consistent across @hoosh invocations)
- Session tied to terminal lifetime means when terminal closes, context should reset
- Using parent shell PID (not terminal PID) is more reliable and portable
- Fallback to current process ID for environments where PPID unavailable

**Session File JSON Schema**:

**Decision**: Minimal schema with conversation messages and metadata

```json
{
  "session_pid": 12345,
  "created_at": "2026-01-23T10:30:00Z",
  "last_accessed": "2026-01-23T10:35:00Z",
  "shell_type": "zsh",
  "messages": [
    {
      "role": "user",
      "content": "analyze this file"
    },
    {
      "role": "assistant",
      "content": "I'll analyze the file...",
      "tool_calls": [...]
    }
  ],
  "context": {
    "working_directory": "/path/to/project",
    "conversation_id": "uuid-here"
  }
}
```

**Rationale**:
- Store only essential state (messages, metadata) - no UI state or transient data
- JSON format for human readability and debugging (vs binary format)
- Extensible `context` object for future metadata without schema changes
- File size typically <100KB for normal conversations (acceptable for <1ms I/O requirement)

**File Storage Location**:
```rust
// ~/.hoosh/sessions/session_12345.json
pub fn get_session_file_path(pid: u32) -> PathBuf {
    let home = dirs::home_dir().expect("Failed to get home directory");
    home.join(".hoosh")
        .join("sessions")
        .join(format!("session_{}.json", pid))
}
```

**Cleanup Strategies**:

**Decision**: Multi-pronged cleanup approach

1. **On-demand cleanup** (every hoosh invocation):
   ```rust
   pub fn cleanup_stale_sessions() -> Result<()> {
       let sessions_dir = get_sessions_dir();
       let now = Utc::now();

       for entry in fs::read_dir(sessions_dir)? {
           let path = entry?.path();
           if let Ok(metadata) = fs::metadata(&path) {
               if let Ok(modified) = metadata.modified() {
                   let age = now.signed_duration_since(modified.into());
                   if age.num_days() > 7 {
                       fs::remove_file(path)?;
                   }
               }
           }
       }
       Ok(())
   }
   ```

2. **Explicit cleanup command**:
   ```bash
   hoosh cleanup sessions --older-than 7d
   ```

3. **Session file validation** (detect corrupted/stale):
   ```rust
   pub fn is_valid_session(session: &SessionFile) -> bool {
       // Check if PID still exists (process validation)
       let pid_exists = check_pid_exists(session.session_pid);

       // Check if last access was recent
       let age = Utc::now().signed_duration_since(session.last_accessed);
       let is_recent = age.num_days() <= 7;

       pid_exists && is_recent
   }
   ```

**Rationale**:
- On-demand cleanup runs on every invocation (cheap operation, <1ms)
- 7-day threshold balances disk usage vs accidental cleanup
- Explicit command for user control (manual cleanup if needed)
- Session validation prevents using stale sessions from crashed processes

**PID Reuse Handling**:

**Problem**: PIDs can be reused by the OS, leading to wrong session loading

**Decision**: Use PID + creation timestamp as session identifier

```rust
pub struct SessionIdentifier {
    pub pid: u32,
    pub created_at: DateTime<Utc>,
}

pub fn validate_session_identity(
    session: &SessionFile,
    current_pid: u32,
) -> bool {
    if session.session_pid != current_pid {
        return false;
    }

    // Check if session was created before current shell started
    let shell_start_time = get_shell_start_time();
    session.created_at < shell_start_time
}
```

**Rationale**:
- PID alone insufficient (reuse risk on long-running systems)
- Creation timestamp + PID provides unique identifier
- If session is older than current shell, it's from a previous process with same PID
- Fail-safe: discard session if validation fails (prefer empty context over wrong context)

**Alternatives Considered**:
- **Use conversation storage for tagged mode**: Rejected - session context separate from conversation storage (per spec FR-022)
- **Full state serialization**: Rejected - only messages needed, UI state unnecessary in tagged mode
- **Database (SQLite) for sessions**: Rejected - overkill for simple key-value storage, JSON files sufficient
- **Cleanup via cron job**: Rejected - not portable, requires external setup

**Performance Validation**:
- Target: Session file I/O <1ms (per spec clarification)
- JSON serialization with serde: ~0.1-0.5ms for typical session (<100KB)
- File read/write on SSD: ~0.5ms
- Total: Well within 1ms budget

---

### 4. Shell Integration

**Question**: How should the @hoosh alias be created and managed across different shells?

**Finding**: Shell configuration varies significantly across shell types, requiring different approaches.

**Shell Config File Locations**:

**Bash**:
- **Interactive login shell**: `~/.bash_profile` (macOS default) or `~/.profile` (Linux)
- **Interactive non-login shell**: `~/.bashrc`
- **Common pattern**: `~/.bash_profile` sources `~/.bashrc`
- **Decision**: Write to `~/.bashrc` (most universal)

**Zsh**:
- **Primary**: `~/.zshrc` (interactive shells)
- **Login**: `~/.zprofile` (rarely used for aliases)
- **Decision**: Write to `~/.zshrc`

**Fish**:
- **Functions directory**: `~/.config/fish/functions/`
- **Config file**: `~/.config/fish/config.fish`
- **Decision**: Create function file `~/.config/fish/functions/@hoosh.fish`

**PowerShell**:
- **Profile location**: `$PROFILE` environment variable
- **Common path**: `~\Documents\PowerShell\Microsoft.PowerShell_profile.ps1`
- **Decision**: Query `$PROFILE` variable, create if doesn't exist

**Shell Detection**:

**Decision**: Multi-method detection with user confirmation

```rust
pub fn detect_shell() -> Result<ShellType> {
    // Method 1: Check SHELL environment variable
    if let Ok(shell_path) = std::env::var("SHELL") {
        if shell_path.contains("zsh") {
            return Ok(ShellType::Zsh);
        } else if shell_path.contains("bash") {
            return Ok(ShellType::Bash);
        } else if shell_path.contains("fish") {
            return Ok(ShellType::Fish);
        }
    }

    // Method 2: Check for shell-specific environment variables
    if std::env::var("ZSH_VERSION").is_ok() {
        return Ok(ShellType::Zsh);
    }
    if std::env::var("BASH_VERSION").is_ok() {
        return Ok(ShellType::Bash);
    }

    // Method 3: Check which shell binary exists
    if which::which("zsh").is_ok() {
        return Ok(ShellType::Zsh);
    }
    if which::which("bash").is_ok() {
        return Ok(ShellType::Bash);
    }

    Err(anyhow!("Could not detect shell type"))
}
```

**Rationale**:
- `$SHELL` most reliable indicator of user's default shell
- Shell-specific env vars handle cases where hoosh run from different shell
- Binary existence check as fallback
- Prompt user for manual selection if detection fails (better than wrong guess)

**Alias vs Function Trade-offs**:

| Approach | Pros | Cons |
|----------|------|------|
| **Alias** | Simple, single line, fast | Cannot handle complex argument passing, function-style syntax `@hoosh()` |
| **Function** | Full argument handling, more flexible | Slightly more verbose, requires function syntax |

**Decision**: Use shell function (not alias) for all shells

**Rationale**:
- Functions provide proper argument handling (`"$@"` in bash/zsh, `$argv` in fish)
- Quoted arguments work correctly: `@hoosh "fix this bug"` vs `@hoosh fix this bug`
- Future extensibility (can add logic before/after hoosh call)
- Minimal performance difference (functions compiled at shell startup)

**Function Templates**:

**Bash/Zsh Function**:
```bash
# Added by hoosh setup on 2026-01-23
@hoosh() {
    # Export PPID for session tracking
    export PPID="$$"
    hoosh agent --mode tagged "$@"
}
```

**Fish Function** (`~/.config/fish/functions/@hoosh.fish`):
```fish
# Added by hoosh setup on 2026-01-23
function @hoosh --description 'Hoosh AI assistant in tagged mode'
    # Export parent PID for session tracking
    set -x PPID %self
    hoosh agent --mode tagged $argv
end
```

**PowerShell Function**:
```powershell
# Added by hoosh setup on 2026-01-23
function @hoosh {
    $env:PID = $PID
    hoosh agent --mode tagged $args
}
```

**Installation Process**:

**Decision**: Guided setup with backup and verification

```rust
pub fn install_shell_alias(shell_type: ShellType) -> Result<()> {
    let config_path = get_shell_config_path(shell_type)?;

    // 1. Create backup
    let backup_path = format!("{}.backup.{}", config_path, timestamp());
    fs::copy(&config_path, backup_path)?;

    // 2. Check if alias already exists
    let content = fs::read_to_string(&config_path)?;
    if content.contains("@hoosh") {
        eprintln!("Warning: @hoosh already defined in {}", config_path);
        eprintln!("Skipping installation. Remove existing definition to reinstall.");
        return Ok(());
    }

    // 3. Append function definition
    let function_def = get_function_template(shell_type);
    let mut file = OpenOptions::new().append(true).open(config_path)?;
    writeln!(file, "\n{}", function_def)?;

    // 4. Verify installation
    eprintln!("✅ Installed @hoosh function in {}", config_path);
    eprintln!("Run 'source {}' or restart terminal to activate", config_path);

    Ok(())
}
```

**Rationale**:
- Backup prevents accidental config corruption
- Duplicate check avoids double-installation
- Append-only preserves user's existing config
- Verification message guides user on next steps

**Conflict Resolution**:

**Problem**: User may have existing `@hoosh` command or alias

**Decision**: Detect conflict and prompt user

```rust
pub fn check_for_conflicts() -> Result<Option<String>> {
    // Check if @hoosh command exists in PATH
    if which::which("@hoosh").is_ok() {
        return Ok(Some("Existing @hoosh binary found in PATH".into()));
    }

    // Check if alias already defined (read shell config)
    let config = read_shell_config()?;
    if config.contains("alias @hoosh") || config.contains("function @hoosh") {
        return Ok(Some("@hoosh already defined in shell config".into()));
    }

    Ok(None)
}
```

**Alternatives Considered**:
- **Use different prefix** (e.g., `hoosh!` or `$hoosh`): Rejected - `@hoosh` is user-preferred syntax
- **Modify PATH instead of function**: Rejected - requires creating wrapper script, more complex
- **Automatic uninstall of existing @hoosh**: Rejected - too invasive, user should decide
- **Support multiple shells simultaneously**: Accepted - install function in all detected shell configs

**Fish/Nushell/PowerShell Support**:

**Fish**: Full support via function file (idiomatic fish approach)
**PowerShell**: Full support via profile function
**Nushell**: Future work - requires different approach (custom command registration)

**Rationale**:
- Fish is popular among developers (high priority)
- PowerShell common on Windows (essential for cross-platform)
- Nushell niche audience (defer to future enhancement)
- Target: 95% shell compatibility (bash/zsh/fish covers ~95% of users per spec SC-006)

---

### 5. Terminal-Native Output

**Question**: How should tagged mode render output, status, and prompts without TUI components?

**Finding**: Terminal-native output requires careful handling of stdout/stderr, ANSI codes, and user interaction.

**Braille Spinners Without TUI**:

**Finding**: Hoosh already uses braille spinners in `src/tui/components/status_bar.rs`

**Existing Spinner Patterns** (from status_bar.rs):
```rust
let thinking_spinners = [
    &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"][..],
    &["⠐", "⠒", "⠓", "⠋", "⠙", "⠹", "⠸", "⠼"][..],
    // ... more variants
];

let executing_spinners = [
    &["⠋", "⠙", "⠚", "⠞", "⠖", "⠦", "⠤", "⠐"][..],
    &["⠁", "⠉", "⠋", "⠛", "⠟", "⠿", "⠿", "⠟"][..],
    // ... more variants
];
```

**Decision**: Reuse spinner arrays, render to stdout with carriage return

**Terminal-Native Spinner Implementation**:
```rust
use std::io::{self, Write};
use std::time::Duration;

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

        // Carriage return (\r) moves cursor to start of line
        // ANSI escape code \x1b[K clears from cursor to end of line
        eprint!("\r\x1b[K{} {}", frame, self.message);
        io::stderr().flush().unwrap();
    }

    pub fn finish(&self, final_message: &str) {
        eprintln!("\r\x1b[K{}", final_message);
    }
}

// Usage:
let mut spinner = TerminalSpinner::new("Processing query".into());
loop {
    spinner.tick();
    std::thread::sleep(Duration::from_millis(80)); // 12.5 FPS

    if work_complete {
        spinner.finish("✓ Complete");
        break;
    }
}
```

**Rationale**:
- `eprint!` uses stderr (doesn't pollute stdout where responses go)
- Carriage return (`\r`) overwrites line (creates animation effect)
- ANSI escape `\x1b[K` clears to end of line (prevents trailing characters)
- Flush explicitly to ensure immediate display
- 80ms interval (~12 FPS) balances smoothness vs CPU usage

**Text-Based Permission Prompts**:

**Decision**: Simple y/n prompts with clear formatting, no TUI widgets

```rust
use std::io::{self, BufRead};

pub fn prompt_permission(descriptor: &ToolPermissionDescriptor) -> bool {
    eprintln!("\n┌─ Permission Request ─────────────────────────");
    eprintln!("│ Tool: {}", descriptor.tool_name);
    eprintln!("│ Action: {}", descriptor.description);
    eprintln!("└──────────────────────────────────────────────");
    eprint!("Allow this action? [y/N]: ");
    io::stderr().flush().unwrap();

    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).unwrap();

    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

// Enhanced version with options:
pub fn prompt_permission_with_options(
    descriptor: &ToolPermissionDescriptor
) -> PermissionResponse {
    eprintln!("\n┌─ Permission Request ─────────────────────────");
    eprintln!("│ Tool: {}", descriptor.tool_name);
    eprintln!("│ Action: {}", descriptor.description);
    eprintln!("└──────────────────────────────────────────────");
    eprintln!("  [a] Allow once");
    eprintln!("  [A] Allow always");
    eprintln!("  [d] Deny once");
    eprintln!("  [D] Deny always");
    eprint!("Choice: ");
    io::stderr().flush().unwrap();

    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).unwrap();

    match line.trim() {
        "a" => PermissionResponse::AllowOnce,
        "A" => PermissionResponse::AllowAlways,
        "d" | "n" => PermissionResponse::DenyOnce,
        "D" | "N" => PermissionResponse::DenyAlways,
        _ => PermissionResponse::DenyOnce, // Default deny
    }
}
```

**Rationale**:
- Box drawing characters (┌─│└) provide visual structure without TUI
- All prompts to stderr (keeps stdout clean for responses)
- Single-letter input minimizes typing
- Clear default ([y/N] means default is No)
- Matches Linux CLI conventions (apt, systemctl, etc.)

**ANSI Color Codes for Readability**:

**Decision**: Use colored crate (already in dependencies) for terminal colors

```rust
use colored::Colorize;

// Status messages
eprintln!("{} Processing query...", "⠋".cyan());
eprintln!("{} Complete!", "✓".green());
eprintln!("{} Error: Failed to execute", "✗".red());

// Tool execution output
println!("{}", "Tool: Bash".bold().blue());
println!("  Command: {}", cmd.yellow());
println!("  Output:");
for line in output.lines() {
    println!("    {}", line.dimmed());
}

// Permission prompts
eprintln!("{}", "Permission Required".bold().yellow());
eprintln!("  Tool: {}", tool_name.cyan());
```

**Rationale**:
- `colored` crate already in dependencies (zero new deps)
- ANSI colors work in all modern terminals
- Improves readability without TUI complexity
- Graceful degradation (colors stripped if terminal doesn't support)

**Stdout vs Stderr Strategy**:

**Decision**: Strict separation of output streams

| Stream | Purpose | Content |
|--------|---------|---------|
| **stdout** | Primary output | LLM responses, command results, final answers |
| **stderr** | Status/metadata | Spinners, progress, permission prompts, errors |

**Rationale**:
- Enables output redirection: `@hoosh query > output.txt` (only response captured)
- Status messages don't pollute response text
- Standard Unix convention (errors/status to stderr)
- Supports piping: `@hoosh "list files" | grep ".rs"`

**Response Display Format**:

**Decision**: Display complete response after processing (no streaming in tagged mode)

```rust
pub fn display_response(response: &str) {
    // Clear status line before printing response
    eprint!("\r\x1b[K");

    // Print response to stdout (clean, no prefixes)
    println!("{}", response);
}

// For tool outputs:
pub fn display_tool_output(tool_name: &str, output: &str) {
    eprintln!("\n{} {}", "─".repeat(40).dimmed(), format!("Tool: {}", tool_name).bold());
    println!("{}", output);
    eprintln!("{}\n", "─".repeat(40).dimmed());
}
```

**Rationale**:
- Complete response at once matches spec requirement (FR-015: no TUI components)
- Clear status line before printing prevents overlap
- Tool outputs visually separated from main response
- All output remains in terminal history (spec requirement FR-016)

**Alternatives Considered**:
- **Streaming response with typewriter effect**: Rejected - adds complexity, not needed for tagged mode
- **Rich markdown rendering in terminal**: Rejected - keep terminal-native, simple text output
- **Progress bars instead of spinners**: Rejected - spinners more compact, less intrusive
- **TUI-lite with termion/crossterm**: Rejected - spec explicitly requires no TUI components (FR-015)

**Error Handling**:

**Decision**: Display errors clearly, don't crash terminal

```rust
pub fn display_error(error: &str) {
    eprint!("\r\x1b[K"); // Clear spinner line
    eprintln!("{} {}", "✗".red().bold(), error.red());
    std::process::exit(1);
}

pub fn display_warning(warning: &str) {
    eprintln!("{} {}", "⚠".yellow(), warning.yellow());
}
```

**Rationale**:
- Clear spinner before showing error (prevents overlap)
- Exit with code 1 (standard error convention, enables shell error handling)
- Warnings visible but don't exit (user can continue)

---

## Summary of Decisions

### Terminal Mode Detection
- **Primary**: CLI flag → config file → auto-detect → default inline
- **Auto-detect**: Check `TERM_PROGRAM` and `VSCODE_*` environment variables
- **Fallback**: Warn user if VSCode detected but inline mode active
- **No automatic switching**: Explicit user control preferred

### Fullview Scrolling
- **Implementation**: ScrollState struct with offset/content_height/viewport_height
- **Input Methods**: Arrow keys, PageUp/PageDown, vim j/k, mouse wheel
- **Performance**: Viewport windowing (render only visible lines)
- **Auto-scroll**: Snap to bottom on new messages

### Session Files
- **Identifier**: Shell PID (via `$PPID`) + creation timestamp
- **Schema**: Minimal JSON with messages and extensible context
- **Location**: `~/.hoosh/sessions/session_{PID}.json`
- **Cleanup**: On-demand (every invocation) + explicit command + 7-day age threshold
- **Validation**: Check PID exists and creation timestamp vs shell start time

### Shell Integration
- **Approach**: Shell function (not alias) for proper argument handling
- **Shells Supported**: Bash, Zsh, Fish, PowerShell (95% coverage target)
- **Installation**: Guided setup with backup, conflict detection, verification
- **Config Locations**: `~/.bashrc`, `~/.zshrc`, `~/.config/fish/functions/@hoosh.fish`, PowerShell `$PROFILE`

### Terminal-Native Output
- **Spinners**: Reuse existing braille patterns with carriage return animation
- **Prompts**: Text-based with box drawing characters, single-letter input
- **Colors**: Use `colored` crate for ANSI color codes
- **Stream Separation**: stdout for responses, stderr for status/errors
- **Display**: Complete response after processing (no streaming)

---

## Risk Assessment

### Low Risk
- ✅ Braille spinners - proven pattern already in codebase
- ✅ Shell config file locations - well-documented standards
- ✅ ANSI color codes - universal terminal support

### Medium Risk
- ⚠️ VSCode terminal detection - environment variables may vary across versions
  - **Mitigation**: Explicit CLI flag override, clear documentation
- ⚠️ Session file cleanup - orphaned files if cleanup fails
  - **Mitigation**: Multiple cleanup strategies, manual command available
- ⚠️ Fullview scroll performance - large conversations (>1000 messages)
  - **Mitigation**: Viewport windowing, reuse optimized MessageRenderer

### High Risk
- 🔴 PID reuse edge cases - wrong session loaded if PID recycled
  - **Mitigation**: Use PID + timestamp validation, discard if mismatch detected
- 🔴 Shell function conflicts - existing `@hoosh` command
  - **Mitigation**: Conflict detection before installation, user prompt for resolution

---

## Dependencies

**No new external dependencies required.**

**Existing dependencies used**:
- `ratatui` 0.29.0 - Fullview viewport management
- `crossterm` 0.27.0 - Terminal events (mouse, keyboard), viewport control
- `colored` 2.0 - ANSI color codes for terminal output
- `serde`/`serde_json` - Session file serialization
- `chrono` - Timestamp handling for session files
- `dirs` 6.0.0 - Home directory path resolution
- `which` 6.0 - Shell binary detection
- `tokio` - Async runtime (existing infrastructure)

---

## Implementation Checklist

### Phase 0 (This Document)
- [x] Research terminal mode detection strategies
- [x] Document fullview scrolling patterns
- [x] Design session file schema and cleanup
- [x] Define shell integration approach
- [x] Specify terminal-native output patterns

### Phase 1 (Next Steps)
- [ ] Create data-model.md with TerminalMode, SessionFile, TerminalSession entities
- [ ] Create contracts/ directory with SessionFile JSON schema, shell alias templates
- [ ] Create quickstart.md with mode selection guide, setup instructions

### Phase 2 (Implementation)
- [ ] Implement TerminalMode enum and detection logic
- [ ] Implement ScrollState and fullview input handlers
- [ ] Implement SessionFile storage and cleanup
- [ ] Implement shell setup command and alias installation
- [ ] Implement tagged mode runner with terminal-native output
- [ ] Add terminal_mode and session_context_enabled config fields
- [ ] Write integration tests for all three modes

---

## Notes

**Backward Compatibility**:
- All new features are opt-in (inline mode remains default)
- Existing config files work without modification
- No breaking changes to current behavior

**Future Extensions**:
- Session file encryption for sensitive conversations
- Auto-detect VSCode and suggest fullview mode
- Windows CMD shell support (currently PowerShell only)
- Nushell custom command registration
- Runtime mode switching (after initial implementation stabilizes)

**Performance Targets** (from spec):
- Session file I/O: <1ms (achievable with JSON + SSD)
- Fullview resize: <200ms (viewport windowing + existing renderer)
- Tagged mode return to shell: <1s (non-TUI eliminates rendering overhead)
