# Quick Start Guide: Markdown Table Rendering

**Feature**: 001-markdown-table-rendering
**For**: Developers implementing or extending table rendering
**Est. Reading Time**: 10 minutes

## Overview

This guide helps you quickly understand and work with the markdown table rendering feature in hoosh CLI. After reading this, you'll know:
- Where the code lives
- How to add/modify table rendering logic
- How to test your changes
- Common patterns and gotchas

## 5-Minute Quick Start

### 1. Locate the Code

**Primary File**: `src/tui/markdown.rs`
- Contains `MarkdownRenderer` struct
- Table rendering logic at ~line 128-400 (after implementation)

**Supporting Files**:
- `src/tui/message_renderer.rs` - Integration point
- `src/tui/colors.rs` - Table color definitions (if needed)

### 2. Understand the Flow

```
Markdown String
    ↓
pulldown-cmark Parser (generates events)
    ↓
MarkdownRenderer::render() (processes events)
    ↓
TableBuilder (accumulates table data)
    ↓
render_table() (formats as ASCII table)
    ↓
Vec<Line<'static>> (styled terminal lines)
```

### 3. Key Data Structures

```rust
// Accumulates table during parsing
struct TableBuilder {
    headers: Vec<TableCell>,
    alignments: Vec<Alignment>,
    rows: Vec<Vec<TableCell>>,
}

// Represents a cell with styling
struct TableCell {
    spans: Vec<Span<'static>>,  // Styled content
    visual_width: usize,          // For alignment
}

// Column alignment
enum Alignment { Left, Center, Right }
```

### 4. Run a Quick Test

```bash
# Build the project
cargo build

# Run table-specific tests
cargo test table

# Run full markdown test suite
cargo test markdown

# Visual test (requires running hoosh)
echo "| A | B |\n|---|---|\n| 1 | 2 |" | cargo run
```

## Development Workflow

### Adding a New Feature

**Example**: Add support for custom border characters

**Step 1**: Add configuration to TableBuilder
```rust
struct TableBuilder {
    // ... existing fields ...
    border_style: BorderStyle,  // NEW
}

enum BorderStyle {
    Pipes,      // Default: |
    Double,     // ║
    Custom(char),
}
```

**Step 2**: Modify render_table() to use border_style
```rust
fn render_header_line(..., border_style: BorderStyle) -> Line<'static> {
    let border = match border_style {
        BorderStyle::Pipes => "|",
        BorderStyle::Double => "║",
        BorderStyle::Custom(c) => &c.to_string(),
    };
    // Use `border` instead of hardcoded "|"
}
```

**Step 3**: Add tests
```rust
#[test]
fn test_custom_border_characters() {
    let markdown = "| A |\n|---|\n| 1 |";
    let mut renderer = MarkdownRenderer::new();
    renderer.set_table_border_style(BorderStyle::Double);
    let output = renderer.render(markdown);
    assert!(output_contains_char(&output, '║'));
}
```

**Step 4**: Run tests and verify
```bash
cargo test test_custom_border_characters
```

### Debugging a Table Rendering Issue

**Symptom**: Tables not appearing in output

**Debug Steps**:

1. **Check if table events are fired**
```rust
// Add to MarkdownRenderer::render()
Tag::Table(_) => {
    eprintln!("DEBUG: Table started");  // Temporary debug
    self.current_table = Some(TableBuilder::new());
}
```

2. **Verify TableBuilder accumulation**
```rust
Event::End(Tag::Table) => {
    if let Some(builder) = self.current_table.take() {
        eprintln!("DEBUG: Headers: {}, Rows: {}",
                  builder.headers.len(), builder.rows.len());
        let lines = self.render_table(builder, terminal_width);
        self.lines.extend(lines);
    }
}
```

3. **Check output generation**
```rust
fn render_table(&self, builder: TableBuilder, width: usize) -> Vec<Line<'static>> {
    let lines = vec![/* ... */];
    eprintln!("DEBUG: Generated {} lines", lines.len());
    lines
}
```

