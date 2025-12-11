# Tasks: Markdown Table Rendering

**Input**: Design documents from `/specs/001-markdown-table-rendering/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Tests are included based on constitution principle IV (Testing Discipline) and spec requirements

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/`, `tests/` at repository root
- Paths assume single project structure from plan.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and dependency setup

- [X] T001 Add unicode-width = "0.1" dependency to Cargo.toml
- [X] T002 [P] Run cargo build to verify dependency resolution and existing codebase compiles

**Checkpoint**: Dependencies ready for table rendering implementation

---

## Phase 2: Foundational (Core Types & Infrastructure)

**Purpose**: Core data structures that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T003 Define TableCell struct with spans and visual_width in src/tui/markdown.rs
- [X] T004 Implement TableCell::new() and TableCell::add_span() methods in src/tui/markdown.rs
- [X] T005 Define Alignment enum (Left, Center, Right) in src/tui/markdown.rs
- [X] T006 Define TableBuilder struct with headers, alignments, rows, in_header, current_row, current_cell in src/tui/markdown.rs
- [X] T007 Implement TableBuilder::new() constructor in src/tui/markdown.rs
- [X] T008 Add current_table: Option<TableBuilder> field to MarkdownRenderer struct in src/tui/markdown.rs

**Checkpoint**: Foundation types ready - user story implementation can now begin

---

## Phase 3: User Story 1 - View Structured Analysis Tables (Priority: P1) 🎯 MVP

**Goal**: Users can see properly formatted tables with visible pipe characters, correct structure, and header separators in CLI output

**Independent Test**: Output any markdown table to CLI and verify pipe characters visible, columns aligned, table structure preserved

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T009 [P] [US1] Write test_render_simple_table() unit test in src/tui/markdown.rs testing 2x2 table rendering with pipe visibility
- [X] T010 [P] [US1] Write test_header_separator() unit test in src/tui/markdown.rs testing dash separator line below headers
- [X] T011 [P] [US1] Write test_empty_cells_no_collapse() unit test in src/tui/markdown.rs verifying empty cells maintain column width
- [X] T012 [P] [US1] Write test_truncate_wide_table() unit test in src/tui/markdown.rs testing ellipsis truncation when table exceeds terminal width

### Implementation for User Story 1

- [X] T0*13 [US1] Implement TableBuilder cell accumulation methods (add_header_cell, add_data_cell, finalize_row) in src/tui/markdown.rs
- [X] T0*14 [US1] Implement Event::Start(Tag::Table) handler to create TableBuilder in src/tui/markdown.rs
- [X] T0*15 [US1] Implement Event::Start(Tag::TableHead) handler to set in_header flag in src/tui/markdown.rs
- [X] T0*16 [US1] Implement Event::Start(Tag::TableCell) handler to initialize current_cell in src/tui/markdown.rs
- [X] T0*17 [US1] Implement Event::Text handler for table cells to add spans to current_cell in src/tui/markdown.rs
- [X] T0*18 [US1] Implement Event::End(Tag::TableCell) handler to finalize cell and add to current_row in src/tui/markdown.rs
- [X] T0*19 [US1] Implement Event::End(Tag::TableRow) handler to finalize row in src/tui/markdown.rs
- [X] T0*20 [US1] Implement Event::End(Tag::TableHead) handler to move headers and parse alignment in src/tui/markdown.rs
- [X] T0*21 [US1] Implement calculate_column_widths() function using hybrid content-based sizing in src/tui/markdown.rs
- [X] T0*22 [US1] Implement apply_truncation() helper for when table exceeds terminal width in src/tui/markdown.rs
- [X] T0*23 [US1] Implement render_header_line() function to format header row with pipes in src/tui/markdown.rs
- [X] T0*24 [US1] Implement render_separator_line() function to create dash separator in src/tui/markdown.rs
- [X] T0*25 [US1] Implement render_data_line() function to format data rows in src/tui/markdown.rs
- [X] T0*26 [US1] Implement render_table() main function coordinating width calculation and line rendering in src/tui/markdown.rs
- [X] T0*27 [US1] Implement Event::End(Tag::Table) handler to call render_table() and append lines in src/tui/markdown.rs
- [X] T0*28 [US1] Run cargo test to verify User Story 1 tests pass

**Checkpoint**: At this point, basic table rendering should work - tables visible with structure, pipes, and header separators

---

## Phase 4: User Story 2 - View Complex Tables with Special Characters (Priority: P2)

**Goal**: Users can view tables with formatting (bold/italic) within cells, special characters, escaped pipes, and varying content lengths while maintaining structure

