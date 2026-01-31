# Implementation Plan: Input Field Refinement

**Branch**: `003-input-field-refinement` | **Date**: 2025-12-11 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/003-input-field-refinement/spec.md`

## Summary

Refine the TUI input field to handle large text pastes gracefully, implement automatic text wrapping at terminal width boundaries, provide an expanded editor mode for comfortable multi-line editing, and add attachment management capabilities. The implementation uses the existing `tui-textarea` widget infrastructure and introduces a new attachment system for managing large pasted content.

**Key Technical Approach**:
- Leverage existing `tui-textarea` (v0.4.0) already in dependencies
- Create attachment storage system (in-memory, session-scoped)
- Implement paste detection and size-based routing (≤200 chars inline, >200 chars attachment, >5MB rejected)
- Add visual wrapping indicators using Unicode symbols (↩ or ⤶)
- Create expanded editor mode (50-60% terminal height) with Ctrl+E / Esc keybindings
- Maintain compatibility with existing input handling infrastructure

## Technical Context

**Language/Version**: Rust 2024 edition (matches Cargo.toml:4)
**Primary Dependencies**:
- ratatui 0.29.0 (TUI framework with scrolling regions)
- tui-textarea 0.4.0 (already present - multi-line text editing)
- crossterm 0.27.0 (terminal control & events)
- tokio 1.0 (async runtime)
- arboard 3.4 (clipboard access)
- unicode-width 0.1 (character width calculations for wrapping)

**Storage**: In-memory only (attachments are ephemeral, cleared after submission)
**Testing**: cargo test with tokio::test for async code
**Target Platform**: Terminal applications on Linux/macOS/Windows
**Project Type**: Single binary TUI application
**Performance Goals**:
- Paste handling: <100ms for content classification
- Terminal resize rewrapping: <100ms (SC-005)
- Attachment operations: <30 seconds per attachment (SC-007)

**Constraints**:
- 200 character threshold for attachment creation
- 5MB maximum attachment size
- No persistence across sessions (ephemeral state)
- 50-60% terminal height for expanded editor
- Text wrapping must respect terminal width boundaries

**Scale/Scope**:
- Support 10,000+ character pastes (SC-001)
- Terminal widths: 80-240 columns (SC-002)
- 100+ lines in expanded editor (SC-003)
- 50+ lines of mixed wrapped/hard-break content navigation (SC-006)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### I. Test-First Development ✅ PASS

**Assessment**: Feature requires comprehensive testing across multiple areas:
- Unit tests: Paste size detection, attachment creation/deletion, wrapping logic
- Integration tests: Input mode switching, attachment expansion on submit, terminal resize handling
- Mock objects: Can mock clipboard operations, terminal dimensions

**Test Strategy**:
- Happy path: Normal paste, expanded mode toggle, attachment lifecycle
- Tool execution: N/A (no agent tool calls in this feature)
- Error handling: >5MB paste rejection, invalid attachment references
- State & events: Mode switching preserves content, terminal resize triggers rewrap

**Compliance**: All new functionality will have corresponding tests following builder pattern for setup.

### II. Trait-Based Design & Dependency Injection ⚠️ PARTIAL

**Assessment**: Limited need for traits in this feature as it's primarily UI-focused.

**Approach**:
- Attachment storage can use a trait (`AttachmentStore`) to enable testing
- Clipboard interaction already uses `arboard` (existing dependency)
- Terminal dimension access via ratatui (existing abstraction)

**Justification**: No new external integrations require trait-based abstraction. Feature extends existing TUI infrastructure.

### III. Single Responsibility Principle ✅ PASS

**Assessment**: Feature naturally decomposes into focused modules:
- `attachment.rs` - Attachment entity and storage logic
- `paste_detector.rs` - Paste size classification
- `wrapping.rs` - Text wrapping with visual indicators
- `expanded_editor.rs` - Expanded mode state and rendering
- Extend existing `src/tui/components/input.rs` for integration

**Compliance**: Each module has single, clear responsibility.

### IV. Flat Module Structure ✅ PASS

**Assessment**: All new code lives under `src/tui/` (existing structure):
- `src/tui/input/` for input-related modules (attachment, wrapping, expanded mode)
- No deep nesting required

**Compliance**: Maintains project's flat structure.

### V. Clean Code Practices ✅ PASS

**Assessment**:
- Naming: `TextAttachment` (entity), `PasteDetector` (service), `create_attachment()` (function)
- Error handling: Use `anyhow::Result` for fallible operations (size checks, clipboard access)
- Idiomatic Rust: `Arc` for shared attachment store if needed across async contexts
- No obvious comments: Code will be self-documenting via descriptive names

**Compliance**: Follows project conventions from CLAUDE.md.

### Summary

✅ **ALL GATES PASSED** - No constitutional violations. Feature aligns with all principles.

## Project Structure

### Documentation (this feature)

```text
specs/003-input-field-refinement/
├── plan.md              # This file
├── research.md          # Phase 0 output (technical decisions)
├── data-model.md        # Phase 1 output (attachment entity model)
├── quickstart.md        # Phase 1 output (user guide)
└── tasks.md             # Phase 2 output (NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
src/
├── tui/
│   ├── components/
│   │   ├── input.rs                    # MODIFY: Add mode switching, attachment rendering
│   │   └── expanded_editor.rs          # NEW: Expanded editor component
│   ├── input/                          # NEW: Input-related modules
│   │   ├── mod.rs                      # Module declarations
│   │   ├── attachment.rs               # NEW: TextAttachment entity & AttachmentStore
│   │   ├── paste_detector.rs           # NEW: Paste size classification logic
│   │   └── wrapping.rs                 # NEW: Text wrapping with visual indicators
│   ├── handlers/
│   │   ├── paste_handler.rs            # MODIFY: Integrate attachment creation
│   │   ├── text_input_handler.rs       # MODIFY: Add Ctrl+E, Esc handling
│   │   └── attachment_handler.rs       # NEW: Attachment management (view, edit, delete)
│   ├── app_state.rs                    # MODIFY: Add expanded_mode, attachments fields
│   └── actions.rs                      # MODIFY: Add ToggleExpandedMode, ManageAttachments actions
└── lib.rs                              # MODIFY: Re-export input modules if needed

