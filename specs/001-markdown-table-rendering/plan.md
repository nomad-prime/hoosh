# Implementation Plan: Markdown Table Rendering

**Branch**: `001-markdown-table-rendering` | **Date**: 2025-12-11 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-markdown-table-rendering/spec.md`

## Summary

Fix markdown table rendering in hoosh CLI by implementing proper table structure output with visible pipe characters, column alignment, and terminal width-aware truncation. Currently, tables are explicitly ignored in the rendering pipeline (empty handlers in `src/tui/markdown.rs:128-130`), causing pipe characters to be omitted and table structure to be lost.

**Technical Approach**: Extend the existing `MarkdownRenderer` with table support by adding `TableBuilder`, `TableCell`, and `Alignment` types. Implement event handlers for `Tag::Table*` events from pulldown-cmark parser. Calculate column widths dynamically with hybrid content-based and constraint-based sizing. Render tables as formatted ASCII art using pipes and dashes, preserving markdown formatting within cells while maintaining table structure.

## Technical Context

**Language/Version**: Rust 2024 edition (matches project `Cargo.toml:4`)
**Primary Dependencies**:
- pulldown-cmark 0.11 (markdown parsing - already present)
- ratatui 0.29.0 (terminal UI - already present)
- unicode-width 0.1 (NEW - visual width calculation for alignment)
- textwrap (line wrapping - already present)

**Storage**: N/A (no persistence, transient rendering only)
**Testing**: cargo test (unit tests in `src/tui/markdown.rs`, integration tests in `tests/integration/`)
**Target Platform**: Cross-platform CLI (Linux, macOS, Windows terminals)
**Project Type**: Single project (TUI application)
**Performance Goals**:
- <100ms rendering for tables up to 100 rows × 10 columns (SC-006)
- No blocking of main UI thread (table rendering is synchronous but fast)

**Constraints**:
- Terminal width must be respected (truncate with ellipsis if needed)
- Minimum 2 columns visible on narrow terminals (<40 chars)
- ASCII-only table characters (cross-platform compatibility)
- No breaking changes to existing markdown rendering

**Scale/Scope**:
- Typical tables: 5-50 rows, 3-10 columns
- Maximum tested: 100 rows × 10 columns
- Code addition: ~300 lines to `markdown.rs`, ~200 lines tests
- Single module extension (no new files)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Verify compliance with `.specify/memory/constitution.md`:

- [x] **Modularity First**: Feature extends existing `markdown.rs` module (single responsibility: markdown rendering). Table rendering is cohesive with other markdown elements. No new modules needed.
- [x] **Explicit Error Handling**: No error paths (tables render best-effort with graceful degradation). Malformed input handled without errors. Aligns with existing markdown renderer error handling (no `Result` types).
- [x] **Async-First Architecture**: N/A - Rendering is synchronous CPU-bound operation. No I/O involved. Existing `MarkdownRenderer::render()` is synchronous.
- [x] **Testing Discipline**: Tests planned with behavior-focused names (`test_render_simple_table`, `test_truncate_wide_table`). Coverage includes happy paths, edge cases (missing cells, narrow terminals), and performance benchmarks.
- [x] **Simplicity and Clarity**: Single new dependency justified (unicode-width for correctness). No premature abstraction. Extends existing patterns (event-driven parsing). Descriptive type names (`TableBuilder`, `TableCell`, `Alignment`).

**Violations**: None. All constitution principles satisfied.

### Post-Design Re-Check (Phase 1 Complete)

- [x] **Modularity First**: Design maintains single module approach. `TableBuilder` and related types defined in `markdown.rs`. Clear separation: accumulation (TableBuilder) vs rendering (render_table methods).
- [x] **Explicit Error Handling**: Data model confirms no error returns. Best-effort parsing with defaults (empty cells, left alignment).
- [x] **Async-First Architecture**: Confirmed N/A. No async boundaries in rendering pipeline.
- [x] **Testing Discipline**: Test scenarios documented in data-model.md. Behavioral focus maintained (e.g., "handles missing cells" not "test padding logic").
- [x] **Simplicity and Clarity**: Data structures kept simple. No complex inheritance. Clear ownership (TableBuilder consumed by render_table). Unicode-width dependency justified for correctness.

**Final Status**: ✅ All principles satisfied. No violations. Ready for implementation.

## Project Structure

### Documentation (this feature)

```text
specs/001-markdown-table-rendering/
├── plan.md              # This file (/speckit.plan command output)
├── spec.md              # Feature specification (user requirements)
├── research.md          # Phase 0 output (technical research, decisions)
├── data-model.md        # Phase 1 output (type definitions, entity relationships)
├── quickstart.md        # Phase 1 output (developer guide)
├── contracts/           # Phase 1 output
│   └── rendering-interface.md  # API contract for table rendering
├── checklists/
│   └── requirements.md  # Specification quality checklist (from /speckit.specify)
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT YET CREATED)
```

### Source Code (repository root)

```text
# Single project structure (Option 1)
src/
├── tui/
│   ├── mod.rs                 # Module exports
│   ├── markdown.rs            # MODIFIED: Add table rendering (TableBuilder, TableCell, Alignment types + render methods)
│   ├── message_renderer.rs    # UNCHANGED: Integration point (no modifications needed)
│   ├── colors.rs              # POTENTIALLY MODIFIED: Add table-specific colors if needed
│   └── terminal/
│       └── mod.rs             # UNCHANGED: Terminal abstraction
├── main.rs                    # UNCHANGED
└── lib.rs                     # UNCHANGED

