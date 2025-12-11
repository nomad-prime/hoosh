# Rendering Interface Contract: Markdown Table Rendering

**Feature**: 001-markdown-table-rendering
**Type**: Internal Rust API Contract
**Date**: 2025-12-11

## Overview

This contract defines the public interface for markdown table rendering within the hoosh CLI. Since this is an internal rendering feature (not a REST/GraphQL API), this document specifies Rust function signatures, behavior contracts, and integration points.

## Public API

### MarkdownRenderer::render()

**Existing Method** (extended behavior):

```rust
impl MarkdownRenderer {
    /// Renders markdown text to styled terminal lines
    ///
    /// # Arguments
    /// * `markdown` - Raw markdown text potentially containing tables
    ///
    /// # Returns
    /// Vector of styled lines ready for terminal display
    ///
    /// # Behavior Changes (This Feature)
    /// - Tables are now rendered as structured ASCII tables
    /// - Pipe characters (`|`) are preserved and visible
    /// - Table structure maintains alignment and formatting
    ///
    /// # Examples
    /// ```rust
    /// let renderer = MarkdownRenderer::new();
    /// let markdown = "| A | B |\n|---|---|\n| 1 | 2 |";
    /// let lines = renderer.render(markdown);
    /// // lines now contains formatted table output
    /// ```
    pub fn render(&self, markdown: &str) -> Vec<Line<'static>>
}
```

**Contract Guarantees**:
- All table tags in markdown are processed (no longer ignored)
- Pipe characters (`|`) appear in output
- Table structure is preserved with visible borders
- Terminal width is respected (truncation applied if needed)
- Returns same type (`Vec<Line<'static>>`) as before (backward compatible)

**Error Handling**:
- No errors thrown for malformed tables (best-effort rendering)
- Empty tables render as empty vec (no output)
- Invalid markdown handled gracefully by pulldown-cmark

### MarkdownRenderer::render_with_indent()

**Existing Method** (extended behavior):

```rust
impl MarkdownRenderer {
    /// Renders markdown with custom indentation prefix
    ///
    /// # Arguments
    /// * `markdown` - Raw markdown text
    /// * `indent` - String to prepend to each line
    ///
    /// # Returns
    /// Vector of styled lines with indentation applied
    ///
    /// # Behavior Changes (This Feature)
    /// - Tables rendered with indent applied to each table line
    /// - Table structure remains intact with indentation
    ///
    /// # Examples
    /// ```rust
    /// let lines = renderer.render_with_indent(markdown, "  ");
    /// // All table lines prefixed with "  "
    /// ```
    pub fn render_with_indent(&self, markdown: &str, indent: &str) -> Vec<Line<'static>>
}
```

**Contract Guarantees**:
- Indent applied consistently to all table lines
- Table alignment calculated after indent width subtracted from terminal width

## Internal API (New Components)

### TableBuilder

**Purpose**: Accumulates table structure during parsing

```rust
struct TableBuilder {
    headers: Vec<TableCell>,
    alignments: Vec<Alignment>,
    rows: Vec<Vec<TableCell>>,
    in_header: bool,
    current_row: Vec<TableCell>,
    current_cell: TableCell,
}

impl TableBuilder {
    /// Creates new empty table builder
    fn new() -> Self;

    /// Adds cell to current row
    fn add_cell(&mut self, cell: TableCell);

    /// Finalizes current row (moves to headers or rows)
    fn finalize_row(&mut self);

    /// Sets column alignments from separator row
    fn set_alignments(&mut self, alignments: Vec<Alignment>);
}
```

**Invariants**:
- `headers.len()` determines column count
- `rows[i].len()` may be less than `headers.len()` (padded during rendering)
- `alignments.len()` equals `headers.len()` or defaults applied

### TableCell

**Purpose**: Represents styled cell content

```rust
struct TableCell {
    spans: Vec<Span<'static>>,
    visual_width: usize,
}

impl TableCell {
    /// Creates empty cell
    fn new() -> Self;

