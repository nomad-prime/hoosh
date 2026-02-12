# Codex TUI Text Input System: Reverse Engineering Document

## Overview

The Codex CLI implements a sophisticated multi-line text input system in the TUI (Terminal User Interface) bottom pane. This document details how cursor handling and line wrapping work together to create a responsive editor experience.

## Architecture

### Core Components

1. **TextArea** (`codex-rs/tui/src/bottom_pane/textarea.rs`)
   - Low-level text buffer and editing operations
   - Manages cursor position, text content, and text elements (styled regions)
   - Handles wrapping and rendering

2. **ChatComposer** (`codex-rs/tui/src/bottom_pane/chat_composer.rs`)
   - High-level editor state machine
   - Manages popups, history, and paste handling
   - Routes keyboard input to TextArea

3. **Wrapping Utilities** (`codex-rs/tui/src/wrapping.rs`)
   - `word_wrap_line()`: Wraps a single ratatui `Line` with proper style preservation
   - `word_wrap_lines()`: Wraps multiple lines with initial/subsequent indent support
   - Uses `textwrap` crate for word-based breaking

4. **RowBuilder** (`codex-rs/tui/src/live_wrap.rs`)
   - Incremental wrapping for streaming text
   - Maintains visual rows at fixed width

## Text Storage & Cursor Position

### TextArea State

```rust
struct TextArea {
    text: String,                          // Raw UTF-8 text buffer
    cursor_pos: usize,                     // Byte offset into text
    wrap_cache: RefCell<Option<WrapCache>>, // Cached wrapped line ranges
    preferred_col: Option<usize>,          // For vertical cursor movement
    elements: Vec<TextElement>,            // Styled regions (e.g., mentions)
    kill_buffer: String,                   // Clipboard for Ctrl+U/Ctrl+K
}

struct WrapCache {
    width: u16,                            // Terminal width used for wrap
    lines: Vec<Range<usize>>,              // Byte ranges for each wrapped line
}
```

**Key insight**: Cursor position is stored as a byte offset, not a column. This is critical for Unicode handling.

## Line Wrapping Algorithm

### Wrapping Process

1. **Trigger**: When terminal width changes or text is modified
2. **Cache**: Results are memoized in `WrapCache` keyed by width
3. **Algorithm**: Uses `textwrap::wrap()` with `FirstFit` algorithm
4. **Output**: Vector of `Range<usize>` representing byte ranges of wrapped lines

### Byte Range to Display Column Conversion

When rendering or computing cursor position:

```rust
// Example: cursor_pos_with_state()
let lines = self.wrapped_lines(area.width);
let i = Self::wrapped_line_index_by_start(&lines, self.cursor_pos)?;
let ls = &lines[i];
// Calculate display column using Unicode width
let col = self.text[ls.start..self.cursor_pos].width() as u16;
```

**Process**:
1. Find which wrapped line contains the cursor position
2. Extract text from line start to cursor position
3. Calculate visual width using `unicode_width` crate
4. Add area.x offset to get screen column

## Cursor Movement

### Movement Functions

#### Horizontal Movement
- `move_cursor_left()` / `move_cursor_right()`: Move by grapheme boundary
- Uses `unicode_segmentation::UnicodeSegmentation` for proper grapheme handling

#### Vertical Movement
- `move_cursor_up()` / `move_cursor_down()`: Move between wrapped lines
- Tries to preserve display column using `preferred_col`
- Falls back to end-of-line if column doesn't exist on target line

#### Line-Based Movement
- `move_cursor_to_beginning_of_line()`: Find previous `\n` or start
- `move_cursor_to_end_of_line()`: Find next `\n` or EOF
- Word movement: `beginning_of_previous_word()`, `end_of_next_word()`

### Column Preservation Logic

```rust
fn move_cursor_down(&mut self) {
    // Save current display column on first movement
    if self.preferred_col.is_none() {
        self.preferred_col = Some(self.current_display_col());
    }
    // Apply preferred column to next line
    let line_start = self.end_of_current_line() + 1;
    let line_end = self.end_of_line(line_start);
    self.move_to_display_col_on_line(line_start, line_end, preferred_col);
}
```

**Behavior**: When pressing ↑/↓ repeatedly, cursor attempts to stay in the same visual column even if lines have different widths.

## Unicode Handling

### Key Considerations