tests/
├── integration/
│   ├── input_attachment_tests.rs       # NEW: Integration tests for attachment workflow
│   ├── input_wrapping_tests.rs         # NEW: Integration tests for wrapping behavior
│   └── input_expanded_mode_tests.rs    # NEW: Integration tests for mode switching
└── unit/
    ├── attachment_tests.rs             # NEW: Unit tests for attachment logic
    ├── paste_detector_tests.rs         # NEW: Unit tests for paste classification
    └── wrapping_tests.rs               # NEW: Unit tests for wrapping calculations
```

**Structure Decision**: Single project structure. All input refinement code lives under `src/tui/` following existing organization. New `input/` submodule groups related functionality (attachment, paste detection, wrapping). Tests are organized by integration vs unit, matching existing project patterns.

## Complexity Tracking

> **No violations to justify** - Constitution check passed all gates.

## Phase 0: Research & Technical Decisions

**Status**: ✅ COMPLETE (research conducted during planning)

Key research topics resolved:

### 1. tui-textarea Capabilities

**Decision**: Use existing `tui-textarea` 0.4.0 for both normal and expanded modes.

**Rationale**:
- Already in dependencies (Cargo.toml:38)
- Handles multi-line editing, cursor navigation, scrolling
- Supports programmatic content manipulation (needed for attachment expansion)
- Proven in current codebase (`AppState.input: TextArea`)

**Alternatives Considered**:
- Custom text widget: Rejected - reinventing wheel, high complexity
- External editor launch: Rejected - breaks UX flow, complicates attachment handling

### 2. Paste Detection Strategy

**Decision**: Detect paste via clipboard content length check on paste events.

**Rationale**:
- `arboard` already provides clipboard access (Cargo.toml:41)
- Crossterm emits paste events we can intercept
- Character count simple to implement (`.len()` on String)

**Alternatives Considered**:
- Line count: Rejected - clarification specified character count only
- Heuristic detection: Rejected - too unreliable, false positives

### 3. Attachment Storage Architecture

**Decision**: In-memory `Vec<TextAttachment>` in `AppState`, indexed by sequential IDs.

**Rationale**:
- Ephemeral lifecycle (cleared after submit) doesn't justify persistence
- Spec explicitly states session-scoped (spec.md:131)
- Simple index-based ID generation (1, 2, 3...)
- Fast access for view/edit/delete operations

**Alternatives Considered**:
- HashMap: Rejected - sequential IDs don't need hash lookup overhead
- File-based temp storage: Rejected - spec requires in-memory, adds I/O complexity

### 4. Text Wrapping Implementation

**Decision**: Leverage `unicode-width` (already in deps) for character width calculations + manual wrapping logic.

**Rationale**:
- `unicode-width` handles complex Unicode correctly (Cargo.toml:43)
- Terminal width available via ratatui's `Rect` dimensions
- Visual indicators (↩/⤶) insertable as display-only markers

**Alternatives Considered**:
- `textwrap` crate: Already in deps (Cargo.toml:42) but designed for static text, not TUI live wrapping
- ratatui paragraph wrapping: Exists but doesn't provide soft-wrap indicator injection points

### 5. Expanded Editor Layout

**Decision**: Render expanded editor as overlay covering 50-60% of terminal height, centered vertically.

**Rationale**:
- Clarification specified 50-60% (spec.md:20)
- Overlay preserves conversation context visibility
- Centered position balances screen real estate

**Alternatives Considered**:
- Replace entire screen: Rejected - loses context, disorienting
- Horizontal split: Rejected - wastes vertical space for editing

**Output**: research.md (see below)

---

## Phase 1: Design & Contracts

### Data Model

**Output**: data-model.md (see below)

#### Entities

**TextAttachment**

```rust
pub struct TextAttachment {
    pub id: usize,
    pub content: String,
    pub size_chars: usize,
    pub line_count: usize,
    pub created_at: Instant,
}
```

**Validation Rules**:
- `size_chars <= 5_000_000` (5MB limit, assuming ~1 byte/char UTF-8)
- `content.len() > 200` (only large pastes become attachments)
- `id` must be unique within session (sequential generation)

**Lifecycle**:
1. Created: On paste event when content.len() > 200
2. Persists: In `AppState.attachments` until submission or deletion
3. Deleted: Explicitly via user action OR automatically on input submission
4. Not serialized: Never saved to disk

**AttachmentStore** (in AppState)

```rust
pub struct AppState {
    // Existing fields...
    pub input: TextArea<'static>,

