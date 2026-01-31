# Data Model: Terminal Display Modes

**Feature**: 002-terminal-modes
**Date**: 2026-01-23
**Status**: Phase 1 Design

## Overview

This document defines the data models for supporting three terminal display modes (inline, fullview, tagged) and session file persistence for tagged mode context.

## Core Entities

### 1. TerminalMode

**Purpose**: Represents the active terminal display mode for a hoosh session.

**Definition**:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalMode {
    /// Default mode: grows dynamically with Viewport::Inline
    /// Works with native terminal scrollback
    Inline,

    /// Fullscreen mode: Viewport::Fullscreen with internal scrolling
    /// Compatible with VSCode terminals and broken scrollback environments
    Fullview,

    /// Non-hijacking mode: Terminal-native output, shell integration
    /// Uses @hoosh alias, session file persistence, returns control to shell
    Tagged,
}

impl Default for TerminalMode {
    fn default() -> Self {
        Self::Inline
    }
}

impl FromStr for TerminalMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "inline" => Ok(Self::Inline),
            "fullview" => Ok(Self::Fullview),
            "tagged" => Ok(Self::Tagged),
            _ => Err(anyhow!("Invalid terminal mode: {}", s)),
        }
    }
}
```

**State Transitions**: None. Mode is selected at startup and remains constant during session.

**Selection Priority** (highest to lowest):
1. CLI flag: `--mode <inline|fullview|tagged>`
2. Project config: `.hoosh/config.toml` → `terminal_mode = "..."`
3. Global config: `~/.hoosh/config.toml` → `terminal_mode = "..."`
4. Default: `Inline`

---

### 2. SessionFile

**Purpose**: Persists conversation context for tagged mode across multiple @hoosh invocations within the same terminal session.

**Definition**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFile {
    /// Terminal process ID (from $PPID)
    pub terminal_pid: u32,

    /// Session creation timestamp
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,

    /// Last access timestamp (updated on every load/save)
    #[serde(with = "chrono::serde::ts_seconds")]
    pub last_accessed: DateTime<Utc>,

    /// Conversation messages (full history)
    pub messages: Vec<ConversationMessage>,

    /// Extensible metadata (e.g., current working directory, tool state)
    #[serde(default)]
    pub context: HashMap<String, serde_json::Value>,
}

impl SessionFile {
    /// Create a new session file for the given terminal PID
    pub fn new(terminal_pid: u32) -> Self {
        let now = Utc::now();
        Self {
            terminal_pid,
            created_at: now,
            last_accessed: now,
            messages: Vec::new(),
            context: HashMap::new(),
        }
    }

    /// Update last_accessed timestamp
    pub fn touch(&mut self) {
        self.last_accessed = Utc::now();
    }

    /// Check if session file is stale (>7 days old)
    pub fn is_stale(&self, threshold_days: i64) -> bool {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.last_accessed);
        duration.num_days() > threshold_days
    }
}
```

**Storage Location**: `~/.hoosh/sessions/session_{PID}.json`

**Lifecycle**:
1. **Creation**: First @hoosh invocation in tagged mode (when `session_context_enabled = true`)
2. **Load**: Subsequent @hoosh invocations read from file
3. **Update**: After each @hoosh invocation completes, save messages + touch timestamp
4. **Cleanup**: Files with `last_accessed > 7 days` old are deleted

**Size Estimates**:
- Empty session: ~200 bytes
- 10-message conversation: ~2-5 KB
- 100-message conversation: ~20-50 KB

**Performance Target**: <1ms for read/write operations (verified in research.md)

---

### 3. TerminalSession

**Purpose**: Captures the terminal environment context for mode selection and capabilities detection.

