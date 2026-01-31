# Feature Specification: Terminal Display Modes

**Feature Branch**: `002-terminal-modes`
**Created**: 2026-01-23
**Status**: Draft
**Input**: User description: "currently I have terminal in inline, this works well in normal terminal, but in vscode terminals it breaks, I want to have the option to run hoosh in fullview terminal, where scrolling is handled internally and not in native window. Also I want another mode where hoosh can be used in the original terminal by just being tagged with @hoosh in ther native terminal, it does the work loops and then gives the control back to the user instead of hijacking the terminal"

## Clarifications

### Session 2026-01-23

- Q: For mode selection (inline/fullview/tagged), how should the system handle conflicting inputs when both CLI flag and config file are present? → A: CLI flag overrides project config, and project config overrides global config
- Q: In tagged mode, conversation context persists "across multiple @hoosh invocations in the same session" (FR-012). How should the system define and maintain this session boundary? → A: Session tied to terminal process lifetime (context lost when terminal closes)
- Q: For tagged mode, how does hoosh run to monitor for @hoosh tags while allowing normal shell operation? → A: @hoosh is a shell alias/function created during initial setup (in .bashrc/.zshrc) that calls hoosh with remaining arguments; no background monitoring needed
- Q: For fullview mode internal scrolling, what input methods should be supported for navigating through conversation history? → A: Both keyboard navigation (arrow keys, page up/down, vim-style j/k) and mouse wheel scrolling
- Q: When no terminal mode is explicitly specified (no CLI flag, no config setting), which mode should hoosh use by default? → A: Inline mode (preserves current default behavior, backward compatible)
- Q: When using tagged mode (e.g., "@hoosh fix this bug"), how should hoosh handle slash commands like "@hoosh /commit" or "@hoosh /help"? → A: Slash commands work normally (e.g., "@hoosh /commit" executes commit command)
- Q: In tagged mode, when hoosh processes queries (which may involve multiple tool steps), how should output and user interactions be rendered? → A: Completely terminal-native - all output flows naturally to terminal like any bash command, no TUI components, everything stays in terminal history; permission dialogs and reviews use simple text prompts (Linux CLI style) instead of TUI components; responses are displayed in full once complete (no streaming)
- Q: In tagged mode, while hoosh is processing a query, what kind of visual feedback should be shown? → A: Use existing braille spinners; status line format should match inline view
- Q: In tagged mode, how does context persist across multiple @hoosh invocations within the terminal process lifetime? → A: Session files tied to terminal PID (~/.hoosh/sessions/); each @hoosh invocation saves/loads context from disk (JSON read/write ~1ms); automatic cleanup of old sessions; session mechanism is separate from conversation storage feature; config option to enable/disable session context preservation
- Q: In tagged mode, when hoosh writes session context to ~/.hoosh/sessions/[PID].json after each invocation, what should happen if the write fails (e.g., disk full, permission denied, filesystem issues)? → A: Hoosh warns to stderr but completes normally (context lost this turn only)
- Q: In tagged mode, when a user interrupts a running @hoosh command with Ctrl+C (SIGINT), what should happen to the ongoing operation and partial session state? → A: Clean exit with partial context saved (up to interruption point)
- Q: For initial setup, how should hoosh handle shells other than bash/zsh (e.g., fish, PowerShell, nushell)? → A: Support bash/zsh/fish; warn and provide manual instructions for others
- Q: Should hoosh attempt to auto-detect the terminal environment and suggest or auto-select the appropriate mode? → A: Detect and warn/suggest appropriate mode, but respect user's explicit choice
- Q: The spec mentions automatic cleanup of stale session files (7 days old). When should this cleanup actually execute? → A: Check and cleanup on each hoosh startup (session dir scan)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Fullview Terminal Mode (Priority: P1)

Users running hoosh in environments where the inline mode breaks (like VSCode terminals) need a fullview mode where hoosh takes over the entire terminal viewport and handles scrolling internally instead of relying on the native terminal's scrolling.

**Why this priority**: This addresses a critical compatibility issue preventing users from using hoosh in VSCode terminals, which is a common development environment. Without this, the tool is effectively broken for a significant portion of potential users.

**Independent Test**: Can be fully tested by launching hoosh with a fullview flag in a VSCode terminal and verifying that the interface renders correctly, scrolling works internally, and the user can interact with the chat interface without visual corruption.

**Acceptance Scenarios**:

1. **Given** hoosh is launched in fullview mode in a VSCode terminal, **When** the user sends messages and receives responses, **Then** the interface renders correctly with proper text wrapping and no visual corruption
2. **Given** hoosh is running in fullview mode with content exceeding the viewport height, **When** the user scrolls using arrow keys, page up/down, vim-style j/k, or mouse wheel, **Then** scrolling is handled internally by hoosh without relying on terminal scrollback
3. **Given** hoosh is in fullview mode, **When** the terminal window is resized, **Then** the interface adapts gracefully to the new dimensions

