# Research: Input Field Refinement

**Branch**: `003-input-field-refinement` | **Date**: 2025-12-11

## tui-textarea Integration

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

## Paste Detection & Classification

**Decision**: Intercept crossterm paste events, classify by character count, route to inline or attachment.

**Rationale**:
- Crossterm emits `Event::Paste(String)` on clipboard paste (Cargo.toml:36)
- Character count via `.chars().count()` is accurate for multi-byte UTF-8
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
- Unicode/emoji: `.chars().count()` counts Unicode scalar values correctly

**Best Practices**:
- Size check before attachment creation (fail fast on >5MB)
- Atomic operation: attachment created + reference inserted together
- Error messages clear: "Paste rejected: exceeds 5MB limit"

**Alternatives Considered**:
- Heuristic detection (typing speed): Rejected - unreliable, false positives
- Line-based threshold: Rejected - clarification specified chars only (spec.md:12)

## Text Wrapping with Visual Indicators

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
- Wide characters (emoji): `unicode-width` returns 2 for full-width chars, accurate counting
- Terminal resize: Recalculate wrapping on resize event (SC-005 <100ms)

**Best Practices**:
- Cache wrapped result if text + width unchanged (optimization)
- Use visual indicator that's widely supported: ↩ (U+21A9) or ⤶ (U+2936)
- Wrapping respects word boundaries (don't split mid-word) per FR-019

**Alternatives Considered**:
- `textwrap` crate: Designed for static text, doesn't support live indicators
- Character-by-character wrapping: Rejected - ignores word boundaries
- No indicators: Rejected - spec requires visual distinction (FR-007)

## Expanded Editor Layout & Mode Switching

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

## Attachment Management Interface

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

## Attachment Storage Architecture

**Decision**: In-memory `Vec<TextAttachment>` in `AppState`, indexed by sequential IDs.

**Rationale**:
- Ephemeral lifecycle (cleared after submit) doesn't justify persistence
- Spec explicitly states session-scoped (spec.md:131)
- Simple index-based ID generation (1, 2, 3...)
- Fast access for view/edit/delete operations

**Implementation Notes**:
```rust
pub struct AppState {
    // Existing fields...
    pub input: TextArea<'static>,

    // NEW fields:
    pub attachments: Vec<TextAttachment>,
    pub next_attachment_id: usize,
    pub input_mode: InputMode,
    pub attachment_view: Option<AttachmentViewState>,
}

impl AppState {
    pub fn create_attachment(&mut self, content: String) -> Result<usize> {
        // Validate size
        if content.len() > 5_000_000 {
            anyhow::bail!("Paste rejected: exceeds 5MB limit");
        }

        let id = self.next_attachment_id;
        self.next_attachment_id += 1;

        let attachment = TextAttachment {
            id,
            content: content.clone(),
            size_chars: content.chars().count(),
            line_count: content.lines().count(),
            created_at: Instant::now(),
        };

        self.attachments.push(attachment);
        Ok(id)
    }

    pub fn delete_attachment(&mut self, id: usize) -> Result<()> {
        let index = self.attachments.iter()
            .position(|a| a.id == id)
            .ok_or_else(|| anyhow::anyhow!("Attachment not found"))?;
        self.attachments.remove(index);
        Ok(())
    }

    pub fn clear_attachments(&mut self) {
        self.attachments.clear();
        self.next_attachment_id = 1; // Reset ID counter
    }
}
```

**Best Practices**:
- ID never reused within session (monotonic counter)
- Deletion by value (ID match), not index (stable after delete)
- Clear all on submit (spec.md:119 FR-017)

**Alternatives Considered**:
- HashMap: Rejected - sequential IDs don't need hash lookup overhead
- File-based temp storage: Rejected - spec requires in-memory, adds I/O complexity
- Reference counting (Rc/Arc): Rejected - single-threaded TUI, no sharing needed