    /// Adds styled span to cell
    fn add_span(&mut self, span: Span<'static>);

    /// Checks if cell has no content
    fn is_empty(&self) -> bool;
}
```

**Invariants**:
- `visual_width` equals sum of span visual widths (unicode-aware)
- Empty cell has `visual_width == 0` and empty `spans`

### Alignment

**Purpose**: Specifies column alignment

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Alignment {
    Left,
    Center,
    Right,
}

impl Alignment {
    /// Parses alignment from separator column (e.g., ":--", ":-:", "--:")
    fn from_separator(sep: &str) -> Self;
}
```

**Parsing Rules**:
- `---` or `:--` → Left (default)
- `:-:` → Center
- `--:` → Right

## Integration Points

### MessageRenderer Integration

**File**: `src/tui/message_renderer.rs`

**Integration Method**: `render_markdown_message()`

```rust
impl MessageRenderer {
    fn render_markdown_message(&mut self, markdown: &str) -> Vec<Line<'static>> {
        // Existing code path - NO CHANGES NEEDED
        let lines = self.markdown_renderer.render(markdown);
        self.wrap_styled_lines(lines, self.terminal_width)
    }
}
```

**Contract**:
- No API changes required in MessageRenderer
- Table rendering transparent to message rendering pipeline
- Text wrapping applied after table rendering (table lines treated as regular styled lines)

**Behavior Change**:
- Tables now appear in message content (previously missing)
- No change to message structure or event handling

### AppState Integration

**File**: `src/tui/app_state.rs`

**No changes required**. Tables rendered through existing markdown pipeline:

```rust
// Existing code continues to work
app_state.add_final_response(markdown_content);
// → MessageLine::Markdown stored
// → MessageRenderer processes via render_markdown_message()
// → MarkdownRenderer now renders tables correctly
```

## Behavioral Contracts

### FR-001: Pipe Character Preservation

**Contract**: All pipe characters (`|`) in table structure must appear in output

**Input**:
```markdown
| A | B |
|---|---|
| 1 | 2 |
```

**Output** (conceptual):
```
| A | B |
|---|---|
| 1 | 2 |
```

**Verification**: Test counts pipe characters in output, must equal expected count

### FR-004: Header Separator

**Contract**: Headers visually distinguished by dash separator line

**Input**:
```markdown
| Header |
|--------|
| Data   |
```

**Output**:
```
| Header |
|--------|
| Data   |
```

**Verification**: Line following header row contains only dashes and pipes

### FR-006: Alignment Support

**Contract**: Column alignment specs honored in output

**Input**:
```markdown
| Left | Center | Right |
|:-----|:------:|------:|
| A    | B      | C     |
```

**Output** (spacing indicates alignment):
```
| Left   | Center  |  Right |
|--------|---------|--------|
| A      |    B    |      C |
```

**Verification**: Visual inspection or spacing calculation confirms alignment

### FR-009: Terminal Width Truncation

**Contract**: Tables wider than terminal show ellipsis

**Input**: 10-column table, terminal width = 60

**Output**: First ~7 columns visible, others truncated with "..."

**Verification**: Total output width ≤ terminal width, ellipsis present

### FR-011: Missing Cell Padding

**Contract**: Rows with fewer cells than headers are padded with empty cells

**Input**:
```markdown
| A | B | C |
|---|---|---|
| 1 | 2 |
```

**Output**:
```
| A | B | C |
|---|---|---|
| 1 | 2 |   |
```

**Verification**: All rows have same number of cells in output

### FR-012: Escaped Pipe Handling

**Contract**: `\|` in cell content renders as literal `|` (not column separator)

**Input**:
```markdown
| Code Example |
|--------------|
| a \| b       |
```

**Output**:
```
| Code Example |
|--------------|
| a | b        |
```

**Verification**: Cell contains single entry with pipe inside, not split

## Performance Contracts

### SC-006: Rendering Performance

**Contract**: Tables up to 100 rows render in < 100ms

**Measurement**: Benchmark test with `cargo bench`

```rust
#[bench]
fn bench_large_table(b: &mut Bencher) {
    let markdown = generate_table(100, 10);  // 100 rows, 10 cols
    b.iter(|| renderer.render(&markdown));
}
// Assert: avg < 100ms
```

**Verification**: CI pipeline runs benchmarks, fails if threshold exceeded

## Compatibility Guarantees

### Backward Compatibility

**Guarantee**: Existing markdown rendering continues to work identically

**Non-table content**: No behavior changes
- Headings, lists, code blocks, etc. render exactly as before
- Performance unchanged for non-table content

**API Compatibility**:
- Public method signatures unchanged
- Return types unchanged (`Vec<Line<'static>>`)
- No new required dependencies for consumers

### Forward Compatibility

**Versioning**: No API version required (internal component)

**Future Extensions** (not breaking):
- Additional alignment options (e.g., justify)
- Custom border styles
- Column width hints

## Error Handling Contract

### No Exceptions Thrown

**Contract**: Table rendering never panics or returns errors

**Malformed Input Handling**:
- Missing cells → Padded with empty (graceful degradation)
- Missing separator → Default left alignment
- Zero columns → Empty output (no table)
- Extremely wide tables → Truncated (doesn't panic)

**Error Signaling**: None. Best-effort rendering always succeeds.

## Testing Contract

### Required Test Coverage

**Unit Tests** (in `src/tui/markdown.rs`):
- `test_render_simple_table()` - Basic 2x2 table
- `test_render_with_alignment()` - Left/center/right alignment
- `test_truncate_wide_table()` - Terminal width handling
- `test_escaped_pipes()` - `\|` handling
- `test_missing_cells()` - Cell padding
- `test_empty_cells()` - Empty cell rendering
- `test_formatted_cells()` - Bold/italic within cells

**Integration Tests** (in `tests/integration/`):
- `test_table_in_message()` - Full pipeline test
- `test_multiple_tables()` - Multiple tables in single markdown
- `test_table_with_other_elements()` - Tables mixed with lists, code, etc.

**Performance Tests** (in `benches/`):
- `bench_large_table()` - 100 row table rendering
- `bench_wide_table()` - Many column table

### Acceptance Criteria

Per spec Success Criteria:
- **SC-001**: 100% pipe character visibility (test: count pipes)
- **SC-002**: 10x50 tables render correctly (test: integration test)
- **SC-003**: Readable output (test: manual/visual regression)
- **SC-004**: Cross-platform (test: CI on Linux/macOS/Windows)
- **SC-005**: Alignment works (test: unit tests per alignment)
- **SC-006**: <100ms for 100 rows (test: benchmark)

## Dependencies

### New Dependency

**unicode-width 0.1**:
- Purpose: Accurate visual width calculation for unicode characters
- Impact: Required for correct alignment with emoji, CJK characters
- License: MIT/Apache-2.0 (compatible)
- Size: ~14KB (minimal)

### Existing Dependencies (No Changes)

- pulldown-cmark 0.11 - Markdown parsing
- ratatui 0.29 - Terminal UI primitives
- textwrap - Line wrapping (used post-table-rendering)

## Migration Guide

### For Existing Code

**No migration needed**. Tables automatically work:

**Before** (tables ignored):
```rust
let renderer = MarkdownRenderer::new();
let output = renderer.render(markdown_with_table);
// Tables missing from output
```

**After** (tables rendered):
```rust
let renderer = MarkdownRenderer::new();
let output = renderer.render(markdown_with_table);
// Tables present in output, correctly formatted
```

### For New Code

**Using tables in markdown**:
```rust
let markdown = r#"
| Feature | Status |
|---------|--------|
| Tables  | Ready  |
"#;
let lines = renderer.render(markdown);
// Produces formatted table output
```

**Custom styling** (future enhancement, not in initial release):
```rust
// Hypothetical future API (NOT IMPLEMENTED YET)
let renderer = MarkdownRenderer::with_options(TableOptions {
    border_style: BorderStyle::Double,
    min_column_width: 10,
});
```

## Change Log

### Version 1.0 (Initial Implementation)

**Added**:
- Table rendering support in `MarkdownRenderer::render()`
- TableBuilder, TableCell, Alignment types
- Column width calculation with truncation
- Alignment support (left/center/right)
- Escaped pipe handling
- Missing cell padding

**Modified**:
- `MarkdownRenderer` internal state machine (added table handlers)

**No Breaking Changes**: Existing API signatures unchanged

## References

- **Feature Spec**: `../spec.md`
- **Data Model**: `../data-model.md`
- **Research**: `../research.md`
- **Code Location**: `src/tui/markdown.rs`
- **Tests**: `src/tui/markdown.rs` (unit), `tests/integration/markdown_rendering_test.rs`