**Independent Test**: Create tables with bold/italic text, special characters, escaped pipes, varying lengths - verify structure maintained and formatting preserved

### Tests for User Story 2

- [X] T0*29 [P] [US2] Write test_formatted_cells() unit test in src/tui/markdown.rs testing bold/italic preservation within cells
- [X] T0*30 [P] [US2] Write test_escaped_pipes_in_cells() unit test in src/tui/markdown.rs verifying \| renders as literal pipe
- [X] T0*31 [P] [US2] Write test_special_characters() unit test in src/tui/markdown.rs testing parentheses, hyphens, quotes in cells
- [X] T0*32 [P] [US2] Write test_varying_cell_lengths() unit test in src/tui/markdown.rs verifying alignment with mixed content lengths

### Implementation for User Story 2

- [X] T0*33 [US2] Enhance TableCell::add_span() to preserve Span styling (Bold, Italic modifiers) in src/tui/markdown.rs
- [X] T0*34 [US2] Update Event::Text handler to respect current formatting state (bold/italic/code) when in table cells in src/tui/markdown.rs
- [X] T0*35 [US2] Implement pad_rows_with_empty_cells() helper to handle missing cells (FR-011) in src/tui/markdown.rs
- [X] T0*36 [US2] Update render_data_line() to render cells with multiple styled spans in src/tui/markdown.rs
- [X] T0*37 [US2] Add unicode-width visual width calculation for cells with formatting in src/tui/markdown.rs
- [X] T0*38 [US2] Verify escaped pipe handling works correctly (test that pulldown-cmark parser handles \|) in src/tui/markdown.rs
- [X] T0*39 [US2] Run cargo test to verify User Story 2 tests pass

**Checkpoint**: At this point, tables with formatted content and special characters should render correctly

---

## Phase 5: User Story 3 - View Tables with Varying Alignment (Priority: P3)

**Goal**: Users can view tables with left/center/right alignment specifications that respect markdown syntax and maintain visual hierarchy

**Independent Test**: Create tables with alignment specs (|:--|, |:-:|, |--:|) and verify output respects these alignments

### Tests for User Story 3

- [X] T0*40 [P] [US3] Write test_left_alignment() unit test in src/tui/markdown.rs testing left-aligned columns
- [X] T0*41 [P] [US3] Write test_center_alignment() unit test in src/tui/markdown.rs testing center-aligned columns
- [X] T0*42 [P] [US3] Write test_right_alignment() unit test in src/tui/markdown.rs testing right-aligned columns
- [X] T0*43 [P] [US3] Write test_mixed_alignment() unit test in src/tui/markdown.rs testing tables with different alignments per column

### Implementation for User Story 3

- [X] T0*44 [US3] Implement parse_alignment() function to extract alignment from separator syntax in src/tui/markdown.rs
- [X] T0*45 [US3] Update Event::End(Tag::TableHead) handler to parse alignments from separator row in src/tui/markdown.rs
- [X] T0*46 [US3] Implement apply_cell_alignment() function for left alignment (pad right with spaces) in src/tui/markdown.rs
- [X] T0*47 [US3] Implement apply_cell_alignment() function for center alignment (pad both sides) in src/tui/markdown.rs
- [X] T0*48 [US3] Implement apply_cell_alignment() function for right alignment (pad left with spaces) in src/tui/markdown.rs
- [X] T0*49 [US3] Update render_data_line() to apply alignment when rendering cell content in src/tui/markdown.rs
- [X] T0*50 [US3] Update render_header_line() to apply alignment to header cells in src/tui/markdown.rs
- [X] T0*51 [US3] Run cargo test to verify User Story 3 tests pass

**Checkpoint**: All alignment specifications should now work correctly for all user stories

---

## Phase 6: Edge Cases & Robustness

**Purpose**: Handle edge cases and narrow terminal scenarios

- [ ] T052 [P] Write test_narrow_terminal() unit test in src/tui/markdown.rs testing minimum 2 column visibility when width < 40
- [ ] T053 [P] Write test_missing_cells_padded() unit test in src/tui/markdown.rs verifying rows with fewer cells are padded
- [ ] T054 [P] Write test_malformed_table_recovery() unit test in src/tui/markdown.rs testing best-effort rendering of malformed tables
- [ ] T055 Implement narrow terminal handling in calculate_column_widths() to ensure minimum 2 columns visible in src/tui/markdown.rs
- [ ] T056 Implement missing cell padding logic in render_table() before rendering lines in src/tui/markdown.rs
- [ ] T057 Add default alignment (Left) fallback when alignments not specified in src/tui/markdown.rs
- [ ] T058 Run cargo test to verify all edge case tests pass