**Definition**:
```rust
#[derive(Debug, Clone)]
pub struct TerminalSession {
    /// Active terminal mode
    pub mode: TerminalMode,

    /// Terminal dimensions (width, height)
    pub dimensions: (u16, u16),

    /// Terminal capabilities
    pub capabilities: TerminalCapabilities,

    /// Terminal process ID (for session file lookup)
    pub pid: u32,
}

impl TerminalSession {
    /// Detect and create terminal session from environment
    pub fn detect(explicit_mode: Option<TerminalMode>) -> Result<Self> {
        let capabilities = TerminalCapabilities::detect()?;
        let dimensions = crossterm::terminal::size()?;
        let pid = get_terminal_pid()?;

        let mode = explicit_mode.unwrap_or_else(|| {
            // Auto-detect logic here (from research.md)
            if capabilities.is_vscode {
                TerminalMode::Fullview
            } else {
                TerminalMode::Inline
            }
        });

        Ok(Self {
            mode,
            dimensions,
            capabilities,
            pid,
        })
    }
}
```

---

### 4. TerminalCapabilities

**Purpose**: Encapsulates detected terminal features and environment type.

**Definition**:
```rust
#[derive(Debug, Clone)]
pub struct TerminalCapabilities {
    /// Mouse events supported (for fullview scrolling)
    pub supports_mouse: bool,

    /// Running in VSCode integrated terminal
    pub is_vscode: bool,

    /// Running in iTerm2
    pub is_iterm: bool,

    /// TERM_PROGRAM environment variable
    pub term_program: Option<String>,

    /// COLORTERM environment variable
    pub colorterm: Option<String>,
}

impl TerminalCapabilities {
    pub fn detect() -> Result<Self> {
        let term_program = std::env::var("TERM_PROGRAM").ok();
        let colorterm = std::env::var("COLORTERM").ok();

        let is_vscode = term_program.as_deref() == Some("vscode")
            || std::env::var("VSCODE_GIT_IPC_HANDLE").is_ok();

        let is_iterm = term_program.as_deref() == Some("iTerm.app");

        // Mouse support check (most modern terminals support it)
        let supports_mouse = !matches!(
            std::env::var("TERM").ok().as_deref(),
            Some("dumb") | Some("unknown")
        );

        Ok(Self {
            supports_mouse,
            is_vscode,
            is_iterm,
            term_program,
            colorterm,
        })
    }
}
```

---

### 5. AppConfig Extensions

**Purpose**: Configuration fields for terminal mode selection and session context control.

**Additions to existing AppConfig**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // ... existing fields (default_backend, backends, etc.) ...

    /// Terminal display mode selection
    /// Priority: CLI flag > project config > global config > default (inline)
    #[serde(default)]
    pub terminal_mode: Option<TerminalMode>,

    /// Enable session context persistence for tagged mode
    /// Default: true (contexts persist across @hoosh invocations)
    /// Set to false to disable session file creation (ephemeral mode)
    #[serde(default = "default_session_context_enabled")]
    pub session_context_enabled: bool,
}

fn default_session_context_enabled() -> bool {
    true
}
```

**Config File Example** (`.hoosh/config.toml`):
```toml
# Terminal display mode: "inline" | "fullview" | "tagged"
# Default: "inline"
terminal_mode = "fullview"

# Enable session context persistence for tagged mode
# Default: true
session_context_enabled = true
```

---

### 6. ScrollState (Fullview Mode)

**Purpose**: Tracks scroll position for fullview mode internal scrolling.

**Definition**:
```rust
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    /// Current scroll offset (lines from top)
    pub offset: usize,

    /// Total content height (lines)
    pub content_height: usize,

    /// Viewport height (visible lines)
    pub viewport_height: usize,

    /// Scroll velocity for smooth scrolling (lines per tick)
    pub velocity: f32,
}

impl ScrollState {
    pub fn new(viewport_height: usize) -> Self {
        Self {
            offset: 0,
            content_height: 0,
            viewport_height,
            velocity: 0.0,
        }
    }

    /// Scroll down by N lines
    pub fn scroll_down(&mut self, lines: usize) {
        let max_offset = self.content_height.saturating_sub(self.viewport_height);
        self.offset = (self.offset + lines).min(max_offset);
    }

