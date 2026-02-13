# Custom TextArea Implementation Guide (Like Codex)

**Complete guide for building your own TextArea widget from scratch, based on codex-rs implementation.**

Date: 2026-02-01
Source: codex-rs/tui/src/bottom_pane/textarea.rs
Approach: Build custom TextArea with integrated wrapping (no external crates)

## Table of Contents

1. [Why Build Your Own](#why-build-your-own)
2. [Architecture Overview](#architecture-overview)
3. [Dependencies](#dependencies)
4. [Core Data Structures](#core-data-structures)
5. [Complete Implementation](#complete-implementation)
6. [Feature Breakdown](#feature-breakdown)
7. [Integration & Usage](#integration--usage)
8. [Testing](#testing)

---

## Why Build Your Own

**Advantages over tui-textarea crate:**
- ✅ Full control over all features
- ✅ Integrated wrapping (no separate cache layer needed)
- ✅ Text elements support (placeholders, atomic regions)
- ✅ Custom keyboard shortcuts
- ✅ Exactly the behavior you want
- ✅ No dependencies on external textarea crates

**What codex's TextArea provides:**
- Multi-line text editing
- Automatic word-wrapping for display
- Emacs-style keyboard shortcuts
- Unicode-aware (grapheme clusters, emoji, CJK)
- Kill/yank buffer (cut/paste)
- Text elements (atomic, non-editable regions)
- Efficient rendering with scrolling
- Cursor position mapping for wrapped display

---

## Architecture Overview

```
┌────────────────────────────────────────────────────────────┐
│                        TextArea                             │
│                                                              │
│  Fields:                                                     │
│  - text: String              (unwrapped storage)             │
│  - cursor_pos: usize         (byte offset)                  │
│  - wrap_cache: WrapCache     (cached wrapped line ranges)   │
│  - preferred_col: Option     (for vertical movement)        │
│  - elements: Vec<TextElem>   (atomic regions)               │
│  - kill_buffer: String       (emacs-style kill ring)        │
│                                                              │
│  Methods:                                                    │
│  - insert_str()              - Insert text at cursor        │
│  - delete_backward/forward() - Delete characters            │
│  - move_cursor_*()           - Navigation                   │
│  - kill/yank                 - Cut/paste operations         │
│  - wrapped_lines()           - Get wrapped line ranges      │
│  - render()                  - Draw to screen               │
└────────────────────────────────────────────────────────────┘
```

**Key Design Decisions:**
1. **Single String storage**: All text in one `String`, cursor as byte offset
2. **Wrapping cache**: Cached `Vec<Range<usize>>` mapping wrapped lines to byte ranges
3. **Grapheme-aware**: Uses `unicode-segmentation` for correct cursor movement
4. **Zero-copy wrapping**: Store ranges, not duplicated strings

---

## Dependencies

```toml
[dependencies]
# TUI framework
ratatui = "0.29"
crossterm = "0.28"

# Text wrapping
textwrap = { version = "0.16", default-features = false }

# Unicode handling
unicode-width = "0.2"
unicode-segmentation = "1.12"

# Optional: for testing
[dev-dependencies]
pretty_assertions = "1.4"
rand = "0.8"
chrono = "0.4"
```

---

## Core Data Structures

### 1. TextArea Struct

```rust
// src/textarea.rs
use std::cell::{Ref, RefCell};
use std::ops::Range;
use textwrap::Options;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Word separator characters for word-wise navigation
const WORD_SEPARATORS: &str = "`~!@#$%^&*()-=+[{]}\\|;:'\",.<>/?";

fn is_word_separator(ch: char) -> bool {
    WORD_SEPARATORS.contains(ch)
}

/// Text element (atomic, non-editable region)
#[derive(Debug, Clone)]
struct TextElement {
    range: Range<usize>,
}

/// Wrapping cache
#[derive(Debug, Clone)]
struct WrapCache {
    width: u16,
    lines: Vec<Range<usize>>,
}

/// State for stateful rendering (tracks scroll position)
#[derive(Debug, Default, Clone, Copy)]
pub struct TextAreaState {
    /// Index into wrapped lines of the first visible line
    pub scroll: u16,
}

/// Main TextArea widget
#[derive(Debug)]
pub struct TextArea {
    /// The text content (unwrapped)
    text: String,

    /// Cursor position as byte offset into text
    cursor_pos: usize,

    /// Cached wrapped line ranges (RefCell for interior mutability)
    wrap_cache: RefCell<Option<WrapCache>>,

    /// Preferred column for vertical navigation
    preferred_col: Option<usize>,

    /// Text elements (atomic regions like placeholders)
    elements: Vec<TextElement>,

    /// Kill buffer for emacs-style cut/paste
    kill_buffer: String,
}
```

### 2. Constructor

```rust
impl TextArea {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor_pos: 0,
            wrap_cache: RefCell::new(None),
            preferred_col: None,
            elements: Vec::new(),
            kill_buffer: String::new(),
        }
    }
}
```

---

## Complete Implementation

### Part 1: Text Manipulation

```rust
impl TextArea {
    /// Get reference to text content
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Insert string at cursor position
    pub fn insert_str(&mut self, text: &str) {
        self.insert_str_at(self.cursor_pos, text);
    }

    /// Insert string at specific position
    pub fn insert_str_at(&mut self, pos: usize, text: &str) {
        let pos = self.clamp_pos_for_insertion(pos);
        self.text.insert_str(pos, text);
        self.wrap_cache.replace(None); // Invalidate cache

        // Move cursor if insertion was before it
        if pos <= self.cursor_pos {
            self.cursor_pos += text.len();
        }

        // Shift text elements after insertion point
        self.shift_elements(pos, 0, text.len());
        self.preferred_col = None;
    }

    /// Replace a range of text
    pub fn replace_range(&mut self, range: Range<usize>, text: &str) {
        let range = self.expand_range_to_element_boundaries(range);
        self.replace_range_raw(range, text);
    }

    fn replace_range_raw(&mut self, range: Range<usize>, text: &str) {
        assert!(range.start <= range.end);
        let start = range.start.clamp(0, self.text.len());
        let end = range.end.clamp(0, self.text.len());
        let removed_len = end - start;
        let inserted_len = text.len();

        if removed_len == 0 && inserted_len == 0 {
            return;
        }

        let diff = inserted_len as isize - removed_len as isize;

        self.text.replace_range(range, text);
        self.wrap_cache.replace(None);
        self.preferred_col = None;
        self.update_elements_after_replace(start, end, inserted_len);

        // Update cursor position
        self.cursor_pos = if self.cursor_pos < start {
            self.cursor_pos
        } else if self.cursor_pos <= end {
            start + inserted_len
        } else {
            ((self.cursor_pos as isize) + diff) as usize
        }
        .min(self.text.len());

        self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
    }

    /// Set text and clear elements
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.cursor_pos = self.cursor_pos.clamp(0, self.text.len());
        self.elements.clear();
        self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
        self.wrap_cache.replace(None);
        self.preferred_col = None;
        self.kill_buffer.clear();
    }
}
```

### Part 2: Cursor Management

```rust
impl TextArea {
    /// Get cursor position (byte offset)
    pub fn cursor(&self) -> usize {
        self.cursor_pos
    }

    /// Set cursor position
    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor_pos = pos.clamp(0, self.text.len());
        self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
        self.preferred_col = None;
    }

    /// Clamp position to nearest char boundary
    fn clamp_pos_to_char_boundary(&self, pos: usize) -> usize {
        let pos = pos.min(self.text.len());
        if self.text.is_char_boundary(pos) {
            return pos;
        }

        // Find nearest char boundary
        let mut prev = pos;
        while prev > 0 && !self.text.is_char_boundary(prev) {
            prev -= 1;
        }
        let mut next = pos;
        while next < self.text.len() && !self.text.is_char_boundary(next) {
            next += 1;
        }

        // Choose closer boundary
        if pos.saturating_sub(prev) <= next.saturating_sub(pos) {
            prev
        } else {
            next
        }
    }

    /// Clamp position to avoid being inside text elements
    fn clamp_pos_to_nearest_boundary(&self, pos: usize) -> usize {
        let pos = self.clamp_pos_to_char_boundary(pos);

        if let Some(idx) = self.find_element_containing(pos) {
            let e = &self.elements[idx];
            let dist_start = pos.saturating_sub(e.range.start);
            let dist_end = e.range.end.saturating_sub(pos);

            if dist_start <= dist_end {
                self.clamp_pos_to_char_boundary(e.range.start)
            } else {
                self.clamp_pos_to_char_boundary(e.range.end)
            }
        } else {
            pos
        }
    }

    /// Clamp position for insertion (can't insert inside elements)
    fn clamp_pos_for_insertion(&self, pos: usize) -> usize {
        let pos = self.clamp_pos_to_char_boundary(pos);

        if let Some(idx) = self.find_element_containing(pos) {
            let e = &self.elements[idx];
            let dist_start = pos.saturating_sub(e.range.start);
            let dist_end = e.range.end.saturating_sub(pos);

            if dist_start <= dist_end {
                self.clamp_pos_to_char_boundary(e.range.start)
            } else {
                self.clamp_pos_to_char_boundary(e.range.end)
            }
        } else {
            pos
        }
    }
}
```

### Part 3: Cursor Movement

```rust
impl TextArea {
    /// Move cursor left by one grapheme cluster
    pub fn move_cursor_left(&mut self) {
        self.cursor_pos = self.prev_atomic_boundary(self.cursor_pos);
        self.preferred_col = None;
    }

    /// Move cursor right by one grapheme cluster
    pub fn move_cursor_right(&mut self) {
        self.cursor_pos = self.next_atomic_boundary(self.cursor_pos);
        self.preferred_col = None;
    }

    /// Move cursor up (visual line)
    pub fn move_cursor_up(&mut self) {
        // Try using wrapped lines if cache exists
        if let Some((target_col, maybe_line)) = {
            let cache_ref = self.wrap_cache.borrow();
            if let Some(cache) = cache_ref.as_ref() {
                let lines = &cache.lines;
                if let Some(idx) = Self::wrapped_line_index_by_start(lines, self.cursor_pos) {
                    let cur_range = &lines[idx];
                    let target_col = self.preferred_col
                        .unwrap_or_else(|| self.text[cur_range.start..self.cursor_pos].width());

                    if idx > 0 {
                        let prev = &lines[idx - 1];
                        Some((target_col, Some((prev.start, prev.end.saturating_sub(1)))))
                    } else {
                        Some((target_col, None))
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } {
            match maybe_line {
                Some((line_start, line_end)) => {
                    if self.preferred_col.is_none() {
                        self.preferred_col = Some(target_col);
                    }
                    self.move_to_display_col_on_line(line_start, line_end, target_col);
                    return;
                }
                None => {
                    self.cursor_pos = 0;
                    self.preferred_col = None;
                    return;
                }
            }
        }

        // Fallback to logical line navigation
        if let Some(prev_nl) = self.text[..self.cursor_pos].rfind('\n') {
            let target_col = match self.preferred_col {
                Some(c) => c,
                None => {
                    let c = self.current_display_col();
                    self.preferred_col = Some(c);
                    c
                }
            };
            let prev_line_start = self.text[..prev_nl].rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let prev_line_end = prev_nl;
            self.move_to_display_col_on_line(prev_line_start, prev_line_end, target_col);
        } else {
            self.cursor_pos = 0;
            self.preferred_col = None;
        }
    }

    /// Move cursor down (visual line)
    pub fn move_cursor_down(&mut self) {
        // Similar to move_cursor_up but in opposite direction
        if let Some((target_col, move_to_last)) = {
            let cache_ref = self.wrap_cache.borrow();
            if let Some(cache) = cache_ref.as_ref() {
                let lines = &cache.lines;
                if let Some(idx) = Self::wrapped_line_index_by_start(lines, self.cursor_pos) {
                    let cur_range = &lines[idx];
                    let target_col = self.preferred_col
                        .unwrap_or_else(|| self.text[cur_range.start..self.cursor_pos].width());

                    if idx + 1 < lines.len() {
                        let next = &lines[idx + 1];
                        Some((target_col, Some((next.start, next.end.saturating_sub(1)))))
                    } else {
                        Some((target_col, None))
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } {
            match move_to_last {
                Some((line_start, line_end)) => {
                    if self.preferred_col.is_none() {
                        self.preferred_col = Some(target_col);
                    }
                    self.move_to_display_col_on_line(line_start, line_end, target_col);
                    return;
                }
                None => {
                    self.cursor_pos = self.text.len();
                    self.preferred_col = None;
                    return;
                }
            }
        }

        // Fallback to logical line navigation
        let target_col = match self.preferred_col {
            Some(c) => c,
            None => {
                let c = self.current_display_col();
                self.preferred_col = Some(c);
                c
            }
        };

        if let Some(next_nl) = self.text[self.cursor_pos..]
            .find('\n')
            .map(|i| i + self.cursor_pos)
        {
            let next_line_start = next_nl + 1;
            let next_line_end = self.text[next_line_start..]
                .find('\n')
                .map(|i| i + next_line_start)
                .unwrap_or(self.text.len());
            self.move_to_display_col_on_line(next_line_start, next_line_end, target_col);
        } else {
            self.cursor_pos = self.text.len();
            self.preferred_col = None;
        }
    }

    /// Move to specific column on a line
    fn move_to_display_col_on_line(&mut self, line_start: usize, line_end: usize, target_col: usize) {
        let mut width_so_far = 0usize;
        for (i, g) in self.text[line_start..line_end].grapheme_indices(true) {
            width_so_far += g.width();
            if width_so_far > target_col {
                self.cursor_pos = line_start + i;
                self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
                return;
            }
        }
        self.cursor_pos = line_end;
        self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
    }

    /// Move to beginning of line
    pub fn move_cursor_to_beginning_of_line(&mut self) {
        let bol = self.beginning_of_current_line();
        self.set_cursor(bol);
        self.preferred_col = None;
    }

    /// Move to end of line
    pub fn move_cursor_to_end_of_line(&mut self) {
        let eol = self.end_of_current_line();
        self.set_cursor(eol);
        self.preferred_col = None;
    }

    fn beginning_of_line(&self, pos: usize) -> usize {
        self.text[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0)
    }

    fn beginning_of_current_line(&self) -> usize {
        self.beginning_of_line(self.cursor_pos)
    }

    fn end_of_line(&self, pos: usize) -> usize {
        self.text[pos..]
            .find('\n')
            .map(|i| i + pos)
            .unwrap_or(self.text.len())
    }

    fn end_of_current_line(&self) -> usize {
        self.end_of_line(self.cursor_pos)
    }

    fn current_display_col(&self) -> usize {
        let bol = self.beginning_of_current_line();
        self.text[bol..self.cursor_pos].width()
    }

    /// Previous grapheme/element boundary
    fn prev_atomic_boundary(&self, pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }

        // Check if inside element
        if let Some(idx) = self.elements.iter()
            .position(|e| pos > e.range.start && pos <= e.range.end)
        {
            return self.elements[idx].range.start;
        }

        // Use grapheme cursor
        let mut gc = unicode_segmentation::GraphemeCursor::new(pos, self.text.len(), false);
        match gc.prev_boundary(&self.text, 0) {
            Ok(Some(b)) => {
                if let Some(idx) = self.find_element_containing(b) {
                    self.elements[idx].range.start
                } else {
                    b
                }
            }
            Ok(None) => 0,
            Err(_) => pos.saturating_sub(1),
        }
    }

    /// Next grapheme/element boundary
    fn next_atomic_boundary(&self, pos: usize) -> usize {
        if pos >= self.text.len() {
            return self.text.len();
        }

        // Check if inside element
        if let Some(idx) = self.elements.iter()
            .position(|e| pos >= e.range.start && pos < e.range.end)
        {
            return self.elements[idx].range.end;
        }

        // Use grapheme cursor
        let mut gc = unicode_segmentation::GraphemeCursor::new(pos, self.text.len(), false);
        match gc.next_boundary(&self.text, 0) {
            Ok(Some(b)) => {
                if let Some(idx) = self.find_element_containing(b) {
                    self.elements[idx].range.end
                } else {
                    b
                }
            }
            Ok(None) => self.text.len(),
            Err(_) => pos.saturating_add(1),
        }
    }
}
```

### Part 4: Delete Operations

```rust
impl TextArea {
    /// Delete N graphemes backward
    pub fn delete_backward(&mut self, n: usize) {
        if n == 0 || self.cursor_pos == 0 {
            return;
        }

        let mut target = self.cursor_pos;
        for _ in 0..n {
            target = self.prev_atomic_boundary(target);
            if target == 0 {
                break;
            }
        }
        self.replace_range(target..self.cursor_pos, "");
    }

    /// Delete N graphemes forward
    pub fn delete_forward(&mut self, n: usize) {
        if n == 0 || self.cursor_pos >= self.text.len() {
            return;
        }

        let mut target = self.cursor_pos;
        for _ in 0..n {
            target = self.next_atomic_boundary(target);
            if target >= self.text.len() {
                break;
            }
        }
        self.replace_range(self.cursor_pos..target, "");
    }

    /// Delete word backward
    pub fn delete_backward_word(&mut self) {
        let start = self.beginning_of_previous_word();
        self.kill_range(start..self.cursor_pos);
    }

    /// Delete word forward
    pub fn delete_forward_word(&mut self) {
        let end = self.end_of_next_word();
        if end > self.cursor_pos {
            self.kill_range(self.cursor_pos..end);
        }
    }

    /// Kill to end of line (Ctrl+K)
    pub fn kill_to_end_of_line(&mut self) {
        let eol = self.end_of_current_line();
        let range = if self.cursor_pos == eol {
            if eol < self.text.len() {
                Some(self.cursor_pos..eol + 1)
            } else {
                None
            }
        } else {
            Some(self.cursor_pos..eol)
        };

        if let Some(range) = range {
            self.kill_range(range);
        }
    }

    /// Kill to beginning of line (Ctrl+U)
    pub fn kill_to_beginning_of_line(&mut self) {
        let bol = self.beginning_of_current_line();
        let range = if self.cursor_pos == bol {
            if bol > 0 {
                Some(bol - 1..bol)
            } else {
                None
            }
        } else {
            Some(bol..self.cursor_pos)
        };

        if let Some(range) = range {
            self.kill_range(range);
        }
    }

    /// Yank (paste) from kill buffer (Ctrl+Y)
    pub fn yank(&mut self) {
        if self.kill_buffer.is_empty() {
            return;
        }
        let text = self.kill_buffer.clone();
        self.insert_str(&text);
    }

    fn kill_range(&mut self, range: Range<usize>) {
        let range = self.expand_range_to_element_boundaries(range);
        if range.start >= range.end {
            return;
        }

        let removed = self.text[range.clone()].to_string();
        if removed.is_empty() {
            return;
        }

        self.kill_buffer = removed;
        self.replace_range_raw(range, "");
    }

    /// Find beginning of previous word
    fn beginning_of_previous_word(&self) -> usize {
        let prefix = &self.text[..self.cursor_pos];
        let Some((first_non_ws_idx, ch)) = prefix
            .char_indices()
            .rev()
            .find(|&(_, ch)| !ch.is_whitespace())
        else {
            return 0;
        };

        let is_separator = is_word_separator(ch);
        let mut start = first_non_ws_idx;

        for (idx, ch) in prefix[..first_non_ws_idx].char_indices().rev() {
            if ch.is_whitespace() || is_word_separator(ch) != is_separator {
                start = idx + ch.len_utf8();
                break;
            }
            start = idx;
        }

        self.adjust_pos_out_of_elements(start, true)
    }

    /// Find end of next word
    fn end_of_next_word(&self) -> usize {
        let Some(first_non_ws) = self.text[self.cursor_pos..]
            .find(|c: char| !c.is_whitespace())
        else {
            return self.text.len();
        };

        let word_start = self.cursor_pos + first_non_ws;
        let mut iter = self.text[word_start..].char_indices();
        let Some((_, first_ch)) = iter.next() else {
            return word_start;
        };

        let is_separator = is_word_separator(first_ch);
        let mut end = self.text.len();

        for (idx, ch) in iter {
            if ch.is_whitespace() || is_word_separator(ch) != is_separator {
                end = word_start + idx;
                break;
            }
        }

        self.adjust_pos_out_of_elements(end, false)
    }

    fn adjust_pos_out_of_elements(&self, pos: usize, prefer_start: bool) -> usize {
        if let Some(idx) = self.find_element_containing(pos) {
            let e = &self.elements[idx];
            if prefer_start {
                e.range.start
            } else {
                e.range.end
            }
        } else {
            pos
        }
    }
}
```

### Part 5: Wrapping & Rendering

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};

impl TextArea {
    /// Get wrapped line ranges (cached)
    fn wrapped_lines(&self, width: u16) -> Ref<'_, Vec<Range<usize>>> {
        // Ensure cache is ready
        {
            let mut cache = self.wrap_cache.borrow_mut();
            let needs_recalc = match cache.as_ref() {
                Some(c) => c.width != width,
                None => true,
            };

            if needs_recalc {
                let lines = crate::wrapping::wrap_ranges(
                    &self.text,
                    Options::new(width as usize)
                        .wrap_algorithm(textwrap::WrapAlgorithm::FirstFit),
                );
                *cache = Some(WrapCache { width, lines });
            }
        }

        let cache = self.wrap_cache.borrow();
        Ref::map(cache, |c| &c.as_ref().unwrap().lines)
    }

    /// Find wrapped line index containing a byte position
    fn wrapped_line_index_by_start(lines: &[Range<usize>], pos: usize) -> Option<usize> {
        let idx = lines.partition_point(|r| r.start <= pos);
        if idx == 0 {
            None
        } else {
            Some(idx - 1)
        }
    }

    /// Calculate cursor position on screen
    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let lines = self.wrapped_lines(area.width);
        let i = Self::wrapped_line_index_by_start(&lines, self.cursor_pos)?;
        let ls = &lines[i];
        let col = self.text[ls.start..self.cursor_pos].width() as u16;
        Some((area.x + col, area.y + i as u16))
    }

    /// Calculate cursor position with scrolling
    pub fn cursor_pos_with_state(&self, area: Rect, state: TextAreaState) -> Option<(u16, u16)> {
        let lines = self.wrapped_lines(area.width);
        let effective_scroll = self.effective_scroll(area.height, &lines, state.scroll);
        let i = Self::wrapped_line_index_by_start(&lines, self.cursor_pos)?;
        let ls = &lines[i];
        let col = self.text[ls.start..self.cursor_pos].width() as u16;
        let screen_row = i.saturating_sub(effective_scroll as usize)
            .try_into()
            .unwrap_or(0);
        Some((area.x + col, area.y + screen_row))
    }

    /// Calculate effective scroll to keep cursor visible
    fn effective_scroll(&self, area_height: u16, lines: &[Range<usize>], current_scroll: u16) -> u16 {
        let total_lines = lines.len() as u16;
        if area_height >= total_lines {
            return 0;
        }

        let cursor_line_idx = Self::wrapped_line_index_by_start(lines, self.cursor_pos)
            .unwrap_or(0) as u16;

        let max_scroll = total_lines.saturating_sub(area_height);
        let mut scroll = current_scroll.min(max_scroll);

        // Ensure cursor is visible
        if cursor_line_idx < scroll {
            scroll = cursor_line_idx;
        } else if cursor_line_idx >= scroll + area_height {
            scroll = cursor_line_idx + 1 - area_height;
        }
        scroll
    }

    /// Get desired height for rendering all content
    pub fn desired_height(&self, width: u16) -> u16 {
        self.wrapped_lines(width).len() as u16
    }

    /// Render to buffer (stateless)
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let lines = self.wrapped_lines(area.width);
        self.render_lines(area, buf, &lines, 0..lines.len());
    }

    /// Render with state (scrolling)
    pub fn render_with_state(&self, area: Rect, buf: &mut Buffer, state: &mut TextAreaState) {
        let lines = self.wrapped_lines(area.width);
        let scroll = self.effective_scroll(area.height, &lines, state.scroll);
        state.scroll = scroll;

        let start = scroll as usize;
        let end = (scroll + area.height).min(lines.len() as u16) as usize;
        self.render_lines(area, buf, &lines, start..end);
    }

    fn render_lines(
        &self,
        area: Rect,
        buf: &mut Buffer,
        lines: &[Range<usize>],
        range: Range<usize>,
    ) {
        for (row, idx) in range.enumerate() {
            let r = &lines[idx];
            let y = area.y + row as u16;
            let line_range = r.start..r.end.saturating_sub(1);

            // Draw text
            buf.set_string(area.x, y, &self.text[line_range.clone()], Style::default());

            // Overlay styled segments for elements
            for elem in &self.elements {
                let overlap_start = elem.range.start.max(line_range.start);
                let overlap_end = elem.range.end.min(line_range.end);
                if overlap_start >= overlap_end {
                    continue;
                }

                let styled = &self.text[overlap_start..overlap_end];
                let x_off = self.text[line_range.start..overlap_start].width() as u16;
                let style = Style::default().fg(Color::Cyan);
                buf.set_string(area.x + x_off, y, styled, style);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}
```

### Part 6: Text Elements Support (Optional)

```rust
impl TextArea {
    /// Insert a text element (atomic, non-editable region)
    pub fn insert_element(&mut self, text: &str) {
        let start = self.clamp_pos_for_insertion(self.cursor_pos);
        self.insert_str_at(start, text);
        let end = start + text.len();
        self.add_element(start..end);
        self.set_cursor(end);
    }

    fn add_element(&mut self, range: Range<usize>) {
        let elem = TextElement { range };
        self.elements.push(elem);
        self.elements.sort_by_key(|e| e.range.start);
    }

    fn find_element_containing(&self, pos: usize) -> Option<usize> {
        self.elements
            .iter()
            .position(|e| pos > e.range.start && pos < e.range.end)
    }

    fn expand_range_to_element_boundaries(&self, mut range: Range<usize>) -> Range<usize> {
        // Expand to include any intersecting elements fully
        loop {
            let mut changed = false;
            for e in &self.elements {
                if e.range.start < range.end && e.range.end > range.start {
                    let new_start = range.start.min(e.range.start);
                    let new_end = range.end.max(e.range.end);
                    if new_start != range.start || new_end != range.end {
                        range.start = new_start;
                        range.end = new_end;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        range
    }

    fn shift_elements(&mut self, at: usize, removed: usize, inserted: usize) {
        let end = at + removed;
        let diff = inserted as isize - removed as isize;

        // Remove elements fully deleted
        self.elements.retain(|e| !(e.range.start >= at && e.range.end <= end));

        // Shift remaining elements
        for e in &mut self.elements {
            if e.range.end <= at {
                // Before edit
            } else if e.range.start >= end {
                // After edit - shift
                e.range.start = ((e.range.start as isize) + diff) as usize;
                e.range.end = ((e.range.end as isize) + diff) as usize;
            } else {
                // Overlap - snap to new bounds
                let new_start = at.min(e.range.start);
                let new_end = at + inserted.max(e.range.end.saturating_sub(end));
                e.range.start = new_start;
                e.range.end = new_end;
            }
        }
    }

    fn update_elements_after_replace(&mut self, start: usize, end: usize, inserted_len: usize) {
        self.shift_elements(start, end.saturating_sub(start), inserted_len);
    }
}
```

### Part 7: Keyboard Input Handling

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl TextArea {
    /// Handle keyboard input
    pub fn input(&mut self, event: KeyEvent) {
        match event {
            // Plain character input
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                ..
            } => self.insert_str(&c.to_string()),

            // Enter key
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => self.insert_str("\n"),

            // Backspace
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.delete_backward(1),

            // Delete
            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.delete_forward(1),

            // Alt+Backspace - delete word backward
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::ALT,
                ..
            } => self.delete_backward_word(),

            // Alt+Delete - delete word forward
            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::ALT,
                ..
            } => self.delete_forward_word(),

            // Ctrl+K - kill to end of line
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.kill_to_end_of_line(),

            // Ctrl+U - kill to beginning of line
            KeyEvent {
                code: KeyCode::Char('u'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.kill_to_beginning_of_line(),

            // Ctrl+Y - yank (paste)
            KeyEvent {
                code: KeyCode::Char('y'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.yank(),

            // Arrow keys
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_cursor_left(),

            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_cursor_right(),

            KeyEvent {
                code: KeyCode::Up,
                ..
            } => self.move_cursor_up(),

            KeyEvent {
                code: KeyCode::Down,
                ..
            } => self.move_cursor_down(),

            // Home/End
            KeyEvent {
                code: KeyCode::Home,
                ..
            } => self.move_cursor_to_beginning_of_line(),

            KeyEvent {
                code: KeyCode::End,
                ..
            } => self.move_cursor_to_end_of_line(),

            // Ctrl+A - beginning of line
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.move_cursor_to_beginning_of_line(),

            // Ctrl+E - end of line
            KeyEvent {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.move_cursor_to_end_of_line(),

            // Alt+B - previous word
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::ALT,
                ..
            } => {
                let pos = self.beginning_of_previous_word();
                self.set_cursor(pos);
            }

            // Alt+F - next word
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::ALT,
                ..
            } => {
                let pos = self.end_of_next_word();
                self.set_cursor(pos);
            }

            _ => {
                // Unhandled key event
            }
        }
    }
}
```

---

## Feature Breakdown

### Unicode Support

**How it works:**
- Uses `unicode-segmentation` for grapheme cluster boundaries
- Uses `unicode-width` for display width calculations
- Handles:
  - Emoji (👍, 🚀, etc.)
  - CJK characters (漢字, 你好)
  - Combining marks (é = e + ́)
  - Zero-width joiners (👩‍💻)

**Example:**
```rust
let mut ta = TextArea::new();
ta.insert_str("Hello 世界 👍");
ta.move_cursor_left(); // Moves by one grapheme (👍)
```

### Wrapping Integration

**How it works:**
- Text stored unwrapped in `text: String`
- Wrapping calculated on-demand via `wrapped_lines(width)`
- Results cached in `RefCell<Option<WrapCache>>`
- Cache invalidated on text changes
- Returns `Vec<Range<usize>>` (byte ranges into original text)

**Key methods:**
```rust
// Get wrapped line ranges
let lines = textarea.wrapped_lines(80); // width = 80

// Find which line contains cursor
let idx = TextArea::wrapped_line_index_by_start(&lines, cursor_pos);

// Calculate screen position
let (x, y) = textarea.cursor_pos(area);
```

### Preferred Column Tracking

**Why needed:**
When moving up/down, you want to stay in the same visual column even if lines have different lengths.

**How it works:**
```rust
// User on line "short", column 5
// Moves up to line "ab" (length 2)
// Cursor goes to end of "ab"
// preferred_col remembers "5"
// Moving down again tries to return to column 5

self.preferred_col = Some(5);
```

**Reset on:**
- Horizontal movement (left/right)
- Insertion/deletion
- Explicit cursor set

### Kill/Yank Buffer

**Emacs-style cut/paste:**
- `Ctrl+K` - Kill to end of line → stores in kill_buffer
- `Ctrl+U` - Kill to beginning of line → stores in kill_buffer
- `Alt+Backspace` - Delete word backward → stores in kill_buffer
- `Ctrl+Y` - Yank (paste) from kill_buffer

**Implementation:**
```rust
fn kill_range(&mut self, range: Range<usize>) {
    let removed = self.text[range.clone()].to_string();
    self.kill_buffer = removed; // Save for yank
    self.replace_range_raw(range, "");
}

pub fn yank(&mut self) {
    let text = self.kill_buffer.clone();
    self.insert_str(&text);
}
```

### Text Elements (Atomic Regions)

**Use case:** Placeholders that can't be partially edited

**Example:**
```rust
ta.insert_str("Hello ");
ta.insert_element("[File: foo.txt]"); // Atomic
ta.insert_str(" world");

// Cursor can't be placed inside "[File: foo.txt]"
// Deleting partially deletes the whole element
```

**How it works:**
- Store `Vec<TextElement>` with byte ranges
- Cursor clamped to element boundaries
- Delete operations expanded to include full elements
- Elements shift when text before them changes

---

## Integration & Usage

### Basic Usage

```rust
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::Rect,
    Terminal,
};
use std::io;

mod textarea;
mod wrapping;

use textarea::{TextArea, TextAreaState};

fn main() -> io::Result<()> {
    // Setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create textarea
    let mut textarea = TextArea::new();
    textarea.insert_str("Type here...\nText wraps automatically!");
    let mut state = TextAreaState::default();

    // Main loop
    loop {
        terminal.draw(|f| {
            let area = f.area();
            let mut buf = f.buffer_mut();

            // Render textarea
            textarea.render_with_state(area, buf, &mut state);

            // Position cursor
            if let Some((x, y)) = textarea.cursor_pos_with_state(area, state) {
                f.set_cursor_position((x, y));
            }
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Esc {
                    break;
                }
                textarea.input(key);
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
```

### Advanced: Multi-line Input with Block

```rust
use ratatui::widgets::{Block, Borders};

// In your draw function:
terminal.draw(|f| {
    let area = f.area();

    // Create block
    let block = Block::default()
        .title("Input")
        .borders(Borders::ALL);

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Render textarea inside block
    let mut buf = f.buffer_mut();
    textarea.render_with_state(inner, buf, &mut state);

    // Position cursor
    if let Some((x, y)) = textarea.cursor_pos_with_state(inner, state) {
        f.set_cursor_position((x, y));
    }
})?;
```

### With Scrolling Indicator

```rust
terminal.draw(|f| {
    let area = f.area();
    let mut buf = f.buffer_mut();

    textarea.render_with_state(area, buf, &mut state);

    // Show scroll indicator
    let total_lines = textarea.desired_height(area.width);
    if total_lines > area.height {
        let scroll_pct = (state.scroll * 100) / total_lines.saturating_sub(area.height);
        let indicator = format!(" {}% ", scroll_pct);
        buf.set_string(
            area.x + area.width - indicator.len() as u16,
            area.y,
            &indicator,
            Style::default().bg(Color::DarkGray),
        );
    }
})?;
```

---

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_cursor() {
        let mut ta = TextArea::new();
        ta.insert_str("hello");
        assert_eq!(ta.text(), "hello");
        assert_eq!(ta.cursor(), 5);
    }

    #[test]
    fn test_delete_backward() {
        let mut ta = TextArea::new();
        ta.insert_str("abc");
        ta.delete_backward(1);
        assert_eq!(ta.text(), "ab");
        assert_eq!(ta.cursor(), 2);
    }

    #[test]
    fn test_wrapping() {
        let mut ta = TextArea::new();
        ta.insert_str("hello world here");
        let area = Rect::new(0, 0, 6, 10);
        assert!(ta.desired_height(area.width) >= 3);
    }

    #[test]
    fn test_unicode() {
        let mut ta = TextArea::new();
        ta.insert_str("a👍b");
        ta.set_cursor(ta.text().len());
        ta.move_cursor_left(); // Should skip over entire emoji
        assert!(ta.cursor() < "a👍b".len());
        assert!(ta.cursor() > "a".len());
    }

    #[test]
    fn test_kill_yank() {
        let mut ta = TextArea::new();
        ta.insert_str("hello world");
        ta.set_cursor(6); // After "hello "
        ta.kill_to_end_of_line();
        assert_eq!(ta.text(), "hello ");

        ta.yank();
        assert_eq!(ta.text(), "hello world");
    }
}
```

---

## Summary

You now have a complete, production-ready TextArea implementation with:

✅ **Multi-line editing** - Full text editing capabilities
✅ **Integrated wrapping** - No external cache needed
✅ **Unicode support** - Handles emoji, CJK, combining marks
✅ **Emacs shortcuts** - Kill/yank, word navigation
✅ **Text elements** - Atomic, non-editable regions
✅ **Efficient rendering** - Cached wrapping, scrolling support
✅ **Full control** - Customize everything

**File structure:**
```
src/
├── wrapping.rs       # wrap_ranges() from previous guide
├── textarea.rs       # All code from this guide
└── main.rs          # Your app using TextArea
```

**Next steps:**
1. Create `wrapping.rs` (copy from TEXTAREA_WRAPPING_IMPLEMENTATION_GUIDE.md)
2. Create `textarea.rs` (copy all code sections from this guide)
3. Add keyboard shortcuts you need
4. Customize rendering (colors, styling)
5. Test with your use cases

This gives you the same powerful TextArea that codex uses, with full control over its behavior!
