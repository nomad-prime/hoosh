# Research: Markdown Table Rendering

**Feature**: 001-markdown-table-rendering
**Date**: 2025-12-11
**Status**: Complete

## Executive Summary

This research document addresses all technical unknowns for implementing markdown table rendering in the hoosh CLI. The investigation focused on understanding the current markdown rendering architecture, evaluating table rendering strategies, and determining the optimal approach for integration.

## Current Architecture Analysis

### Existing Markdown Infrastructure

**Library**: pulldown-cmark 0.11
- Industry-standard CommonMark parser used in Rust ecosystem
- Event-driven parsing model (Start/End tag events)
- Already supports table parsing via `Tag::Table`, `Tag::TableHead`, `Tag::TableRow`, `Tag::TableCell`
- Table support is built-in but currently not handled in hoosh (empty handlers at markdown.rs:128-130)

**Rendering System**: MarkdownRenderer (`src/tui/markdown.rs`)
- Converts pulldown-cmark events to ratatui `Line<'static>` and `Span<'static>` types
- Line-based output model (returns `Vec<Line<'static>>`)
- State machine approach for tracking formatting context
- Integration with syntax highlighting (syntect) for code blocks

**Terminal Framework**: ratatui 0.29.0
- Modern terminal UI framework with extensive widget support
- `Paragraph` widget used for rendering styled text
- Terminal width-aware through custom terminal abstraction
- Full color and styling support

### Key Finding

Tables are explicitly identified but not implemented. The empty handler at lines 128-130 of `src/tui/markdown.rs` is the root cause of missing pipe characters and table structure.

## Research Questions & Decisions

### 1. Table Rendering Strategy

**Question**: How should markdown tables be converted to terminal output given the line-based rendering model?

**Options Evaluated**:

A. **ASCII Table Formatting** (Line-by-line construction)
   - Build table as formatted ASCII art with proper spacing
   - Calculate column widths based on content
   - Generate border characters using pipes and dashes
   - Pros: Matches markdown source appearance, terminal-native
   - Cons: Complex width calculation, alignment logic needed

B. **Compact Table Format** (Simplified representation)
   - Header row, separator, data rows without column alignment
   - Minimal width calculation
   - Pros: Simple implementation
   - Cons: Poor readability, doesn't meet spec requirements

C. **Widget-Based Rendering** (New ratatui Table widget)
   - Use ratatui's `Table` widget for rendering
   - Pros: Built-in handling of widths, borders, alignment
   - Cons: Requires restructuring rendering pipeline, may not integrate cleanly with line-based output

**Decision**: Option A - ASCII Table Formatting

**Rationale**:
- Aligns with line-based output model (`Vec<Line<'static>>`)
- Maintains consistency with existing markdown rendering approach
- Meets all spec requirements (pipe visibility, alignment, truncation)
- No architectural changes required to rendering pipeline
- Full control over formatting behavior

**Alternatives Considered**:
- Option C (Widget-Based) rejected because ratatui's Table widget expects structured input and doesn't integrate cleanly with the current event-driven, line-accumulation model used for all other markdown elements
- Option B rejected because it fails to meet visibility and readability requirements in the spec

### 2. Column Width Calculation

**Question**: How should column widths be determined to balance content visibility and terminal width constraints?

**Options Evaluated**:

A. **Content-Based Sizing** - Width = max(content) per column
B. **Equal Distribution** - Width = terminal_width / num_columns
C. **Hybrid Approach** - Content-based with max limits, fallback to truncation

**Decision**: Option C - Hybrid Approach

**Rationale**:
- Prioritizes content visibility for narrow columns
- Respects terminal width constraints (FR-009, FR-013)
- Enables ellipsis truncation when needed (clarification decision)
- Balances readability with practicality

**Implementation Details**:
```
For each column:
  ideal_width = max(cell_content_lengths) + padding
  max_width = (terminal_width - borders) / num_columns * 1.5  // Allow some columns to be wider
  actual_width = min(ideal_width, max_width)

If total_width > terminal_width:
  Apply proportional reduction with minimum viable width per column
  Priority truncation: rightmost columns first
  Minimum: 2 columns visible (per clarification FR-013)
```

### 3. Alignment Implementation

**Question**: How should left/center/right alignment be implemented at the character level?

**Options Evaluated**:

A. **String Padding** - Use format! with width specifiers (`{:<width}`, `{:^width}`, `{:>width}`)
B. **Manual Spacing** - Calculate spaces needed and construct aligned strings
C. **Unicode-Aware Alignment** - Account for multi-byte characters and ANSI codes

**Decision**: Option A with unicode-width awareness