---

## Phase 7: Integration & Performance

**Purpose**: End-to-end testing and performance validation

- [ ] T059 [P] Create tests/integration/markdown_rendering_test.rs integration test file
- [ ] T060 [P] Write test_table_in_message_pipeline() integration test verifying full markdown → MessageRenderer → terminal pipeline in tests/integration/markdown_rendering_test.rs
- [ ] T061 [P] Write test_multiple_tables_in_markdown() integration test testing multiple tables in single markdown string in tests/integration/markdown_rendering_test.rs
- [ ] T062 [P] Write test_table_with_other_markdown_elements() integration test verifying tables work alongside headings, lists, code blocks in tests/integration/markdown_rendering_test.rs
- [ ] T063 [P] Create benches/markdown_bench.rs benchmark file (create benches/ directory if needed)
- [ ] T064 [P] Write bench_small_table() benchmark for 10x5 table in benches/markdown_bench.rs
- [ ] T065 [P] Write bench_large_table() benchmark for 100x10 table (must be <100ms per SC-006) in benches/markdown_bench.rs
- [ ] T066 [P] Write bench_wide_table() benchmark for table with many columns requiring truncation in benches/markdown_bench.rs
- [ ] T067 Run cargo test --test '*' to verify all integration tests pass
- [ ] T068 Run cargo +nightly bench (if nightly available) to verify performance targets met

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories and final validation

- [ ] T069 [P] Add inline documentation comments for TableBuilder, TableCell, Alignment types in src/tui/markdown.rs
- [ ] T070 [P] Add inline documentation comments for render_table() and helper functions in src/tui/markdown.rs
- [ ] T071 Run cargo clippy to check for linting issues
- [ ] T072 Run cargo fmt to format code according to Rust standards
- [ ] T073 Manually test table rendering in actual hoosh CLI with sample markdown files
- [ ] T074 [P] Update quickstart.md if any implementation details changed from design
- [ ] T075 Run all tests one final time: cargo test && cargo test --test '*'
- [ ] T076 Verify all functional requirements FR-001 through FR-013 are satisfied by running acceptance tests

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion (T001-T002) - BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational phase (T003-T008) - Core table rendering
- **User Story 2 (Phase 4)**: Depends on Foundational phase (T003-T008) - Can run in parallel with US1 OR after US1
- **User Story 3 (Phase 5)**: Depends on Foundational phase (T003-T008) - Can run in parallel with US1/US2 OR after them
- **Edge Cases (Phase 6)**: Depends on User Story 1 completion (T009-T028) - Extends core rendering
- **Integration (Phase 7)**: Depends on all user stories being complete
- **Polish (Phase 8)**: Depends on all previous phases

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories ✅ INDEPENDENT
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - Extends US1 but is independently testable ✅ INDEPENDENT
- **User Story 3 (P3)**: Can start after Foundational (Phase 2) - Extends US1 but is independently testable ✅ INDEPENDENT

**Key Insight**: All three user stories are independently testable. Each can be validated on its own with appropriate test markdown.

### Within Each User Story

- Tests (T009-T012 for US1, T029-T032 for US2, T040-T043 for US3) MUST be written FIRST and FAIL before implementation
- Event handlers must be implemented in order: Start(Table) → Start(TableHead) → Start(TableRow) → Start(TableCell) → Text → End(TableCell) → End(TableRow) → End(TableHead) → End(Table)
- Rendering helpers (calculate_column_widths, render_*_line) can be implemented in parallel once TableBuilder is complete
- Final Event::End(Table) handler depends on all rendering helpers being complete

### Parallel Opportunities

- **Setup phase**: T001 and T002 can run in parallel
- **Foundational phase**: T003-T008 must be sequential (struct definitions build on each other)
- **User Story 1 tests**: T009-T012 can all run in parallel (write all 4 tests simultaneously)
- **User Story 1 impl**: T013-T022 (helpers) can run in parallel; T023-T027 are sequential
- **User Story 2 tests**: T029-T032 can all run in parallel
- **User Story 2 impl**: T033-T037 can run in parallel
- **User Story 3 tests**: T040-T043 can all run in parallel
- **User Story 3 impl**: T046-T048 (alignment functions) can run in parallel
- **Edge cases tests**: T052-T054 can all run in parallel
- **Integration tests**: T059-T062 can all run in parallel
- **Benchmarks**: T063-T066 can all run in parallel
- **Polish phase**: T069-T070, T071-T072, T074 can all run in parallel