tests/
├── integration/
│   └── markdown_rendering_test.rs  # NEW: End-to-end table rendering tests
└── unit/
    └── (unit tests in src/tui/markdown.rs inline with #[cfg(test)])

benches/  # May need to create if doesn't exist
└── markdown_bench.rs           # NEW: Performance benchmarks for table rendering
```

**Structure Decision**: Selected **Option 1: Single project**. This is a TUI application with unified codebase under `src/`. Table rendering is an extension of existing markdown rendering functionality in `src/tui/markdown.rs`. No separate frontend/backend or mobile components. Tests follow Rust conventions (unit tests inline, integration tests in `tests/`).

**Key Files Modified**:
- `src/tui/markdown.rs` - Main changes (add ~300 lines)
- `Cargo.toml` - Add `unicode-width = "0.1"` dependency

**Key Files Added**:
- `tests/integration/markdown_rendering_test.rs` - Integration tests
- `benches/markdown_bench.rs` - Performance benchmarks (optional, if benches/ doesn't exist)

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No violations. This section is empty.

## Technical Architecture

### Current State Analysis

**Existing Infrastructure** (from research.md and codebase exploration):

1. **Markdown Parser**: pulldown-cmark 0.11
   - Event-driven parsing (Start/End tags, Text events)
   - Table support built-in (`Tag::Table`, `Tag::TableHead`, `Tag::TableRow`, `Tag::TableCell`)
   - Already handles table parsing, just not rendered

2. **Rendering System**: `MarkdownRenderer` in `src/tui/markdown.rs`
   - Line-based output model: `render() -> Vec<Line<'static>>`
   - State machine approach for tracking formatting context
   - Supports headings, lists, code blocks, quotes, emphasis, etc.
   - **Gap**: Empty handlers for table tags (lines 128-130)

3. **Terminal Framework**: ratatui 0.29.0
   - `Line` and `Span` primitives for styled text
   - Terminal width detection available
   - Full color and modifier support (Bold, Italic, etc.)

4. **Integration Points**:
   - `MessageRenderer::render_markdown_message()` - Calls `MarkdownRenderer::render()`
   - `AppState` - Stores messages with `MessageLine::Markdown` variant
   - No changes needed to integration points (transparent extension)

### Implementation Phases

**Phase 0: Research** ✅ COMPLETE
- Explored codebase to understand current architecture
- Evaluated table rendering strategies (decided: ASCII table formatting)
- Resolved technical unknowns (width calculation, alignment, truncation)
- Output: `research.md`

**Phase 1: Design** ✅ COMPLETE
- Defined data structures (`TableBuilder`, `TableCell`, `Alignment`)
- Documented API contracts and behavioral guarantees
- Created developer quick start guide
- Output: `data-model.md`, `contracts/rendering-interface.md`, `quickstart.md`

**Phase 2: Task Breakdown** (NEXT - via `/speckit.tasks`)
- Generate actionable tasks from design
- Sequence implementation steps
- Assign priorities and dependencies
- Output: `tasks.md`

**Phase 3: Implementation** (AFTER /speckit.tasks)
- Implement TableBuilder and accumulation logic
- Implement render_table() and width calculation
- Add alignment and truncation support
- Handle edge cases (escaped pipes, missing cells)
- Write tests and benchmarks

### Key Design Decisions (from research.md)

1. **Table Rendering Strategy**: ASCII table formatting (line-by-line)
   - Rationale: Aligns with line-based output model, no architectural changes needed

2. **Column Width Calculation**: Hybrid approach (content-based with max limits)
   - Rationale: Balances visibility with terminal constraints

3. **Alignment Implementation**: Rust format specifiers with unicode-width awareness
   - Rationale: Concise, correct handling of multi-byte characters

4. **Escaped Pipe Handling**: Rely on pulldown-cmark parser
   - Rationale: Parser already handles escapes correctly, no custom logic needed

5. **Header Separator**: Dash-only (`|---|---|`)
   - Rationale: Matches markdown syntax, ASCII-only for compatibility

6. **Formatting in Cells**: Preserve styling with ratatui Span
   - Rationale: Meets FR-008, enhances readability

7. **Performance Strategy**: Simple eager rendering, measure first
   - Rationale: Avoid premature optimization (constitution principle V)

8. **Error Handling**: Best-effort parsing, graceful degradation
   - Rationale: No error paths needed, consistent with existing renderer

### Data Flow

```
User Input (markdown with table)
    ↓
pulldown-cmark Parser
    ↓
Event Stream
    ↓
MarkdownRenderer::render()
    ├→ Event::Start(Tag::Table) → Create TableBuilder
    ├→ Event::Start(Tag::TableCell) → Initialize current_cell
    ├→ Event::Text(...) → Add span to current_cell
    ├→ Event::End(Tag::TableCell) → Finalize cell
    ├→ Event::End(Tag::TableRow) → Finalize row
    └→ Event::End(Tag::Table) → render_table(builder)
                                      ↓
                                calculate_column_widths()
                                      ↓
                                render_header_line()
                                render_separator_line()
                                render_data_lines()
                                      ↓
                                Vec<Line<'static>>
    ↓
MessageRenderer::wrap_styled_lines()
    ↓
Terminal Display
```

### Core Components

**TableBuilder** (accumulation):
- Purpose: Collect table data during parsing
- Lifecycle: Created on `Start(Table)`, consumed on `End(Table)`
- State: headers, alignments, rows, in_header flag

**TableCell** (content + metadata):
- Purpose: Represent styled cell content
- Properties: spans (styled text), visual_width (cached for performance)
- Invariant: visual_width = sum of span widths (unicode-aware)

**Alignment** (enum):
- Purpose: Specify column alignment
- Values: Left (default), Center, Right
- Derived from: Markdown separator row syntax

**ColumnWidths** (transient):
- Purpose: Calculated widths for rendering
- Properties: widths per column, total width, truncation flags
- Algorithm: Hybrid (content-based + constraint-based)

### Integration Strategy

**Non-Breaking Extension**:
- Existing `MarkdownRenderer::render()` signature unchanged
- Return type unchanged: `Vec<Line<'static>>`
- No changes to `MessageRenderer` or `AppState`
- Tables transparently integrated into existing pipeline

**Testing Strategy**:
- Unit tests: `src/tui/markdown.rs` (inline with #[cfg(test)])
- Integration tests: `tests/integration/markdown_rendering_test.rs`
- Benchmarks: `benches/markdown_bench.rs`
- Visual regression: Snapshot testing with `insta` crate (optional)

### Performance Considerations

**Target**: <100ms for 100 rows × 10 columns (SC-006)

**Optimization Opportunities** (implement if needed):
1. Pre-allocate vectors with capacity
2. Cache column widths (calculated once)
3. Reuse span allocations where possible
4. Profile with `cargo bench` and `flamegraph`

**Current Approach**: Simple eager rendering
- Rationale: Profile first, optimize if needed (avoid premature optimization)

## Dependencies

### Existing Dependencies (No Changes)

- **pulldown-cmark 0.11**: Markdown parsing (table support already present)
- **ratatui 0.29**: Terminal UI primitives (Line, Span, styling)
- **textwrap**: Line wrapping (used post-rendering)
- **syntect 5.2**: Syntax highlighting for code blocks (not used in tables)

### New Dependency

**unicode-width 0.1**:
- **Purpose**: Accurate visual width calculation for unicode characters
- **Rationale**: Required for correct alignment with emoji, CJK characters
- **Alternative**: Manual width calculation - rejected (bug-prone, incomplete)
- **Size**: ~14KB
- **License**: MIT/Apache-2.0 (compatible)
- **Usage**: `UnicodeWidthStr::width()` for all width calculations
- **Justification**: Industry standard, minimal footprint, correctness-critical

**Add to Cargo.toml**:
```toml
[dependencies]
unicode-width = "0.1"
```

## Risk Assessment

### Technical Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Unicode width miscalculation causes misalignment | High | Medium | Use unicode-width crate; test with emoji, CJK characters |
| Performance regression for large tables | Medium | Low | Benchmark early; profile if needed; lazy rendering fallback |
| Terminal width detection edge cases | Low | Low | Use ratatui's terminal abstraction (battle-tested) |
| Complex cell formatting breaks layout | Medium | Medium | Fallback to plain text for overly complex cells; document limits |

### Compatibility Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Windows terminal incompatibilities | Medium | Low | ASCII-only table chars; CI testing on Windows |
| Color rendering issues across terminals | Low | Very Low | Ratatui handles cross-platform color; already working for other elements |

### Mitigation Status

- ✅ unicode-width crate selected (mitigates alignment risk)
- ✅ ASCII-only design (mitigates Windows risk)
- ✅ Benchmark tests planned (mitigates performance risk)
- ✅ Best-effort rendering (mitigates malformed input risk)

## Success Criteria Mapping

Per spec.md Success Criteria:

| Criterion | Implementation Strategy | Verification Method |
|-----------|-------------------------|---------------------|
| **SC-001**: 100% pipe visibility | All table lines include `\|` characters | Unit test: count pipes in output |
| **SC-002**: 10×50 tables render correctly | Column width calculation handles up to 10 cols | Integration test with 10×50 table |
| **SC-003**: Readable output | Structured ASCII table with alignment | Manual review + snapshot tests |
| **SC-004**: Cross-platform consistency | ASCII-only chars, ratatui color handling | CI on Linux, macOS, Windows |
| **SC-005**: Alignment works | Left/center/right alignment logic | Unit tests per alignment type |
| **SC-006**: <100ms for 100 rows | Efficient rendering, no unnecessary allocation | Benchmark test (cargo bench) |

## Implementation Priorities

**Phase 1 (P1 - Core Functionality)**:
- TableBuilder accumulation logic
- Basic table structure rendering (pipes and dashes)
- Width calculation and truncation
- Header separator line
- Empty cell handling

**Phase 2 (P2 - Enhancements)**:
- Column alignment (left/center/right)
- Formatting preservation in cells (bold, italic)
- Narrow terminal handling (<40 chars)

**Phase 3 (P3 - Edge Cases)**:
- Escaped pipe handling
- Malformed table recovery
- Performance optimization (if needed)

## Next Steps

1. ✅ **Phase 0 Complete**: Research and technical investigation
   - Output: `research.md`

2. ✅ **Phase 1 Complete**: Design and contracts
   - Output: `data-model.md`, `contracts/rendering-interface.md`, `quickstart.md`

3. **Phase 2 (NEXT)**: Generate actionable tasks
   - Command: `/speckit.tasks`
   - Output: `tasks.md` with step-by-step implementation plan

4. **Phase 3 (AFTER /speckit.tasks)**: Implementation
   - Follow tasks.md for execution
   - Test-driven development (write tests first)
   - Incremental commits per task

5. **Phase 4**: Validation and acceptance
   - Run full test suite
   - Verify all success criteria
   - Manual testing with real tables
   - Performance benchmarking

## References

- **Feature Spec**: [spec.md](./spec.md) - User requirements and acceptance criteria
- **Research**: [research.md](./research.md) - Technical decisions and alternatives evaluated
- **Data Model**: [data-model.md](./data-model.md) - Type definitions and entity relationships
- **Contracts**: [contracts/rendering-interface.md](./contracts/rendering-interface.md) - API behavioral contracts
- **Quick Start**: [quickstart.md](./quickstart.md) - Developer guide
- **Constitution**: `../../.specify/memory/constitution.md` - Project principles and standards

## Appendix: Key Code Locations

**Current Code**:
- `src/tui/markdown.rs:128-130` - Empty table handlers (TO BE REPLACED)
- `src/tui/markdown.rs:519` - MarkdownRenderer implementation
- `src/tui/message_renderer.rs:102-117` - Integration point (render_markdown_message)
- `Cargo.toml:53` - pulldown-cmark dependency

**New Code Locations**:
- `src/tui/markdown.rs:~130-430` - Table rendering implementation (~300 lines)
- `tests/integration/markdown_rendering_test.rs` - Integration tests (~200 lines)
- `benches/markdown_bench.rs` - Performance benchmarks (~50 lines)
- `Cargo.toml` - Add unicode-width dependency

**Test Coverage Targets**:
- Unit tests: 10-15 tests covering core functionality
- Integration tests: 5-10 tests covering full pipeline
- Benchmarks: 2-3 scenarios (small, large, wide tables)
- Total test code: ~200-300 lines