---

### User Story 2 - Tagged Non-Hijacking Mode (Priority: P2)

Users who want to integrate hoosh into their existing terminal workflow need a mode where they can invoke hoosh via @hoosh command in their native terminal. The @hoosh command (implemented as a shell alias/function) calls hoosh, which processes the request using completely terminal-native output (no TUI components), displays responses inline, and returns control to the user's normal terminal session. All output stays in terminal history like any other bash command.

**Why this priority**: This enables a fundamentally different use case - non-disruptive integration into existing workflows. While important, it's lower priority than fixing the broken VSCode experience because users can still use hoosh in dedicated terminals.

**Independent Test**: Can be fully tested by running initial setup to create the @hoosh alias, typing "@hoosh [query]" in the terminal, observing that hoosh processes the request with terminal-native output, and verifying that the terminal prompt returns for normal shell commands afterward.

**Acceptance Scenarios**:

1. **Given** the @hoosh alias has been set up, **When** the user types "@hoosh what is the weather" in their shell, **Then** hoosh processes the query, displays terminal-native output, and returns the terminal prompt
2. **Given** the @hoosh alias exists, **When** the user types normal shell commands without @hoosh, **Then** the commands execute normally as the alias is not invoked
3. **Given** the user invokes @hoosh with a multi-step query requiring tool execution, **When** hoosh is processing, **Then** all tool outputs and responses flow naturally to terminal like any bash command output
4. **Given** hoosh needs user permission (e.g., file write confirmation), **When** the permission dialog is presented in tagged mode, **Then** a simple text prompt is used (Linux CLI style) instead of TUI components
5. **Given** hoosh completes processing a @hoosh request, **When** the user scrolls terminal history, **Then** all hoosh output remains visible in scrollback like any other command output
6. **Given** the user types "@hoosh /commit" or other slash commands, **When** hoosh processes the command, **Then** the slash command executes normally as it would in other modes
7. **Given** the user runs "@hoosh analyze this file" and then "@hoosh what did you find", **When** the second @hoosh invocation runs, **Then** it has access to the conversation context from the first invocation via the session file
8. **Given** session context preservation is enabled, **When** the terminal closes, **Then** the session file is marked for cleanup (removed after 7 days or by cleanup process)

---

### User Story 3 - Inline Mode Enhancement (Priority: P3)

Users running hoosh in standard terminals (non-VSCode) can continue using the current inline mode where conversation output flows naturally with terminal scrollback.

**Why this priority**: This is the existing functionality that already works well in normal terminals. The priority is to maintain compatibility while adding the new modes rather than enhancing it.

**Independent Test**: Can be fully tested by launching hoosh in inline mode in a standard terminal emulator and verifying existing behavior is preserved.

**Acceptance Scenarios**:

1. **Given** hoosh is launched in inline mode in a standard terminal, **When** the user has a conversation, **Then** messages flow naturally with terminal scrollback as they currently do
2. **Given** hoosh is in inline mode, **When** the user scrolls using terminal scrollback, **Then** the conversation history is accessible via native terminal scrolling

---

### Edge Cases

- What happens when the user switches terminal modes while hoosh is running?
- How does the system handle "@hoosh" invocation with no query arguments?
- What happens in fullview mode when the terminal is extremely small (< 80x24)?
- How does hoosh detect whether it's running in a VSCode terminal vs a standard terminal? → Check TERM_PROGRAM environment variable; emit warning if inline mode selected in VSCode, but respect user's explicit choice
- What happens if the @hoosh alias conflicts with an existing command or alias?
- How does fullview mode handle terminal color scheme changes or theme switches?
- What happens if standard input/output is redirected when using @hoosh?
- How does initial setup handle shells other than bash/zsh (e.g., fish, PowerShell)? → Automated setup supports bash/zsh/fish; for others, setup warns and provides manual instructions showing the alias command to add
- What happens if the user manually modifies or removes the @hoosh alias?
- How does tagged mode handle extremely long outputs that exceed typical terminal buffer sizes?
- What happens if the user interrupts (Ctrl+C) a @hoosh command mid-execution in tagged mode? → Clean exit with partial context saved (conversation state up to interruption point)
- How are permission prompts formatted in tagged mode to ensure they're clear without TUI highlighting?
- What happens if a session file becomes corrupted or has invalid JSON? → On read failure or invalid JSON, hoosh warns to stderr and starts fresh stateless session; on write failure, hoosh warns to stderr but completes normally (context lost this turn only)
- How does the system handle session files from crashed terminal processes with reused PIDs? → Cleanup runs on each hoosh startup, removing files >7 days old; PID reuse within 7 days would overwrite old session (acceptable edge case given low probability and short retention window)
- What happens if session context preservation is disabled mid-session?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST support three distinct terminal display modes: inline, fullview, and tagged
- **FR-002**: System MUST allow users to select terminal mode via command-line flag, project config, or global config, with precedence order: CLI flag > project config > global config; when no mode is specified, system defaults to inline mode
- **FR-003**: Fullview mode MUST render the interface within the terminal viewport without relying on native scrollback
- **FR-004**: Fullview mode MUST handle internal scrolling for conversations exceeding viewport height using keyboard navigation (arrow keys, page up/down, vim-style j/k) and mouse wheel scrolling
- **FR-005**: Fullview mode MUST respond to terminal resize events and reflow content appropriately
- **FR-006**: System MUST provide initial setup that detects user's shell type and creates @hoosh alias/function in appropriate config file; automated setup supports bash (.bashrc), zsh (.zshrc), and fish (config.fish); for unsupported shells, setup emits warning and provides manual instructions
- **FR-007**: The @hoosh alias/function MUST call hoosh with remaining command-line arguments
- **FR-008**: Tagged mode MUST return control to the native terminal shell after completing each request
- **FR-009**: Tagged mode MUST display processing status while handling a query using existing braille spinners with status line format matching inline view
- **FR-010**: Inline mode MUST preserve current behavior for compatibility with standard terminals
- **FR-011**: Each mode MUST maintain conversation context between interactions; for tagged mode, context persists across multiple @hoosh invocations using session files tied to terminal PID (stored in ~/.hoosh/sessions/), independent of the conversation storage feature
- **FR-012**: System MUST provide clear documentation on when to use each mode and how to switch between them
- **FR-013**: Tagged mode MUST support all slash commands (e.g., /commit, /help) with the same functionality as inline and fullview modes; in tagged mode, slash commands use simple text prompts (per FR-016) instead of TUI components
- **FR-014**: Tagged mode MUST use terminal-native output only - all responses, tool outputs, and status messages flow naturally to stdout/stderr without TUI components
- **FR-015**: Tagged mode MUST NOT clear or redraw terminal content - all output persists in terminal history like standard bash command output
- **FR-016**: Tagged mode MUST use simple text prompts for user interactions (permission dialogs, reviews, confirmations) instead of TUI components used in inline/fullview modes
- **FR-017**: Tagged mode session files MUST be stored in ~/.hoosh/sessions/ directory with filenames based on terminal process PID
- **FR-018**: Each @hoosh invocation MUST save context to the session file after completion and load context from the session file at startup
- **FR-019**: System MUST perform automatic cleanup of stale session files (older than 7 days) on each hoosh startup by scanning ~/.hoosh/sessions/ directory and removing expired files
- **FR-020**: System MUST provide a configuration option to enable/disable session context preservation in tagged mode
- **FR-021**: Session file mechanism MUST operate independently of the conversation storage feature (which can be enabled/disabled separately)
- **FR-022**: When session file write fails in tagged mode, system MUST emit warning to stderr and complete the command normally; context is lost only for that invocation
- **FR-023**: Tagged mode MUST handle SIGINT (Ctrl+C) by immediately stopping the current operation and saving partial context (conversation state up to interruption point) before exiting
- **FR-024**: System MUST detect terminal environment type (e.g., VSCode terminal via TERM_PROGRAM environment variable) and emit a warning to stderr if the selected mode is potentially incompatible; system respects user's explicit mode choice regardless of warnings

### Key Entities

- **Terminal Mode**: Represents the display/interaction mode (inline, fullview, or tagged), with associated rendering and input handling behavior
- **Terminal Session**: Represents the active terminal environment with properties like dimensions, capabilities, environment type (VSCode vs standard), and process lifetime boundary (conversation context is tied to terminal process lifetime)
- **Conversation Context**: Maintains message history and state that persists across mode operations
- **Session File**: JSON file stored in ~/.hoosh/sessions/ named by terminal PID; contains conversation context for tagged mode; automatically cleaned up after terminal closes or after 7 days; independent of conversation storage feature

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can successfully run hoosh in VSCode terminals without visual corruption or interface issues
- **SC-002**: In tagged mode, users can alternate between shell commands and @hoosh queries with control returning to the shell prompt within 1 second after response completion
- **SC-003**: Fullview mode correctly handles terminal resize events with interface reflow completing within 200ms
- **SC-004**: 95% of users can select the appropriate mode for their environment based on documentation
- **SC-005**: Conversation context is maintained across all mode operations with zero message loss
- **SC-006**: Initial setup automatically creates @hoosh alias for bash, zsh, and fish shells; provides clear manual instructions for other shells with 100% success rate