---

## Parallel Example: User Story 1

```bash
# Launch all tests for User Story 1 together:
Task T009: "Write test_render_simple_table() unit test in src/tui/markdown.rs"
Task T010: "Write test_header_separator() unit test in src/tui/markdown.rs"
Task T011: "Write test_empty_cells_no_collapse() unit test in src/tui/markdown.rs"
Task T012: "Write test_truncate_wide_table() unit test in src/tui/markdown.rs"

# After tests written, launch parallel helper implementations:
Task T021: "Implement calculate_column_widths() function in src/tui/markdown.rs"
Task T022: "Implement apply_truncation() helper in src/tui/markdown.rs"
Task T023: "Implement render_header_line() function in src/tui/markdown.rs"
Task T024: "Implement render_separator_line() function in src/tui/markdown.rs"
Task T025: "Implement render_data_line() function in src/tui/markdown.rs"
```

---

## Parallel Example: User Story 3

```bash
# Launch all alignment tests together:
Task T040: "Write test_left_alignment() unit test in src/tui/markdown.rs"
Task T041: "Write test_center_alignment() unit test in src/tui/markdown.rs"
Task T042: "Write test_right_alignment() unit test in src/tui/markdown.rs"
Task T043: "Write test_mixed_alignment() unit test in src/tui/markdown.rs"

# Launch all alignment implementations together:
Task T046: "Implement apply_cell_alignment() for left in src/tui/markdown.rs"
Task T047: "Implement apply_cell_alignment() for center in src/tui/markdown.rs"
Task T048: "Implement apply_cell_alignment() for right in src/tui/markdown.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T002)
2. Complete Phase 2: Foundational (T003-T008) - CRITICAL
3. Complete Phase 3: User Story 1 (T009-T028)
4. **STOP and VALIDATE**: Test User Story 1 independently with simple markdown tables
5. Commit and potentially deploy/demo basic table rendering

**MVP Delivers**: Basic table rendering with visible pipes, structure, header separators, and truncation

### Incremental Delivery

1. Setup + Foundational → Foundation ready (T001-T008)
2. Add User Story 1 → Test independently → **MVP Ready!** (T009-T028)
3. Add User Story 2 → Test independently → Formatted tables work (T029-T039)
4. Add User Story 3 → Test independently → Alignment works (T040-T051)
5. Add Edge Cases → Narrow terminals handled (T052-T058)
6. Add Integration & Performance → Full validation (T059-T068)
7. Polish → Production ready (T069-T076)

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together (T001-T008)
2. Once Foundational is done:
   - **Developer A**: User Story 1 (T009-T028) - Core rendering
   - **Developer B**: User Story 2 tests (T029-T032) → wait for US1 core → implement (T033-T039)
   - **Developer C**: User Story 3 tests (T040-T043) → wait for US1 core → implement (T044-T051)
3. All stories complete and work independently
4. Team collaborates on Edge Cases, Integration, Polish

**Note**: While US2 and US3 are independently testable, they build on US1 core rendering, so practical execution may be sequential or with US1 foundation first.

---

## Notes

- [P] tasks = different files or independent implementations, no dependencies
- [Story] label maps task to specific user story for traceability
- All tasks in src/tui/markdown.rs - single file modification per plan.md
- Each user story should be independently completable and testable
- Verify tests fail before implementing (TDD approach)
- Commit after each logical task group (e.g., after each user story phase)
- Stop at any checkpoint to validate story independently
- All tests use #[cfg(test)] inline in src/tui/markdown.rs per Rust conventions
- Integration tests in separate tests/integration/ directory
- Benchmarks in benches/ directory (may need to create)

---

## Task Count Summary

- **Phase 1 (Setup)**: 2 tasks
- **Phase 2 (Foundational)**: 6 tasks
- **Phase 3 (User Story 1)**: 20 tasks (4 tests + 16 implementation)
- **Phase 4 (User Story 2)**: 11 tasks (4 tests + 7 implementation)
- **Phase 5 (User Story 3)**: 12 tasks (4 tests + 8 implementation)
- **Phase 6 (Edge Cases)**: 7 tasks (3 tests + 4 implementation)
- **Phase 7 (Integration)**: 10 tasks (6 tests + 4 benchmarks)
- **Phase 8 (Polish)**: 8 tasks

**Total**: 76 tasks

**Parallel Opportunities**:
- 16 tasks marked [P] can run in parallel (tests, independent helpers, documentation)
- 3 user stories can work in parallel after foundational phase (with coordination)
- Significant parallelization possible within each user story phase