**Rationale**:
- Rust's format specifiers are concise and well-tested
- Must account for unicode character width (use `unicode-width` crate)
- ANSI color codes don't affect visual width
- Aligns with Rust ecosystem best practices

**Implementation**:
```rust
use unicode_width::UnicodeWidthStr;

fn pad_cell(content: &str, width: usize, alignment: Alignment) -> String {
    let visual_width = UnicodeWidthStr::width(content);
    match alignment {
        Alignment::Left => format!("{:<width$}", content, width = width),
        Alignment::Center => {
            let pad_left = (width - visual_width) / 2;
            let pad_right = width - visual_width - pad_left;
            format!("{}{}{}", " ".repeat(pad_left), content, " ".repeat(pad_right))
        },
        Alignment::Right => format!("{:>width$}", content, width = width),
    }
}
```

### 4. Escaped Pipe Character Handling

**Question**: How should backslash-escaped pipes (`\|`) be processed during parsing and rendering?

**Options Evaluated**:

A. **Pre-processing** - Scan markdown before parsing, replace `\|` with placeholder
B. **Post-processing** - Handle after pulldown-cmark parsing
C. **Parser-Level** - Rely on pulldown-cmark's escape handling

**Decision**: Option C - Parser-Level with verification

**Rationale**:
- pulldown-cmark already handles standard markdown escapes
- Testing revealed it correctly treats `\|` as literal text in cell content
- No custom escape logic needed
- Maintains compatibility with CommonMark spec

**Verification**:
```rust
// Test case: pulldown-cmark handles this correctly
let markdown = "| Header |\n|---|\n| Content with \\| pipe |";
// Parses correctly, pipe not treated as delimiter
```

### 5. Header Separator Rendering

**Question**: What character pattern should be used for the separator line between header and data rows?

**Options Evaluated**:

A. **Dash-Only** - `|---------|---------|`
B. **Mixed Characters** - `|=========|=========|`
C. **Unicode Box Drawing** - `├─────────┼─────────┤`

**Decision**: Option A - Dash-Only

**Rationale**:
- Matches standard markdown table syntax exactly
- ASCII-only ensures compatibility across all terminals (no unicode rendering issues)
- Visual consistency with markdown source
- Clear distinction from regular rows
- Aligns with clarification decision (separator line of dashes)

**Implementation**:
```rust
fn render_separator(column_widths: &[usize]) -> Line<'static> {
    let parts: Vec<String> = column_widths
        .iter()
        .map(|w| "-".repeat(*w))
        .collect();
    Line::from(format!("|{}|", parts.join("|")))
}
```

### 6. Formatting Preservation Within Cells

**Question**: How should bold, italic, and code formatting within table cells be handled while maintaining table structure?

**Options Evaluated**:

A. **Strip Formatting** - Render plain text only
B. **Preserve Styling** - Use ratatui Span with modifiers
C. **Escape Formatting** - Show markdown syntax literally

**Decision**: Option B - Preserve Styling (with caveats)

**Rationale**:
- Meets FR-008 requirement to preserve markdown formatting
- ratatui's `Span` supports modifiers (Bold, Italic, etc.)
- Enhances readability and information density
- Consistent with rest of markdown rendering

**Implementation Approach**:
```rust
// Cell content rendered as Vec<Span> with preserved styles
struct TableCell {
    spans: Vec<Span<'static>>,  // Preserve formatting
    visual_width: usize,          // For layout calculation
}

// Width calculation must account for ANSI codes not affecting visual width
fn calculate_visual_width(spans: &[Span]) -> usize {
    spans.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum()
}
```

**Caveat**: Very complex nested formatting may need fallback to plain text if it breaks layout. Implement sanity check during rendering.

### 7. Performance Considerations

**Question**: What performance optimizations are needed for large tables (up to 100 rows per SC-006)?

**Analysis**:

**Measurements Needed**:
- Width calculation: O(n*m) where n=rows, m=columns
- String allocation: O(n*m*avg_cell_length)
- Terminal rendering: O(n) rows

**Optimization Strategy**:

A. **Lazy Rendering** - Only render visible rows (viewport)
   - Con: Requires integration with scrolling state
   - Benefit: Handles arbitrarily large tables

B. **Batch Allocation** - Pre-allocate String capacity
   - Con: Memory overhead for large tables
   - Benefit: Reduces reallocation

C. **Simple Eager Rendering** - Render all rows upfront
   - Con: May exceed 100ms for very large tables
   - Benefit: Simple implementation

**Decision**: Start with Option C, measure, then optimize if needed

**Rationale**:
- SC-006 specifies <100ms for 100 rows
- Typical tables in CLI output are <50 rows
- Premature optimization violates constitution (Simplicity principle)
- Profile with `cargo bench` if performance issues arise