4. **Visual inspection**
```bash
# Capture output to file
echo "| A |\n|---|\n| 1 |" | cargo run > output.txt
cat output.txt
```

### Common Modifications

#### Change Column Width Calculation

**File**: `src/tui/markdown.rs`, function `calculate_column_widths()`

**Current**:
```rust
fn calculate_column_widths(table: &TableBuilder, terminal_width: usize) -> ColumnWidths {
    let mut widths: Vec<usize> = (0..num_cols)
        .map(|col| max_content_width(col) + 2)  // +2 padding
        .collect();
    // ... truncation logic ...
}
```

**Modification** (e.g., minimum column width):
```rust
fn calculate_column_widths(table: &TableBuilder, terminal_width: usize) -> ColumnWidths {
    const MIN_COLUMN_WIDTH: usize = 10;  // NEW
    let mut widths: Vec<usize> = (0..num_cols)
        .map(|col| max_content_width(col).max(MIN_COLUMN_WIDTH) + 2)
        .collect();
    // ... rest unchanged ...
}
```

#### Add Color to Table Borders

**File**: `src/tui/colors.rs` (add new constant)

```rust
pub const MARKDOWN_TABLE_BORDER: Color = Color::DarkGray;
```

**File**: `src/tui/markdown.rs` (use color)

```rust
fn render_header_line(...) -> Line<'static> {
    let mut spans = vec![
        Span::styled("|", Style::default().fg(colors::MARKDOWN_TABLE_BORDER))
    ];
    // ... rest of spans ...
}
```

#### Adjust Truncation Behavior

**File**: `src/tui/markdown.rs`, function `apply_truncation()`

**Current**:
```rust
fn apply_truncation(widths: &mut Vec<usize>, truncated: &mut Vec<bool>,
                    terminal_width: usize, borders_width: usize) {
    // Proportional reduction from rightmost columns
}
```

**Modification** (e.g., truncate from center):
```rust
fn apply_truncation(widths: &mut Vec<usize>, truncated: &mut Vec<bool>,
                    terminal_width: usize, borders_width: usize) {
    // NEW: Prioritize first and last columns, truncate middle
    let priority = [0, widths.len() - 1];  // Keep edges
    // ... custom truncation logic ...
}
```

## Testing Guide

### Running Tests

```bash
# All markdown tests
cargo test markdown

# Specific test
cargo test test_render_simple_table

# With output
cargo test test_render_simple_table -- --nocapture

# Integration tests only
cargo test --test '*' markdown

# Benchmarks (requires nightly)
cargo +nightly bench
```

### Writing a New Test

**Pattern 1**: Unit test (in `src/tui/markdown.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_your_feature() {
        // Arrange
        let renderer = MarkdownRenderer::new();
        let markdown = "| A | B |\n|---|---|\n| 1 | 2 |";

        // Act
        let lines = renderer.render(markdown);

        // Assert
        assert!(!lines.is_empty());
        assert_eq!(lines.len(), 3);  // Header + separator + data

        // Check specific content
        let first_line = lines[0].to_string();
        assert!(first_line.contains('|'));
        assert!(first_line.contains('A'));
    }
}
```

**Pattern 2**: Integration test (in `tests/integration/markdown_rendering_test.rs`)

```rust
#[test]
fn test_table_in_full_pipeline() {
    use hoosh::tui::markdown::MarkdownRenderer;
    use hoosh::tui::message_renderer::MessageRenderer;

    let renderer = MarkdownRenderer::new();
    let markdown = "Regular text\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nMore text";

    let lines = renderer.render(markdown);

    // Verify table is present and surrounded by text
    let output: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    assert!(output.iter().any(|l| l.contains("Regular text")));
    assert!(output.iter().any(|l| l.contains('|')));
    assert!(output.iter().any(|l| l.contains("More text")));
}
```

**Pattern 3**: Visual regression test (using insta crate)

```rust
#[test]
fn test_table_visual_snapshot() {
    let renderer = MarkdownRenderer::new();
    let markdown = "| Feature | Status |\n|---------|--------|\n| Tables  | Done   |";

    let lines = renderer.render(markdown);
    let output = lines.iter().map(|l| l.to_string()).join("\n");

    insta::assert_snapshot!(output);
    // First run: creates snapshot file
    // Subsequent runs: compares against snapshot
}
```

### Test Data Generators

**Helper for creating test tables**:

```rust
fn generate_test_table(rows: usize, cols: usize) -> String {
    let header = (0..cols).map(|i| format!("Col{}", i)).collect::<Vec<_>>().join(" | ");
    let separator = (0..cols).map(|_| "---").collect::<Vec<_>>().join("|");
    let data_rows: Vec<String> = (0..rows)
        .map(|r| (0..cols).map(|c| format!("{}x{}", r, c)).collect::<Vec<_>>().join(" | "))
        .collect();

    format!("| {} |\n|{}|\n{}",
            header,
            separator,
            data_rows.iter().map(|r| format!("| {} |", r)).collect::<Vec<_>>().join("\n"))
}

// Usage
let table_markdown = generate_test_table(100, 10);  // 100 rows, 10 columns
```

## Performance Profiling

### Benchmarking

**File**: `benches/markdown_bench.rs` (create if doesn't exist)

```rust
#![feature(test)]
extern crate test;

use test::Bencher;
use hoosh::tui::markdown::MarkdownRenderer;

#[bench]
fn bench_small_table(b: &mut Bencher) {
    let markdown = generate_test_table(10, 5);
    let renderer = MarkdownRenderer::new();
    b.iter(|| renderer.render(&markdown));
}

#[bench]
fn bench_large_table(b: &mut Bencher) {
    let markdown = generate_test_table(100, 10);
    let renderer = MarkdownRenderer::new();
    b.iter(|| renderer.render(&markdown));
}
```

**Run**:
```bash
cargo +nightly bench
```

### Profiling with flamegraph

```bash
# Install
cargo install flamegraph

# Profile table rendering
cargo flamegraph --bin hoosh -- < test_table.md

# Opens flamegraph SVG in browser
```

### Identifying Bottlenecks

**Common hotspots**:
1. Unicode width calculation → Solution: Cache widths
2. String allocation → Solution: Pre-allocate with capacity
3. Span creation → Solution: Reuse spans where possible

**Profiling snippet**:
```rust
use std::time::Instant;

let start = Instant::now();
let lines = renderer.render(markdown);
let elapsed = start.elapsed();
eprintln!("Rendered in {:?}", elapsed);
```

## Troubleshooting

### Issue: Alignment is off

**Symptom**: Columns don't line up properly

**Diagnosis**:
1. Check if using `unicode-width` crate for width calculation
2. Verify ANSI codes excluded from width calculation
3. Test with emoji or CJK characters

**Fix**:
```rust
use unicode_width::UnicodeWidthStr;

let visual_width = cell_content.width();  // Correct
// NOT: cell_content.len()  // Wrong (byte count, not visual width)
```

### Issue: Terminal width not respected

**Symptom**: Tables overflow terminal

**Diagnosis**:
1. Check `terminal_width` parameter passed to `render_table()`
2. Verify truncation logic applied
3. Test with narrow terminal (`export COLUMNS=40; cargo run`)

**Debug**:
```rust
eprintln!("Terminal width: {}", terminal_width);
eprintln!("Table total width: {}", total_width);
eprintln!("Truncation applied: {:?}", truncated_columns);
```

### Issue: Pipe characters missing

**Symptom**: Table renders as unformatted text

**Diagnosis**:
1. Check if `Tag::Table` events handled (not empty)
2. Verify `render_table()` called on `End(Table)`
3. Ensure lines appended to output

**Fix**:
```rust
Event::End(Tag::Table) => {
    if let Some(builder) = self.current_table.take() {
        let lines = self.render_table(builder, terminal_width);
        self.lines.extend(lines);  // CRITICAL: Don't forget this
    }
}
```

### Issue: Performance too slow

**Symptom**: Rendering takes > 100ms for 100-row table

**Diagnosis**:
1. Run benchmark to confirm
2. Profile with flamegraph
3. Check for quadratic algorithms

**Optimizations**:
```rust
// BAD: Quadratic complexity
for row in &rows {
    for cell in row {
        for span in &cell.spans {
            // Nested loops = O(n^3)
        }
    }
}

// GOOD: Linear with pre-calculation
let column_widths = calculate_once(&table);  // O(n*m)
for row in &rows {  // O(n)
    render_row(row, &column_widths);  // O(m)
}
// Total: O(n*m)
```

## Architecture Deep Dive

### Event-Driven Parsing

pulldown-cmark generates events for markdown elements:

```rust
Event::Start(Tag::Table(_))       // Table begins
Event::Start(Tag::TableHead)      // Header section begins
Event::Start(Tag::TableRow)       // New row
Event::Start(Tag::TableCell)      // New cell
Event::Text(content)              // Cell content
Event::End(Tag::TableCell)        // Cell ends
Event::End(Tag::TableRow)         // Row ends
Event::End(Tag::TableHead)        // Header ends
Event::Start(Tag::TableRow)       // Data row begins
// ... more rows ...
Event::End(Tag::Table)            // Table ends → RENDER HERE
```

**Key Insight**: Must accumulate all table data before rendering (can't render line-by-line).

### State Machine

```rust
impl MarkdownRenderer {
    fn render(&self, markdown: &str) -> Vec<Line<'static>> {
        let parser = Parser::new(markdown);
        let mut lines = Vec::new();
        let mut current_table: Option<TableBuilder> = None;

        for event in parser {
            match event {
                Event::Start(Tag::Table(_)) => {
                    current_table = Some(TableBuilder::new());
                }
                Event::Text(text) if current_table.is_some() => {
                    // Add text to current cell
                }
                Event::End(Tag::Table) => {
                    if let Some(builder) = current_table.take() {
                        lines.extend(render_table(builder));
                    }
                }
                // ... other events ...
            }
        }

        lines
    }
}
```

**State Tracking**:
- `current_table: Option<TableBuilder>` - Active table (None when outside table)
- `in_header: bool` - Inside header section vs data rows
- `current_row: Vec<TableCell>` - Accumulating current row
- `current_cell: TableCell` - Accumulating current cell

### Width Calculation Strategy

**Goal**: Balance content visibility with terminal width constraints

**Algorithm**:
```
1. Calculate ideal width per column (max content width + padding)
2. Sum total width (include borders: num_cols + 1 pipes)
3. If total > terminal_width:
   a. Calculate available width (terminal_width - borders)
   b. Reduce columns proportionally from right to left
   c. Set minimum width (3 chars for "...")
   d. Mark truncated columns
4. Return ColumnWidths struct with widths and truncation flags
```

**Edge Cases**:
- Terminal < 40 chars → Show minimum 2 columns
- Single column table → Always show full column (no minimum)
- All columns equally wide → Reduce all proportionally
- Some columns narrow → Keep narrow, reduce wide ones first

## Best Practices

### DO

✅ Use `unicode-width` crate for all width calculations
✅ Test with emoji, CJK characters, and ANSI-styled text
✅ Pre-allocate vectors when size is known (`Vec::with_capacity()`)
✅ Cache calculated widths (don't recalculate per row)
✅ Handle malformed tables gracefully (best-effort rendering)
✅ Write tests for every new feature
✅ Profile performance changes with benchmarks
✅ Use meaningful variable names (not `w`, `h`, `t`)

### DON'T

❌ Use `.len()` for visual width (counts bytes, not characters)
❌ Panic on malformed input (graceful degradation)
❌ Assume ASCII-only content
❌ Modify existing markdown rendering code without tests
❌ Introduce dependencies without justification
❌ Implement features not in spec (avoid scope creep)
❌ Use mutable global state (violates constitution)

## Code Patterns

### Pattern: Safe Option Unwrapping

```rust
// GOOD: Safe with defaults
let width = column_widths.get(col_idx).copied().unwrap_or(10);

// GOOD: Early return pattern
let Some(table) = self.current_table.take() else {
    return;  // No table to render
};

// BAD: Unsafe unwrap
let width = column_widths[col_idx];  // Panics if col_idx out of bounds
```

### Pattern: Iterating with Context

```rust
// GOOD: Enumerate when index needed
for (i, cell) in row.iter().enumerate() {
    let width = column_widths[i];
    render_cell(cell, width);
}

// GOOD: Zip when pairing collections
for (cell, &width) in row.iter().zip(&column_widths.widths) {
    render_cell(cell, width);
}

// BAD: Manual indexing
for i in 0..row.len() {
    let cell = &row[i];
    let width = column_widths[i];
    // Error-prone, verbose
}
```

### Pattern: Builder Pattern for Complex Construction

```rust
// GOOD: Builder pattern
let table = TableBuilder::new()
    .with_capacity(estimated_rows, estimated_cols)
    .header(vec![cell("A"), cell("B")])
    .row(vec![cell("1"), cell("2")])
    .build();

// OKAY: Explicit construction (for simple cases)
let mut table = TableBuilder::new();
table.add_header_cell(cell("A"));
table.add_header_cell(cell("B"));
table.finalize_row();
```

## Quick Reference

### Key Files

| File | Purpose |
|------|---------|
| `src/tui/markdown.rs` | Main rendering logic |
| `src/tui/message_renderer.rs` | Integration with message system |
| `src/tui/colors.rs` | Color definitions |
| `src/tui/terminal/mod.rs` | Terminal abstraction |
| `tests/integration/markdown_rendering_test.rs` | Integration tests |
| `benches/markdown_bench.rs` | Performance benchmarks |

### Key Functions

| Function | Purpose |
|----------|---------|
| `MarkdownRenderer::render()` | Main entry point |
| `render_table()` | Converts TableBuilder to styled lines |
| `calculate_column_widths()` | Determines column widths with truncation |
| `apply_alignment()` | Aligns cell content (left/center/right) |
| `render_header_line()` | Formats header row |
| `render_separator_line()` | Formats dash separator |
| `render_data_line()` | Formats data row |

### Key Types

| Type | Purpose |
|------|---------|
| `TableBuilder` | Accumulates table during parsing |
| `TableCell` | Styled cell content |
| `Alignment` | Column alignment (Left/Center/Right) |
| `ColumnWidths` | Calculated widths with truncation flags |
| `Line<'static>` | ratatui styled line (output type) |
| `Span<'static>` | ratatui styled text span |

### Terminal Width Detection

```rust
// In MarkdownRenderer
let terminal_width = terminal::size()?.0 as usize;

// Or passed from MessageRenderer
let terminal_width = self.terminal_width;
```

### Common Width Values

- Standard terminal: 80 characters
- Wide terminal: 120-160 characters
- Narrow terminal: 40-60 characters
- Minimum viable: 40 characters (per spec FR-013)

## Next Steps

**After Reading This Guide**:

1. **Explore the code**: Read `src/tui/markdown.rs` focusing on table sections
2. **Run tests**: `cargo test markdown` to see existing test coverage
3. **Make a small change**: Add a color to table borders, run tests
4. **Read the spec**: `../spec.md` for requirements and edge cases
5. **Read research**: `../research.md` for design decisions and rationale

**For Implementation Tasks**:

Refer to `tasks.md` (generated by `/speckit.tasks` command) for step-by-step implementation plan.

## Getting Help

**Resources**:
- **Spec**: `specs/001-markdown-table-rendering/spec.md` - Requirements
- **Research**: `specs/001-markdown-table-rendering/research.md` - Design decisions
- **Data Model**: `specs/001-markdown-table-rendering/data-model.md` - Type definitions
- **Contract**: `specs/001-markdown-table-rendering/contracts/rendering-interface.md` - API contract

**Codebase References**:
- pulldown-cmark docs: https://docs.rs/pulldown-cmark/
- ratatui docs: https://docs.rs/ratatui/
- unicode-width docs: https://docs.rs/unicode-width/

**Constitution**: `.specify/memory/constitution.md` - Project principles and standards