    // NEW fields:
    pub attachments: Vec<TextAttachment>,
    pub next_attachment_id: usize,
    pub expanded_mode: bool,
    pub attachment_view: Option<AttachmentViewState>, // For viewing/editing
}

pub struct AttachmentViewState {
    pub attachment_id: usize,
    pub editor: TextArea<'static>, // Editing the attachment content
}
```

**InputMode** (extend existing if present, or new enum)

```rust
pub enum InputMode {
    Normal,          // Existing
    Expanded,        // NEW: 50-60% height editor
    AttachmentView,  // NEW: Viewing/editing an attachment
}
```

### API Contracts

**Note**: This is a TUI feature with no network APIs. "Contracts" here refer to internal module interfaces.

#### AttachmentStore Trait

```rust
pub trait AttachmentStore: Send + Sync {
    fn create(&mut self, content: String) -> Result<usize>;
    fn get(&self, id: usize) -> Option<&TextAttachment>;
    fn get_mut(&mut self, id: usize) -> Option<&mut TextAttachment>;
    fn delete(&mut self, id: usize) -> Result<()>;
    fn list(&self) -> Vec<&TextAttachment>;
    fn clear_all(&mut self);
}
```

**Rationale**: Trait enables testing with mock implementations, follows principle II.

#### PasteDetector Interface

```rust
pub struct PasteDetector {
    threshold_chars: usize,
    max_size_bytes: usize,
}

impl PasteDetector {
    pub fn new(threshold: usize, max_size: usize) -> Self;

    pub fn classify_paste(&self, content: &str) -> PasteClassification;
}

pub enum PasteClassification {
    Inline,               // ≤200 chars
    Attachment,           // >200 chars, ≤5MB
    Rejected(String),     // >5MB (error message)
}
```

#### WrappingCalculator Interface

```rust
pub struct WrappingCalculator {
    terminal_width: u16,
    wrap_indicator: char, // ↩ or ⤶
}

impl WrappingCalculator {
    pub fn new(terminal_width: u16, indicator: char) -> Self;

    pub fn wrap_text(&self, text: &str) -> Vec<WrappedLine>;
}

