# Data Model: Markdown Table Rendering

**Feature**: 001-markdown-table-rendering
**Date**: 2025-12-11
**Status**: Design Complete

## Overview

This document defines the data structures and types used to represent and render markdown tables in the hoosh CLI. The model extends the existing `MarkdownRenderer` in `src/tui/markdown.rs` without requiring database persistence or external state management.

## Core Entities

### 1. TableBuilder

**Purpose**: Accumulates table data during markdown parsing before rendering

**Location**: `src/tui/markdown.rs` (new struct)

**Structure**:
```rust
struct TableBuilder {
    /// Header row cells
    headers: Vec<TableCell>,

    /// Column alignment specifications (left, center, right)
    alignments: Vec<Alignment>,

    /// Data rows, each containing cells
    rows: Vec<Vec<TableCell>>,

    /// Flag indicating if currently parsing header section
    in_header: bool,

    /// Currently accumulating row (before finalization)
    current_row: Vec<TableCell>,

    /// Currently accumulating cell (before finalization)
    current_cell: TableCell,
}
```

**Lifecycle**:
1. **Creation**: On `Event::Start(Tag::Table)`
2. **Population**: During table parsing events
3. **Rendering**: On `Event::End(Tag::Table)` → consumed by `render_table()`
4. **Destruction**: Dropped after rendering

**State Transitions**:
```
Created → InHeader → ProcessingRows → Complete
         (on TableHead)  (on TableRow)   (on End(Table))
```

**Validation Rules**:
- Headers can be empty (fallback: single column)
- Alignments default to Left if not specified
- Rows with fewer cells than headers are padded (FR-011)
- Minimum viable output: 2 columns when terminal < 40 chars (FR-013)

### 2. TableCell

**Purpose**: Represents a single table cell with styled content

**Location**: `src/tui/markdown.rs` (new struct)

**Structure**:
```rust
struct TableCell {
    /// Styled content spans (preserves formatting like bold/italic)
    spans: Vec<Span<'static>>,

    /// Cached visual width (excludes ANSI codes, counts unicode properly)
    visual_width: usize,
}
```

**Attributes**:
- `spans`: Preserves markdown formatting within cells (bold, italic, code)
- `visual_width`: Pre-calculated for performance, used in width calculation and alignment

**Creation**:
```rust
impl TableCell {
    fn new() -> Self {
        Self {
            spans: Vec::new(),
            visual_width: 0,
        }
    }

    fn add_span(&mut self, span: Span<'static>) {
        use unicode_width::UnicodeWidthStr;
        self.visual_width += UnicodeWidthStr::width(span.content.as_ref());
        self.spans.push(span);
    }

    fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
}
```

**Invariants**:
- `visual_width` must always equal sum of span visual widths
- `spans` can be empty (represents empty cell)
- Visual width calculation must use `unicode-width` crate for accuracy

### 3. Alignment

**Purpose**: Specifies column alignment strategy

**Location**: `src/tui/markdown.rs` (new enum)

**Structure**:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Alignment {
    /// Left-aligned (default) - markdown syntax: `:--` or `---`
    Left,

    /// Center-aligned - markdown syntax: `:-:`
    Center,

    /// Right-aligned - markdown syntax: `--:`
    Right,
}
```

**Default**: `Alignment::Left`

**Derivation**: Extracted from markdown separator row (e.g., `|:--|:-:|--:|`)

**Parsing Logic**:
```rust
fn parse_alignment(separator: &str) -> Alignment {
    let trimmed = separator.trim_matches('-');
    match (trimmed.starts_with(':'), trimmed.ends_with(':')) {
        (true, true) => Alignment::Center,
        (false, true) => Alignment::Right,
        _ => Alignment::Left,  // default includes (true, false)
    }
}
```

### 4. ColumnWidths

**Purpose**: Calculated widths for each table column

**Location**: Used in `render_table()` method (transient)

**Structure**:
```rust
struct ColumnWidths {
    /// Width for each column (includes padding, excludes borders)
    widths: Vec<usize>,

    /// Total table width (includes all borders and separators)
    total_width: usize,

    /// Flags indicating which columns are truncated
    truncated: Vec<bool>,
}
```

**Calculation Strategy** (from research.md Decision 2):
```rust
fn calculate_column_widths(
    table: &TableBuilder,
    terminal_width: usize,
) -> ColumnWidths {
    let num_cols = table.headers.len().max(
        table.rows.iter().map(|r| r.len()).max().unwrap_or(0)
    );

    // Calculate ideal widths based on content
    let mut widths: Vec<usize> = (0..num_cols)
        .map(|col_idx| {
            let header_width = table.headers.get(col_idx)
                .map(|c| c.visual_width)
                .unwrap_or(0);
            let max_row_width = table.rows.iter()
                .filter_map(|row| row.get(col_idx))
                .map(|cell| cell.visual_width)
                .max()
                .unwrap_or(0);
            header_width.max(max_row_width) + 2  // +2 for padding
        })
        .collect();

    // Calculate total with borders: | col1 | col2 | col3 |
    let borders_width = num_cols + 1;  // num_cols + 1 pipes
    let total_width: usize = widths.iter().sum::<usize>() + borders_width;

    // Apply truncation if needed
    let mut truncated = vec![false; num_cols];
    if total_width > terminal_width {
        apply_truncation(&mut widths, &mut truncated, terminal_width, borders_width);
    }

    ColumnWidths {
        widths,
        total_width,
        truncated,
    }
}
```

**Constraints**:
- Minimum column width: 3 characters (for "...")
- At least 2 columns visible when terminal < 40 chars (FR-013)
- Proportional reduction when total exceeds terminal width

## Supporting Types

### 5. TableRenderState

**Purpose**: Tracks rendering state during table event processing

**Location**: Field in `MarkdownRenderer` (modified existing struct)

**Addition to existing MarkdownRenderer**:
```rust
pub struct MarkdownRenderer {
    // ... existing fields ...

    /// Table being accumulated (Some when inside table tags)
    current_table: Option<TableBuilder>,
}
```

**State Machine**:
- `None`: Not currently parsing a table
- `Some(builder)`: Actively accumulating table data

**Transitions**:
- `None → Some(builder)`: On `Event::Start(Tag::Table)`
- `Some(builder) → None`: On `Event::End(Tag::Table)` (after rendering)

## Data Flow

### Parsing Flow

```
Markdown Input
    ↓
pulldown-cmark Parser
    ↓
Event Stream: Start(Table) → Start(TableHead) → Start(TableRow) → Start(TableCell) → Text → End(TableCell) → ...
    ↓
MarkdownRenderer::render()
    ↓
Event Handler (match on Tag::Table*)
    ↓
TableBuilder::accumulate()
    ↓
[Header cells] + [Alignment specs] + [Data rows]
    ↓
TableBuilder (complete)
```

### Rendering Flow

```
TableBuilder (complete)
    ↓
calculate_column_widths(terminal_width)
    ↓
ColumnWidths
    ↓
render_header_line(headers, widths, alignments)
    ↓
render_separator_line(widths)
    ↓
render_data_lines(rows, widths, alignments)
    ↓
Vec<Line<'static>>
    ↓
Append to MarkdownRenderer output
    ↓
Return to MessageRenderer
    ↓
Display in Terminal
```

### Example Flow (Concrete)

**Input**:
```markdown
| Name | Age |
|------|-----|
| Alice | 30 |
```

**Event Sequence**:
1. `Start(Table)` → Create `TableBuilder`
2. `Start(TableHead)` → Set `in_header = true`
3. `Start(TableRow)` → Initialize `current_row`
4. `Start(TableCell)` → Initialize `current_cell`
5. `Text("Name")` → Add span to `current_cell`
6. `End(TableCell)` → Push `current_cell` to `current_row`
7. `Start(TableCell)` → New `current_cell`
8. `Text("Age")` → Add span
9. `End(TableCell)` → Push to `current_row`
10. `End(TableRow)` → Move `current_row` to `builder.headers`
11. `End(TableHead)` → Set `in_header = false`, parse alignments from next row
12. [Separator row processed for alignments]
13. `Start(TableRow)` → New data row
14. [Similar cell processing for "Alice", "30"]
15. `End(TableRow)` → Push to `builder.rows`
16. `End(Table)` → Call `render_table(builder)` → Output lines

**Rendered Output**:
```
| Name  | Age |
|-------|-----|
| Alice | 30  |
```

## Entity Relationships

```
MarkdownRenderer
    ├── current_table: Option<TableBuilder>
    │
    └── (on End(Table)) → render_table()
                              │
                              ↓
                        TableBuilder
                         ├── headers: Vec<TableCell>
                         ├── alignments: Vec<Alignment>
                         └── rows: Vec<Vec<TableCell>>
                                      │
                                      └── TableCell
                                           ├── spans: Vec<Span<'static>>
                                           └── visual_width: usize

ColumnWidths (transient)
    ├── widths: Vec<usize>
    ├── total_width: usize
    └── truncated: Vec<bool>
```

**Cardinality**:
- `MarkdownRenderer` : `TableBuilder` = 1 : 0..1
- `TableBuilder` : `TableCell` = 1 : N (headers + all row cells)
- `TableCell` : `Span` = 1 : N
- `TableBuilder` : `Alignment` = 1 : N (one per column)

## Memory Characteristics

### Size Estimates

**TableCell** (per cell):
- `Vec<Span>`: 24 bytes (pointer, length, capacity) + span data
- `visual_width`: 8 bytes (usize)
- **Total**: ~32 bytes + span content

**TableBuilder** (per table):
- `headers`: 24 bytes + (num_cols × TableCell size)
- `alignments`: 24 bytes + (num_cols × 1 byte)
- `rows`: 24 bytes + (num_rows × (24 bytes + row data))
- **Typical table** (10 cols, 50 rows): ~30KB

**Performance Impact**:
- Temporary allocation during parsing (dropped after rendering)
- Typical CLI tables: < 100KB memory footprint
- No heap fragmentation concerns (short-lived)

### Optimization Notes

**Pre-allocation**:
```rust
impl TableBuilder {
    fn with_capacity(estimated_rows: usize, estimated_cols: usize) -> Self {
        Self {
            headers: Vec::with_capacity(estimated_cols),
            alignments: Vec::with_capacity(estimated_cols),
            rows: Vec::with_capacity(estimated_rows),
            // ...
        }
    }
}
```

**Ownership**:
- `Span<'static>`: Requires owned strings (no lifetime issues)
- `TableBuilder`: Moved into `render_table()`, no cloning needed
- Zero-copy where possible (spans already owned from markdown parsing)