**Measurement Plan**:
```rust
#[bench]
fn bench_table_rendering(b: &mut Bencher) {
    let markdown = create_table(100, 10);  // 100 rows, 10 columns
    b.iter(|| markdown_renderer.render(&markdown));
}
// Target: <100ms avg
```

### 8. Error Handling for Malformed Tables

**Question**: How should malformed table markdown be handled (per edge case in spec)?

**Options Evaluated**:

A. **Strict Mode** - Reject/skip malformed tables, show error
B. **Graceful Degradation** - Render as plain text
C. **Best-Effort Parsing** - Attempt to recover and render partial table

**Decision**: Option C - Best-Effort Parsing

**Rationale**:
- Aligns with constitution principle V (Simplicity and Clarity)
- Better user experience than silent failure
- Consistent with how other markdown elements are handled
- Clarification decision: pad missing cells (handles one type of malformation)

**Implementation**:
```rust
// Malformed table scenarios:
// 1. Missing cells → pad with empty (per FR-011)
// 2. No header row → treat first row as header
// 3. No separator → synthesize default left-align separator
// 4. Mixed column counts → use max column count, pad others
```

## Technology Recommendations

### Required Dependencies

**New Dependency**: `unicode-width = "0.1"`
- **Purpose**: Accurate visual width calculation for alignment
- **Rationale**: Unicode characters (emoji, CJK) have variable display widths
- **Alternative**: Manual width calculation - rejected (bug-prone, reinventing wheel)
- **Justification**: Industry standard, minimal footprint (14KB), widely used in Rust CLI ecosystem

### No Additional Dependencies Needed

- **pulldown-cmark**: Already present, supports tables
- **ratatui**: Already present, provides necessary primitives
- **textwrap**: Already present, handles line wrapping integration
- **syntect**: Not needed for tables

## Integration Architecture

### Module Structure

**Location**: Extend existing `src/tui/markdown.rs`

**New Components**:
```rust
// Add to markdown.rs (maintain single-module approach per constitution)

struct TableBuilder {
    headers: Vec<TableCell>,
    alignments: Vec<Alignment>,
    rows: Vec<Vec<TableCell>>,
    in_header: bool,
}

enum Alignment {
    Left,
    Center,
    Right,
}

struct TableCell {
    spans: Vec<Span<'static>>,
    visual_width: usize,
}

impl MarkdownRenderer {
    fn render_table(&self, builder: TableBuilder, terminal_width: usize) -> Vec<Line<'static>> {
        // Calculate widths
        // Build header line
        // Build separator line
        // Build data lines
        // Handle truncation if needed
    }

    fn calculate_column_widths(&self, table: &TableBuilder, max_width: usize) -> Vec<usize> {
        // Hybrid width calculation per Decision 2
    }
}
```

**Design Decision**: Extend existing module rather than create new `table.rs` module

**Rationale**:
- Table rendering is part of markdown rendering functionality
- Keeps related code together (cohesion)
- Avoids premature abstraction (constitution principle V)
- Total module size will be ~700 lines (acceptable for single responsibility)

### State Machine Updates

**Current Flow**:
```
Event::Start(Tag::...) → Update state → Store formatting
Event::Text(..) → Apply current formatting → Accumulate content
Event::End(Tag::...) → Finalize element → Append to lines
```

**Table Extension**:
```
Event::Start(Tag::Table) → Create TableBuilder
Event::Start(Tag::TableHead) → Set builder.in_header = true
Event::Start(Tag::TableRow) → Start new row accumulation
Event::Start(Tag::TableCell) → Start new cell accumulation
Event::Text(..) → Accumulate into current cell (preserve formatting)
Event::End(Tag::TableCell) → Finalize cell → Add to current row
Event::End(Tag::TableRow) → Finalize row → Add to builder
Event::End(Tag::TableHead) → Extract alignments from separator row
Event::End(Tag::Table) → render_table(builder) → Append lines
```

## Testing Strategy

### Unit Tests

**Test Coverage Required** (per constitution principle IV):

```rust
#[cfg(test)]
mod table_tests {
    // Happy path
    #[test]
    fn renders_simple_table() { /* 3x3 table, verify structure */ }

    #[test]
    fn preserves_all_pipe_characters() { /* FR-001 */ }

    #[test]
    fn applies_column_alignment() { /* FR-006: left/center/right */ }

    // Edge cases
    #[test]
    fn handles_missing_cells() { /* FR-011: pad with empty */ }

    #[test]
    fn truncates_wide_tables() { /* FR-009: ellipsis */ }

    #[test]
    fn handles_narrow_terminal() { /* FR-013: min 2 columns */ }

    #[test]
    fn escapes_pipes_in_cells() { /* FR-012: \| handling */ }

    #[test]
    fn preserves_cell_formatting() { /* FR-008: bold/italic */ }

    // Performance
    #[test]
    fn renders_large_table_quickly() { /* SC-006: <100ms for 100 rows */ }
}
```

### Integration Tests

**Test File**: `tests/integration/markdown_rendering_test.rs` (new file)

```rust
#[test]
fn table_integrates_with_message_renderer() {
    // Full pipeline: markdown string → MessageRenderer → styled lines
}

#[test]
fn tables_work_with_text_wrapping() {
    // Verify tables + wrapping don't conflict
}
```

### Visual Regression Tests

**Approach**: Snapshot testing for terminal output

```rust
// Use insta crate for snapshot testing
#[test]
fn table_visual_snapshot() {
    let markdown = "| A | B |\n|---|---|\n| 1 | 2 |";
    let output = render_to_string(&markdown);
    insta::assert_snapshot!(output);
}
```

## Risk Analysis

### Technical Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Unicode width miscalculation causes misalignment | High | Medium | Use unicode-width crate; extensive test coverage with emoji, CJK |
| Performance regression for large tables | Medium | Low | Benchmark early; implement lazy rendering if needed |
| Complex cell formatting breaks layout | Medium | Medium | Fallback to plain text for overly complex cells; document limit |
| Terminal width detection edge cases | Low | Low | Use ratatui's terminal abstraction; already battle-tested |

### Compatibility Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Incompatible with Windows terminals | Medium | Low | ASCII-only table chars; test on Windows terminal, PowerShell |
| Color rendering issues | Low | Very Low | Ratatui handles cross-platform color; already working for other elements |
| pulldown-cmark version incompatibility | Low | Very Low | Version pinned in Cargo.toml; test suite will catch breaks |

## Success Metrics

Per spec Success Criteria:

- **SC-001**: 100% pipe character visibility → Verify with test suite
- **SC-002**: Tables up to 10x50 render correctly → Integration tests
- **SC-003**: Tabular data readable → Visual snapshot tests
- **SC-004**: Cross-platform consistency → CI testing on Linux, macOS, Windows
- **SC-005**: Alignment specifications work → Unit tests per alignment type
- **SC-006**: <100ms for 100 rows → Benchmark test with `cargo bench`

## Implementation Phases

### Phase 1: Basic Table Structure (P1 - Core)
- Implement TableBuilder and state machine updates
- Render simple tables with pipes and dashes
- No alignment, no truncation, no formatting preservation
- **Deliverable**: Tables visible with correct structure

### Phase 2: Alignment & Formatting (P2 - Enhancement)
- Implement column alignment (left/center/right)
- Preserve bold/italic/code within cells
- **Deliverable**: Professional-looking tables

### Phase 3: Width Constraints (P1 - Core)
- Implement width calculation and truncation
- Handle narrow terminals (min 2 columns)
- Add ellipsis for truncated content
- **Deliverable**: Tables work in all terminal widths

### Phase 4: Edge Cases (P3 - Polish)
- Handle escaped pipes
- Handle missing cells (padding)
- Handle malformed tables (best-effort)
- **Deliverable**: Robust table handling

## References

- **pulldown-cmark docs**: https://docs.rs/pulldown-cmark/0.11/
- **ratatui docs**: https://docs.rs/ratatui/0.29/
- **CommonMark spec tables extension**: https://github.github.com/gfm/#tables-extension-
- **unicode-width crate**: https://docs.rs/unicode-width/0.1/
- **Hoosh constitution**: `.specify/memory/constitution.md` (principles I-V)

## Appendix: Example Rendering

### Input Markdown:
```markdown
| Category | Status | Notes |
|----------|--------|-------|
| **Functional** | Clear | Well-defined |
| **Data Model** | Partial | Needs work |
```

### Expected Terminal Output:
```
| Category        | Status  | Notes         |
|-----------------|---------|---------------|
| Functional      | Clear   | Well-defined  |
| Data Model      | Partial | Needs work    |
```
(with bold formatting preserved on "Functional" and "Data Model")

### When Terminal Width = 60:
```
| Category    | Status  | Notes        |
|-------------|---------|--------------|
| Functional  | Clear   | Well-defi... |
| Data Model  | Partial | Needs work   |
```
(note truncation with ellipsis)

## Conclusion

All technical unknowns have been resolved. The implementation approach is clear, aligns with the project constitution, and integrates cleanly with existing architecture. The path forward requires:

1. Add `unicode-width` dependency
2. Extend `MarkdownRenderer` with table support (~300 lines)
3. Implement test suite (~200 lines)
4. Verify performance benchmarks

No architectural changes needed. No constitution violations. Ready for Phase 1 design.