pub struct WrappedLine {
    pub content: String,
    pub is_soft_wrap: bool, // true = show indicator, false = hard line break
}
```

### Agent Context Update

Running agent context update script:

```bash
.specify/scripts/bash/update-agent-context.sh claude
```

**Expected Output**:
- Updates `CLAUDE.md` active technologies section with:
  - tui-textarea 0.4.0 (multi-line text editing)
  - unicode-width 0.1 (character width calculations)
- Preserves manual additions between markers
- Adds new commands if applicable

---

## research.md

**File**: `/Users/armin/Projects/hoosh/specs/003-input-field-refinement/research.md`

### tui-textarea Integration

**Decision**: Use `tui-textarea::TextArea` for both normal input and expanded editor modes.

**Rationale**:
- Already integrated (Cargo.toml:38, used in AppState.input)
- Provides all needed features: multi-line editing, cursor navigation, scrolling
- Supports dynamic resizing (needed for terminal resize handling)
- Can programmatically insert/replace text (needed for attachment expansion)

**Implementation Notes**:
- Normal mode: Existing usage, minimal changes
- Expanded mode: Same `TextArea`, different rendering area (50-60% height)
- Mode switching: Transfer content bidirectionally between same widget instance

**Best Practices**:
- Use `.lines()` to get content for attachment expansion
- Use `.set_cursor_style()` to differentiate modes visually
- Use `.set_block()` to add borders/titles for expanded mode

**Alternatives Considered**:
- Custom widget: 500+ lines of cursor/selection logic, high bug risk
- Separate widgets per mode: Rejected - content duplication, sync issues

### Paste Detection & Classification

**Decision**: Intercept crossterm paste events, classify by character count, route to inline or attachment.

**Rationale**:
- Crossterm emits `Event::Paste(String)` on clipboard paste (Cargo.toml:36)
- Character count via `.len()` is O(1) for String
- 200 char threshold and 5MB limit are simple numeric comparisons
- Existing `paste_handler.rs` already exists (src/tui/handlers/paste_handler.rs:1)

**Implementation Notes**:
```rust
match event {
    Event::Paste(content) => {
        let classification = paste_detector.classify_paste(&content);
        match classification {
            PasteClassification::Inline => {
                // Insert directly into TextArea
                state.input.insert_str(&content);
            }
            PasteClassification::Attachment => {
                // Create attachment, insert reference token
                let id = state.attachments.create(content)?;
                state.input.insert_str(&format!("[pasted text-{}]", id));
            }
            PasteClassification::Rejected(msg) => {
                // Show error (use existing error display mechanism)
                state.show_error(&msg);
            }
        }
    }
}
```

**Edge Case Handling**:
- Exactly 200 chars: Inline (≤ threshold per spec.md:95)
- Binary data: Rust String validates UTF-8, invalid = error automatically
- Unicode/emoji: `unicode-width` calculates display width correctly

**Best Practices**:
- Size check before attachment creation (fail fast on >5MB)
- Atomic operation: attachment created + reference inserted together
- Error messages clear: "Paste rejected: exceeds 5MB limit"

**Alternatives Considered**:
- Heuristic detection (typing speed): Rejected - unreliable, false positives
- Line-based threshold: Rejected - clarification specified chars only (spec.md:12)

### Text Wrapping with Visual Indicators

**Decision**: Calculate wrapping boundaries using `unicode-width`, insert indicator symbols (↩) at soft-wrap points during rendering.

**Rationale**:
- `unicode-width` already in deps (Cargo.toml:43), handles CJK/emoji correctly
- Terminal width from ratatui `Rect.width`
- Indicators are display-only (not in actual content buffer)
- Spec requires distinction between soft/hard wraps (FR-007, spec.md:114)

**Implementation Notes**:
```rust
pub fn wrap_text(text: &str, width: u16) -> Vec<WrappedLine> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') { // Hard breaks preserved
        let mut current_line = String::new();
        let mut current_width = 0;

        for word in paragraph.split_whitespace() {
            let word_width = UnicodeWidthStr::width(word);
            if current_width + word_width + 1 > width {
                // Soft wrap needed
                lines.push(WrappedLine {
                    content: current_line.clone(),
                    is_soft_wrap: true, // Show ↩
                });
                current_line = word.to_string();
                current_width = word_width;
            } else {
                if !current_line.is_empty() {
                    current_line.push(' ');
                    current_width += 1;
                }
                current_line.push_str(word);
                current_width += word_width;
            }
        }
        lines.push(WrappedLine {
            content: current_line,
            is_soft_wrap: false, // Hard break, no indicator
        });
    }
    lines
}
```

**Edge Case Handling**:
- Word longer than width: Force-break at boundary, add indicator (spec.md:100)
- Wide characters (emoji): `unicode-width` returns 2 for emoji, accurate counting
- Terminal resize: Recalculate wrapping on resize event (SC-005 <100ms)

**Best Practices**:
- Cache wrapped result if text + width unchanged (optimization)
- Use visual indicator that's widely supported: ↩ (U+21A9) or ⤶ (U+2936)
- Wrapping respects word boundaries (don't split mid-word) per FR-019

**Alternatives Considered**:
- `textwrap` crate: Designed for static text, doesn't support live indicators
- Character-by-character wrapping: Rejected - ignores word boundaries
- No indicators: Rejected - spec requires visual distinction (FR-007)

### Expanded Editor Layout & Mode Switching

**Decision**: Render expanded editor as centered vertical overlay at 50-60% terminal height, reusing same `TextArea` instance.

**Rationale**:
- Clarification specified 50-60% height (spec.md:20)
- Overlay preserves conversation context visibility above/below
- Single `TextArea` instance avoids content synchronization
- Ctrl+E / Esc keybindings follow TUI conventions (spec.md:18,21)

**Implementation Notes**:
```rust
// In app_loop rendering
match state.input_mode {
    InputMode::Normal => {
        // Render in bottom area (existing behavior)
        Input::new().render(state, input_area, buf);
    }
    InputMode::Expanded => {
        // Calculate expanded area (50-60% of terminal height, centered)
        let term_height = terminal_size.height;
        let editor_height = (term_height * 55 / 100).max(10); // 55% of height, min 10 lines
        let y_offset = (term_height - editor_height) / 2;

        let expanded_area = Rect {
            x: 2, // Small margin
            y: y_offset,
            width: terminal_size.width - 4,
            height: editor_height,
        };

        ExpandedEditor::new().render(state, expanded_area, buf);
    }
}