1. **Grapheme Boundaries**: Movement uses `unicode_segmentation` to avoid splitting combining characters
2. **Display Width**: Uses `unicode_width` for calculating visual columns (handles emoji, CJK at width 2)
3. **Byte Position Clamping**: Cursor is always positioned at valid UTF-8 boundaries
   - `clamp_pos_to_char_boundary()`: Adjust to nearest valid UTF-8 boundary
   - `clamp_pos_to_nearest_boundary()`: Also respects text element boundaries

### Example: Emoji Handling
```
Text:     "hello 😀 world"
Bytes:    0..5 | 6..10 | 11..16
Display:  5    | 2     | 5
```
- Emoji takes 4 bytes but displays as 2 columns
- Cursor can only be at bytes 0, 5, 6, 10, 11, 16 (not in middle of emoji)

## Rendering Pipeline

### High-Level Flow

```
ChatComposer::render()
  ↓
TextArea::render_ref_with_state()
  ↓
TextArea::render_lines()
  ↓ (for each wrapped line)
  1. buf.set_string() - Draw base text
  2. Overlay styled elements (cyan color for mentions/attachments)
  3. Track scroll position to keep cursor visible
```

### Cursor Visibility Guarantee

The `effective_scroll()` function ensures:
- If content fits in area: no scrolling
- If cursor above visible area: scroll up
- If cursor below visible area: scroll down
- Prefers showing cursor at line boundaries to minimize jitter

```rust
fn effective_scroll(
    &self,
    area_height: u16,
    lines: &[Range<usize>],
    current_scroll: u16,
) -> u16 {
    let cursor_line_idx = Self::wrapped_line_index_by_start(lines, self.cursor_pos)?;
    // Clamp scroll to keep cursor visible
    if cursor_line_idx < scroll {
        scroll = cursor_line_idx;
    } else if cursor_line_idx >= scroll + area_height {
        scroll = cursor_line_idx + 1 - area_height;
    }
    scroll
}
```

## Styled Text Elements

### Text Elements
```rust
struct TextElement {
    range: Range<usize>,  // Byte range in text
}
```

Used for:
- User mentions (`@user`)
- Attachment placeholders (`[Image #1]`)
- Slash commands

### Rendering
Elements are overlaid with cyan color on top of base text. When text is edited, element ranges are adjusted:
- `shift_elements()`: Shift ranges when inserting/deleting
- `expand_range_to_element_boundaries()`: Ensure edits respect element boundaries

## Keyboard Input Handling

### Input Chain

```
ChatComposer::handle_key_event()
  ↓
ActivePopup check (slash commands, file search, etc.)
  ↓
ChatComposer::handle_key_event_without_popup()
  ↓
PasteBurst detection (non-bracketed paste)
  ↓
TextArea::input()
  ↓
match KeyEvent {
    Backspace, Delete, Char, Left, Right, Up, Down, Home, End,
    Ctrl+A/E (home/end), Ctrl+B/F (left/right),
    Ctrl+P/N (up/down), Ctrl+U/K (kill line),
    Meta+B/F (word movement), Meta+Delete/Backspace (word delete)
}
```

### Key Bindings

**Cursor Movement**:
- Arrow keys: Single character/grapheme
- Ctrl+B/F or Ctrl+Left/Right: Word boundaries
- Home/Ctrl+A: Line start
- End/Ctrl+E: Line end
- Up/Down: Wrapped lines (preserving column)

**Editing**:
- Backspace/Ctrl+H: Delete backward
- Delete/Ctrl+D: Delete forward
- Ctrl+W: Delete word backward
- Meta+Delete: Delete word forward
- Ctrl+U: Kill to line start
- Ctrl+K: Kill to line end
- Ctrl+Y: Yank (paste from kill buffer)

**Insertion**:
- Regular chars inserted at cursor
- Ctrl+J/M or Enter: Insert newline (unless submitting)
- Shift+Enter: Always insert newline (with `enhanced_keys_supported`)

## Text Modification & Cursor Updates

### Insert Operation
```rust
pub fn insert_str(&mut self, text: &str) {
    let pos = self.cursor_pos;
    self.text.insert_str(pos, text);
    self.wrap_cache.replace(None);  // Invalidate wrap cache
    self.cursor_pos += text.len();   // Move cursor after insertion
    self.shift_elements(pos, 0, text.len());
    self.preferred_col = None;       // Clear preferred column
}
```

**Effects**:
1. Insert text at cursor position
2. Invalidate wrapped line cache
3. Move cursor to end of inserted text
4. Shift all text elements past insertion point
5. Reset vertical movement state

### Replace Operation
```rust
pub fn replace_range(&mut self, range: Range<usize>, text: &str) {
    let removed_len = range.end - range.start;
    let inserted_len = text.len();
    self.text.replace_range(range, text);
    self.wrap_cache.replace(None);
    
    // Cursor adjustment logic:
    self.cursor_pos = if self.cursor_pos < range.start {
        self.cursor_pos  // Before range, no change
    } else if self.cursor_pos <= range.end {
        range.start + inserted_len  // Inside range, move to end
    } else {
        self.cursor_pos + (inserted_len as isize - removed_len as isize) as usize
    };
}
```

## Paste Handling

### Non-Bracketed Paste Detection

On Windows/some terminals, pastes arrive as rapid `KeyCode::Char` events instead of bracketed paste. The `PasteBurst` state machine detects this:

1. **ASCII chars**: Buffer first char, wait for more (flicker suppression)
2. **Non-ASCII**: Pass through immediately (IME friendliness)
3. **Threshold**: If multiple chars arrive within ~100ms, treat as paste
4. **Result**: Entire burst inserted atomically via `handle_paste()`

### Paste Burst Flushing
- Called from UI tick when timer expires
- Returns `bool` indicating if text changed (to trigger redraw)
- Can be manually flushed via `flush_paste_burst_if_due()`

## Desired Height Calculation

### Formula
```rust
fn desired_height(&self, width: u16) -> u16 {
    let footer_height = calculate_footer_height();
    let textarea_height = self.textarea.desired_height(width);
    textarea_height + 2 + footer_height  // 2 = spacing
}
```

**TextArea Desired Height**:
```rust
pub fn desired_height(&self, width: u16) -> u16 {
    self.wrapped_lines(width).len() as u16
}
```

Returns the total number of wrapped lines needed to display all text.

## Performance Optimizations

### Wrap Cache
- Keyed by terminal width
- Invalidated on text changes or width changes
- Reused for all cursor/rendering operations during same frame
- Implementation: `RefCell<Option<WrapCache>>` allows interior mutability

### Preferred Column
- Cached on first vertical movement
- Reset when:
  - Text is modified
  - Cursor is explicitly set
  - Horizontal movement occurs

### Scroll Position
- Maintained by `TextAreaState`
- Updated each frame based on cursor visibility
- Prevents unnecessary recalculation

## Edge Cases & Special Handling

### Empty Lines
- Wrapping produces at least one empty range
- Cursor can exist on empty lines
- Preferred column applies even on empty lines

### Very Long Words
- `textwrap` breaks long words if `break_words: true`
- Prevents display overflow when word exceeds terminal width

### Element Boundaries
- Cursor never lands inside an element span
- Edits that cross element boundaries expand to full element boundaries
- Elements are shifted/updated on every text modification

### Multiple Newlines
- Each `\n` is treated as a line break
- Empty lines between newlines are displayed as empty wrapped lines
- Preferred column is reset on explicit newline insertion

## Integration with ComposerInput (Public API)

The public `ComposerInput` wrapper provides:

```rust
impl ComposerInput {
    pub fn desired_height(&self, width: u16) -> u16;
    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)>;
    pub fn render_ref(&self, area: Rect, buf: &mut Buffer);
    pub fn input(&mut self, key: KeyEvent) -> ComposerAction;
    pub fn handle_paste(&mut self, pasted: String) -> bool;
}
```

Used by other crates (e.g., `codex-cloud-tasks`) for embedded text input without the full bottom pane.

## Summary Table

| Aspect | Implementation | Details |
|--------|---|---|
| **Text Storage** | `String` (UTF-8) | Byte offset based cursor |
| **Wrapping** | `textwrap::wrap()` + cache | FirstFit algorithm, keyed by width |
| **Cursor Position** | Byte offset + display column | Clamped to grapheme boundaries |
| **Scrolling** | Automatic, cursor-following | Keeps cursor visible in viewport |
| **Unicode** | Full support | Grapheme boundaries, display width aware |
| **Styled Elements** | Cyan overlay | Preserved through wrapping/editing |
| **Paste Detection** | PasteBurst state machine | Detects non-bracketed pastes |
| **Performance** | Caching + lazy recalc | Invalidated on width/text changes |