    /// Scroll up by N lines
    pub fn scroll_up(&mut self, lines: usize) {
        self.offset = self.offset.saturating_sub(lines);
    }

    /// Scroll to bottom (auto-scroll on new messages)
    pub fn scroll_to_bottom(&mut self) {
        self.offset = self.content_height.saturating_sub(self.viewport_height);
    }

    /// Check if scrolled to bottom (for auto-scroll detection)
    pub fn is_at_bottom(&self) -> bool {
        self.offset >= self.content_height.saturating_sub(self.viewport_height)
    }
}
```

---

## Relationships

```text
TerminalSession (1) ───┬──> (1) TerminalMode
                       │
                       └──> (1) TerminalCapabilities
                       │
                       └──> (0..1) SessionFile
                                    (only if mode == Tagged
                                     && session_context_enabled)

SessionFile (1) ──────────> (*) ConversationMessage
                             (existing model from agent/conversation.rs)

AppConfig (1) ────────────> (0..1) TerminalMode
                             (config overrides)
```

---

## Validation Rules

### TerminalMode
- Must be one of: `inline`, `fullview`, `tagged` (case-insensitive)
- Default: `inline` if not specified

### SessionFile
- `terminal_pid` must be > 0
- `created_at` must be <= `last_accessed`
- `messages` array can be empty (new session)
- File size should not exceed 10 MB (warn if approaching limit)

### TerminalSession
- `dimensions` width/height must be > 0
- `pid` must be > 0
- If `mode == Tagged`, `pid` must be resolvable to a session file path

### ScrollState
- `offset` must be <= `content_height - viewport_height`
- `viewport_height` must be > 0
- `content_height` must be >= 0

---

## Performance Considerations

### Session File I/O
- **Target**: <1ms for read/write
- **Strategy**:
  - Use `serde_json` for serialization (fast)
  - Buffer writes (write on session exit, not per message)
  - Lazy cleanup (check stale files only on startup)

### Fullview Scrolling
- **Target**: <16ms frame time (60 FPS)
- **Strategy**:
  - Viewport windowing (only render visible lines)
  - Diff-based rendering (ratatui built-in)
  - Smooth scrolling with velocity damping

### Tagged Mode
- **Target**: <1s total invocation time
- **Strategy**:
  - Skip TUI initialization entirely
  - Direct stdout/stderr writes
  - Minimal session file loading (only messages array)

---

## Migration & Compatibility

### From Inline to Fullview
- No data migration needed
- Conversation context preserved in memory
- Terminal re-initialized with `Viewport::Fullscreen`

### From Inline/Fullview to Tagged
- Create new session file from current conversation
- Switch to terminal-native output
- Preserve conversation storage if enabled

### Session File Schema Evolution
- `context` field is extensible (HashMap)
- Future fields can be added without breaking existing files
- Version field can be added if needed (currently schema is v1 implicit)

---

## Testing Strategy

### Unit Tests
- TerminalMode: FromStr parsing, default value
- SessionFile: new(), touch(), is_stale(), serialization round-trip
- ScrollState: scroll_down(), scroll_up(), boundary conditions
- TerminalCapabilities: detection logic (mock env vars)

### Integration Tests
- Session file persistence: Create → Load → Update → Cleanup
- Mode switching: Verify conversation context preserved
- Fullview scrolling: Simulate resize, scroll events, verify offset
- Tagged mode: Simulate @hoosh invocation, verify session file created

---

## Open Questions

**Q: Should session files include tool call history?**
A: Yes, included in `messages` array (ConversationMessage already supports tool_calls)

**Q: What happens if session file is corrupted?**
A: Log warning, start fresh session, move corrupted file to `.bak`

**Q: Should fullview mode support horizontal scrolling?**
A: No, rely on existing text wrapping (message_renderer.rs)

**Q: Should session files be encrypted?**
A: Not in v1. Marked as future enhancement (same as conversation storage)