// In text_input_handler.rs
match key_event {
    KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
        state.input_mode = InputMode::Expanded;
    }
    KeyCode::Esc if state.input_mode == InputMode::Expanded => {
        state.input_mode = InputMode::Normal;
        // Content automatically preserved (same TextArea instance)
    }
}
```

**Mode Transition**:
1. Ctrl+E pressed → Set `input_mode = Expanded`
2. Next render → Calculate expanded area, render `TextArea` there
3. Esc pressed → Set `input_mode = Normal`
4. Next render → Render `TextArea` in bottom area
5. Content never copied, same widget instance throughout

**Best Practices**:
- Block/title in expanded mode: "Expanded Editor (Esc to exit)"
- Scrollbar visible if content exceeds visible area (FR-011)
- Cursor remains at same position during mode switch
- Expanded mode visually distinct: thicker border, different color

**Alternatives Considered**:
- Full-screen takeover: Rejected - loses context (spec.md:72 context must be visible)
- Separate TextArea per mode: Rejected - content sync complexity, memory waste
- Horizontal split: Rejected - vertical space more valuable for editing

### Attachment Management Interface

**Decision**: Add keybinding (Ctrl+A) to open attachment list, select to view/edit, 'd' to delete.

**Rationale**:
- User Story 4 requires list/view/edit/delete (spec.md:81-84)
- TUI pattern: list → select → action
- Existing dialogs (permission, completion) establish interaction pattern

**Implementation Notes**:
```rust
pub enum InputMode {
    Normal,
    Expanded,
    AttachmentList,     // NEW: Showing list of attachments
    AttachmentView,     // NEW: Viewing/editing one attachment
}