## Validation & Constraints

### Input Validation

**At TableBuilder creation**:
- No validation needed (empty builder is valid)

**During accumulation**:
- Cell text can be any string (including empty)
- No maximum cell count enforced (trust markdown parser)

**At rendering**:
- **Column count mismatch**: Pad rows with empty cells (FR-011)
- **Missing alignments**: Default to `Alignment::Left`
- **Zero columns**: Treat as empty table, output nothing

### Constraints (from spec)

**FR-009**: Tables exceeding terminal width → Truncate with ellipsis
- Implementation: `ColumnWidths.truncated` flags
- Render: Append "..." to truncated cells

**FR-011**: Rows with fewer cells → Pad with empty
- Implementation: Check `row.len() < headers.len()`, extend with `TableCell::new()`

**FR-013**: Terminal < 40 chars → Show minimum 2 columns
- Implementation: Prioritize first 2 columns, truncate rest
- Edge case: Single-column table → show full column

**SC-006**: Performance < 100ms for 100 rows
- Implementation: No lazy rendering initially (per research.md)
- Validation: Benchmark test required

## Testing Considerations

### Unit Test Scenarios

**TableCell**:
- `add_span()` correctly updates `visual_width`
- Unicode characters (emoji, CJK) calculated correctly
- Empty cell has `visual_width == 0`

**TableBuilder**:
- Accumulates headers and rows correctly
- Handles missing cells (pad with empty)
- Extracts alignments from separator row

**Alignment**:
- `parse_alignment()` handles all markdown variations
- Default to Left for ambiguous cases

**ColumnWidths**:
- `calculate_column_widths()` produces correct widths
- Truncation logic respects minimum column requirements
- Narrow terminal (< 40 chars) shows 2 columns

### Integration Test Scenarios

**End-to-end**:
- Markdown with table → `MarkdownRenderer::render()` → Correct output lines
- Verify pipe characters visible (FR-001)
- Verify header separator line (FR-004)
- Verify alignment (left/center/right) (FR-006)

**Edge cases**:
- Table with escaped pipes (`\|`) in cells (FR-012)
- Table with bold/italic formatting in cells (FR-008)
- Wide table on narrow terminal (FR-009, FR-013)

## Future Considerations

### Not Included in Current Design

**Out of scope** (per spec.md):
- Nested tables
- Interactive features (sorting, filtering)
- Color coding within tables (beyond existing markdown styling)
- Automatic table width optimization based on content heuristics

**Potential future enhancements**:
- Lazy rendering for very large tables (>1000 rows)
- Horizontal scrolling support
- Cell text wrapping (multi-line cells)
- Custom table styling (border characters)

## References

- **Feature Spec**: `specs/001-markdown-table-rendering/spec.md`
- **Research**: `specs/001-markdown-table-rendering/research.md`
- **Existing Code**: `src/tui/markdown.rs` (MarkdownRenderer)
- **Constitution**: `.specify/memory/constitution.md` (principles I, II, V)
- **Dependencies**: pulldown-cmark 0.11, ratatui 0.29, unicode-width 0.1

## Appendix: Type Signatures

```rust
// Core types
struct TableBuilder { /* ... */ }
struct TableCell { /* ... */ }
enum Alignment { Left, Center, Right }
struct ColumnWidths { /* ... */ }

// Key methods
impl MarkdownRenderer {
    fn render_table(&self, builder: TableBuilder, terminal_width: usize) -> Vec<Line<'static>>;
    fn calculate_column_widths(&self, table: &TableBuilder, terminal_width: usize) -> ColumnWidths;
    fn render_header_line(&self, headers: &[TableCell], widths: &ColumnWidths, alignments: &[Alignment]) -> Line<'static>;
    fn render_separator_line(&self, widths: &ColumnWidths) -> Line<'static>;
    fn render_data_line(&self, row: &[TableCell], widths: &ColumnWidths, alignments: &[Alignment]) -> Line<'static>;
    fn apply_alignment(&self, cell: &TableCell, width: usize, alignment: Alignment, truncated: bool) -> Vec<Span<'static>>;
}

impl TableBuilder {
    fn new() -> Self;
    fn with_capacity(rows: usize, cols: usize) -> Self;
    fn add_header_cell(&mut self, cell: TableCell);
    fn add_data_cell(&mut self, cell: TableCell);
    fn finalize_row(&mut self);
    fn set_alignments(&mut self, alignments: Vec<Alignment>);
}

impl TableCell {
    fn new() -> Self;
    fn add_span(&mut self, span: Span<'static>);
    fn is_empty(&self) -> bool;
}
```