// In attachment_handler.rs
pub fn handle_attachment_list_event(state: &mut AppState, event: KeyEvent) {
    match event.code {
        KeyCode::Up => state.attachment_list.select_previous(),
        KeyCode::Down => state.attachment_list.select_next(),
        KeyCode::Enter => {
            // Open selected attachment for viewing/editing
            let id = state.attachment_list.selected_id();
            state.input_mode = InputMode::AttachmentView;
            state.attachment_view = Some(AttachmentViewState {
                attachment_id: id,
                editor: TextArea::from(state.attachments.get(id)?.content.lines()),
            });
        }
        KeyCode::Char('d') => {
            // Delete selected attachment
            let id = state.attachment_list.selected_id();
            state.attachments.delete(id)?;
            // Remove reference from input
            let ref_token = format!("[pasted text-{}]", id);
            state.input.remove_str(&ref_token); // Pseudocode, needs actual impl
        }
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
        }
    }
}
```

**UI Layout**:
```
╭─ Attachments (2) ─────────────────────╮
│ [1] pasted text-1  (1,234 chars, 45 lines) │
│ [2] pasted text-2  (5,678 chars, 123 lines)│
│                                         │
│ Enter: View/Edit  d: Delete  Esc: Close│
╰─────────────────────────────────────────╯
```

**Best Practices**:
- Metadata displayed: ID, size, line count (spec.md:81)
- Editing uses same `TextArea` infrastructure (consistency)
- Saving edits updates attachment content + recalculates size/line count
- Deleting attachment removes reference from input field (spec.md:84)

**Alternatives Considered**:
- Inline editing (edit reference token in input): Rejected - unclear UX, error-prone
- Separate keybindings per action: Rejected - cluttered, discoverable list UI better
- No editing capability: Rejected - spec requires edit (FR-014)

---

## data-model.md

**File**: `/Users/armin/Projects/hoosh/specs/003-input-field-refinement/data-model.md`

# Data Model: Input Field Refinement

## Entities

### TextAttachment

Represents large pasted content stored temporarily and separately from the main input field.

**Attributes**:
- `id: usize` - Unique sequential identifier within session (1, 2, 3...)
- `content: String` - Full text content of the paste
- `size_chars: usize` - Character count (cached for display)
- `line_count: usize` - Number of lines (cached for display)
- `created_at: Instant` - Creation timestamp for metadata

**Constraints**:
- `size_chars > 200` - Only pastes exceeding threshold become attachments
- `size_chars <= 5_000_000` - 5MB limit (~1 byte/char UTF-8 assumption)
- `id` unique within session (enforced by sequential generation)
- `content` must be valid UTF-8 (enforced by Rust `String`)

**Relationships**:
- Referenced in `InputContent` by ID token `[pasted text-{id}]`
- Many-to-one: Multiple attachments can exist per input session
- Owned by `AppState.attachments: Vec<TextAttachment>`

**Lifecycle**:
1. **Created**: On paste event when `content.len() > 200`
2. **Persists**: In memory (`AppState`) until submission or deletion
3. **Deleted**:
   - Explicitly via user action (attachment management UI)
   - Automatically on input submission (all attachments cleared)
4. **Never serialized**: Ephemeral, session-scoped only

**State Transitions**:
```
┌─────────────┐  Paste >200 chars   ┌─────────────┐
│   (None)    │ ───────────────────> │   Active    │
└─────────────┘                      └─────────────┘
                                            │
                           ┌────────────────┴────────────────┐
                           │ User Delete                     │ Submit Input
                           v                                 v
                     ┌──────────┐                      ┌──────────┐
                     │ Deleted  │                      │ Deleted  │
                     └──────────┘                      └──────────┘
                     (removed from Vec)                (all cleared)
```

### InputContent

Represents the main text being composed by the user (existing entity, extended).

**Attributes** (existing):
- `text: String` - Content of the input (managed by `TextArea`)
- `cursor_position: (u16, u16)` - Cursor location (managed by `TextArea`)

**Attributes** (NEW):
- `mode: InputMode` - Current display/editing mode

**Relationships**:
- May reference zero or more `TextAttachment` via ID tokens in text
- Owned by `AppState.input: TextArea`

**Lifecycle**:
1. **Created**: On app startup (empty)
2. **Modified**: User types, pastes, edits
3. **Submitted**: Content (with expanded attachments) sent to LLM
4. **Cleared**: After submission, reset to empty

### InputMode (new enum)

Represents the current input editing mode.

**Values**:
- `Normal` - Standard input area at bottom of screen
- `Expanded` - Enlarged editor (50-60% terminal height)
- `AttachmentList` - Viewing list of attachments
- `AttachmentView` - Viewing/editing one specific attachment

**Relationships**:
- Owned by `AppState.input_mode: InputMode`
- Affects rendering logic in `Input` and `ExpandedEditor` components

**State Transitions**:
```
                    Ctrl+E
     ┌─────────────────────────────────────┐
     │                                     │
     v                                     │
┌─────────┐                           ┌──────────┐
│ Normal  │ <───────── Esc ─────────> │ Expanded │
└─────────┘                           └──────────┘
     │                                     │
     │ Ctrl+A                              │ Ctrl+A
     v                                     v
┌────────────────┐  Enter on item   ┌────────────────┐
│ AttachmentList │ ───────────────>  │ AttachmentView │
└────────────────┘                   └────────────────┘
     ^                                     │
     │                                     │
     └──────────────── Esc ────────────────┘
```

### AttachmentViewState (new struct)

Represents the state when viewing/editing a specific attachment.

**Attributes**:
- `attachment_id: usize` - ID of attachment being viewed
- `editor: TextArea` - Text editor for attachment content
- `is_modified: bool` - Whether content changed since opening

**Relationships**:
- References one `TextAttachment` by ID
- Owned by `AppState.attachment_view: Option<AttachmentViewState>`
- `None` when not viewing an attachment

**Lifecycle**:
1. **Created**: When user selects attachment to view
2. **Modified**: User edits content in editor
3. **Saved**: Content written back to `TextAttachment`, sizes recalculated
4. **Destroyed**: On Esc (discard changes) or Save (commit changes)

## Validation Rules

### Paste Classification

```rust
fn classify_paste(content: &str) -> PasteClassification {
    let size_bytes = content.len();

    if size_bytes > 5_000_000 {
        PasteClassification::Rejected("Paste rejected: exceeds 5MB limit".into())
    } else if content.chars().count() > 200 {
        PasteClassification::Attachment
    } else {
        PasteClassification::Inline
    }
}
```

**Rules**:
- Size check first (fail fast on huge pastes)
- Character count uses `.chars().count()` (not byte length, handles multi-byte UTF-8)
- Exactly 200 chars → Inline (≤ threshold)

### Attachment Reference Token

**Format**: `[pasted text-{id}]`

**Validation**:
- `id` must be numeric and exist in `attachments` vector
- Format is literal (no user customization)
- Token is plain text (not special object) in `TextArea.content`

### Content Expansion on Submit

```rust
fn expand_attachments(input: &str, attachments: &[TextAttachment]) -> String {
    let mut expanded = input.to_string();
    for attachment in attachments {
        let token = format!("[pasted text-{}]", attachment.id);
        expanded = expanded.replace(&token, &attachment.content);
    }
    expanded
}
```

**Rules**:
- All attachment references replaced with full content
- Replacement order doesn't matter (IDs are unique)
- After expansion, attachments cleared (single-use)

## Data Flow

### Paste → Attachment Creation

```
User Paste
    │
    v
┌──────────────────┐
│ Crossterm Event  │
│ Event::Paste(str)│
└──────────────────┘
    │
    v
┌──────────────────┐      >200 chars      ┌────────────────────┐
│ PasteDetector    │ ──────────────────>   │ AttachmentStore    │
│ .classify_paste()│                       │ .create(content)   │
└──────────────────┘                       └────────────────────┘
    │                                              │ Returns ID
    │ ≤200 chars                                   v
    v                                      ┌────────────────────┐
┌──────────────────┐                      │ Insert reference   │
│ Insert inline    │                      │ [pasted text-{id}] │
│ into TextArea    │                      │ into TextArea      │
└──────────────────┘                      └────────────────────┘
```

### Submit → Attachment Expansion

```
User Submit (Enter)
    │
    v
┌──────────────────┐
│ Get input text   │
│ from TextArea    │
└──────────────────┘
    │
    v
┌──────────────────┐
│ expand_attachments│
│ (replace tokens)  │
└──────────────────┘
    │
    v
┌──────────────────┐
│ Send to LLM      │
│ backend          │
└──────────────────┘
    │
    v
┌──────────────────┐
│ Clear attachments│
│ Clear input      │
└──────────────────┘
```

### Mode Switch → Rendering

```
User presses Ctrl+E
    │
    v
┌──────────────────┐
│ Set input_mode = │
│ Expanded         │
└──────────────────┘
    │
    v
┌──────────────────┐
│ Next frame render│
└──────────────────┘
    │
    v
┌──────────────────┐      input_mode?       ┌────────────────────┐
│ App render loop  │ ──── Expanded ───────>  │ ExpandedEditor     │
└──────────────────┘                         │ .render()          │
    │                                         └────────────────────┘
    │ Normal                                         │
    v                                                v
┌──────────────────┐                         ┌────────────────────┐
│ Input (normal)   │                         │ Render TextArea in │
│ .render()        │                         │ 50-60% height area │
└──────────────────┘                         └────────────────────┘
```

---

## quickstart.md

**File**: `/Users/armin/Projects/hoosh/specs/003-input-field-refinement/quickstart.md`

# Quickstart: Refined Input Field

## Overview

The hoosh input field now supports:
- **Large paste handling**: Paste 10,000+ characters without breaking the UI
- **Automatic text wrapping**: Content never extends beyond terminal width
- **Expanded editor mode**: 50-60% screen editor for comfortable multi-line editing
- **Attachment management**: Review, edit, or delete pasted content before submitting

## Basic Usage

### Normal Input

Type normally at the prompt:
```
> Hello, how are you?
```

Press **Enter** to submit.

### Large Paste Handling

When you paste text **over 200 characters**:
1. Content is automatically saved as an attachment
2. A reference token appears in the input: `[pasted text-1]`
3. The UI remains stable and responsive

Example:
```
> I have a question about [pasted text-1]
```

**Note**: When you submit, the full attachment content is automatically expanded inline.

### Text Wrapping

Text automatically wraps at terminal edges:
```
> This is a very long line that will automatically wrap when it reaches
↩ the edge of your terminal window without extending beyond the visible
↩ area
```

The **↩** symbol indicates a soft-wrap (automatic). Hard line breaks (when you press Enter while typing) have no indicator.

### Expanded Editor Mode

For comfortable multi-line editing:

1. Press **Ctrl+E** to open expanded editor
2. Editor occupies 50-60% of screen height
3. Type, edit, navigate normally
4. Press **Esc** to return to normal mode

All content is preserved when switching modes.

```
╭─ Expanded Editor (Esc to exit) ─────────────────╮
│                                                  │
│ This is a longer message that I'm composing     │
│ across multiple lines in the expanded view.     │
│                                                  │
│ I can see more context here and edit           │
│ comfortably before submitting.                   │
│                                                  │
│                                                  │
╰──────────────────────────────────────────────────╯
```

### Attachment Management

To review, edit, or delete attachments:

1. Press **Ctrl+A** to open attachment list
2. Use **↑/↓** to select an attachment
3. Press **Enter** to view/edit the attachment
4. Press **d** to delete the selected attachment
5. Press **Esc** to close and return

```
╭─ Attachments (2) ────────────────────────────────╮
│ [1] pasted text-1  (1,234 chars, 45 lines)       │
│ [2] pasted text-2  (5,678 chars, 123 lines)      │
│                                                   │
│ Enter: View/Edit  d: Delete  Esc: Close          │
╰───────────────────────────────────────────────────╯
```

When viewing an attachment:
- Edit content directly
- Press **Ctrl+S** to save changes
- Press **Esc** to discard and return

## Edge Cases

### Paste Size Limits

- **≤200 chars**: Inserted directly into input (no attachment)
- **>200 chars, ≤5MB**: Saved as attachment
- **>5MB**: Rejected with error message

### Exactly 200 Characters

Pastes of exactly 200 characters are treated as inline (no attachment created).

### Terminal Resizing

Text automatically rewraps when you resize the terminal window. Wrapping recalculates within 100ms.

### Very Long Words

URLs or file paths exceeding terminal width are force-broken at the boundary:
```
> Check this URL: https://example.com/very/long/path/that/exceeds/termi
↩ nal/width/and/gets/broken/visually
```

The actual content remains intact (no characters lost), only the display is broken.

## Keyboard Shortcuts

| Key          | Action                              |
|--------------|-------------------------------------|
| **Ctrl+E**   | Open expanded editor                |
| **Esc**      | Exit expanded editor / attachment UI|
| **Ctrl+A**   | Open attachment list                |
| **Enter**    | Submit input / Select attachment    |
| **↑/↓**      | Navigate attachment list            |
| **d**        | Delete selected attachment          |
| **Ctrl+S**   | Save attachment edits               |

## Tips

1. **Use expanded editor for long messages**: Press Ctrl+E when composing multi-paragraph content
2. **Review large pastes before submitting**: Press Ctrl+A to check attachment content
3. **Clean up attachments**: Delete unwanted attachments with 'd' to keep input clean
4. **Terminal width matters**: For best wrapping, use at least 80 columns width

---

## Agent Context Update

Now updating CLAUDE.md with new technologies:
