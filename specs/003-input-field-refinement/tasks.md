# Tasks: Input Field Refinement

**Input**: Design documents from `/specs/003-input-field-refinement/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

**Tests**: Test tasks are included per Constitution Principle I (Test-First Development)

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

Repository root structure (single project):
- `src/` - Source code
- `tests/` - Test files (unit/ and integration/ subdirectories)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create module structure and define core types

- [ ] T001 Create `src/tui/input/` module directory and `src/tui/input/mod.rs`
- [ ] T002 [P] Define `InputMode` enum in `src/tui/app_state.rs` with variants: Normal, Expanded, AttachmentList, AttachmentView
- [ ] T003 [P] Define `TextAttachment` struct in `src/tui/input/attachment.rs` with fields: id, content, size_chars, line_count, created_at
- [ ] T004 [P] Define `PasteClassification` enum in `src/tui/input/paste_detector.rs` with variants: Inline, Attachment, Rejected(String)
- [ ] T005 [P] Define `WrappedLine` struct in `src/tui/input/wrapping.rs` with fields: content, is_soft_wrap

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [ ] T006 Add new fields to `AppState` in `src/tui/app_state.rs`: `attachments: Vec<TextAttachment>`, `next_attachment_id: usize`, `input_mode: InputMode`, `attachment_view: Option<AttachmentViewState>`
- [ ] T007 Implement `AppState::create_attachment()` method in `src/tui/app_state.rs` with size validation (>200 chars, <=5MB)
- [ ] T008 [P] Implement `AppState::delete_attachment()` method in `src/tui/app_state.rs` with ID-based lookup
- [ ] T009 [P] Implement `AppState::clear_attachments()` method in `src/tui/app_state.rs` to clear vector and reset ID counter
- [ ] T010 [P] Implement `AppState::get_attachment()` method in `src/tui/app_state.rs` for ID-based retrieval
- [ ] T011 [P] Define `AttachmentViewState` struct in `src/tui/app_state.rs` with fields: attachment_id, editor (TextArea), is_modified
- [ ] T012 Add `ToggleExpandedMode` and `OpenAttachmentList` actions to `src/tui/actions.rs`

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Paste Large Content Without Breaking UI (Priority: P1) 🎯 MVP

**Goal**: Handle large paste operations gracefully by creating attachments for content >200 chars, preventing UI breakage

**Independent Test**: Paste content exceeding 200 characters and verify UI displays `[pasted text-#id]` reference with stable rendering. Paste 500 lines of code and confirm UI remains responsive. Paste content >5MB and confirm clear error message.

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T013 [P] [US1] Unit test for `PasteDetector::classify_paste()` in `tests/unit/paste_detector_tests.rs` covering: ≤200 chars (Inline), >200 chars <=5MB (Attachment), >5MB (Rejected), exactly 200 chars (Inline)
- [ ] T014 [P] [US1] Unit test for `AppState::create_attachment()` in `tests/unit/attachment_tests.rs` covering: sequential ID generation, size validation, character count calculation, line count calculation
- [ ] T015 [P] [US1] Integration test for paste workflow in `tests/integration/input_attachment_tests.rs` covering: small paste inline insertion, large paste attachment creation with reference token, oversized paste rejection with error display, attachment expansion on submit

### Implementation for User Story 1

- [ ] T016 [P] [US1] Implement `PasteDetector` struct and `new()` method in `src/tui/input/paste_detector.rs` with threshold and max_size fields
- [ ] T017 [US1] Implement `PasteDetector::classify_paste()` method in `src/tui/input/paste_detector.rs` with byte size check first, then character count check
- [ ] T018 [US1] Modify `paste_handler.rs` in `src/tui/handlers/paste_handler.rs` to integrate PasteDetector and route pastes to inline vs attachment
- [ ] T019 [US1] Implement attachment reference token insertion logic in `src/tui/handlers/paste_handler.rs` with format `[pasted text-{id}]`
- [ ] T020 [US1] Implement attachment expansion logic in `src/tui/handlers/submit_handler.rs` to replace tokens with full content before LLM submission
- [ ] T021 [US1] Add attachment clearing call in `src/tui/handlers/submit_handler.rs` after successful submission
- [ ] T022 [US1] Add error display for rejected pastes in `src/tui/handlers/paste_handler.rs` using existing error mechanism

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently - paste large content without UI breakage

---

## Phase 4: User Story 2 - Text Wraps to Terminal Width (Priority: P1)

**Goal**: Implement automatic text wrapping at terminal width boundaries with visual indicators for soft-wrap points

**Independent Test**: Type text until reaching terminal edge and verify automatic wrapping. Resize terminal and confirm dynamic rewrapping within 100ms. Verify soft-wrap points display ↩ symbol while hard breaks have no indicator. Navigate with arrow keys through wrapped content and confirm correct cursor movement.

### Tests for User Story 2

- [ ] T023 [P] [US2] Unit test for `WrappingCalculator::wrap_text()` in `tests/unit/wrapping_tests.rs` covering: word boundary wrapping, soft-wrap indicator marking, hard line break preservation, force-breaking words exceeding terminal width, Unicode/emoji width calculation
- [ ] T024 [P] [US2] Integration test for wrapping behavior in `tests/integration/input_wrapping_tests.rs` covering: typing past terminal width triggers wrap, terminal resize triggers rewrap <100ms, cursor navigation across wrapped lines, indicator display at soft-wrap points only

### Implementation for User Story 2

- [ ] T025 [P] [US2] Implement `WrappingCalculator` struct in `src/tui/input/wrapping.rs` with terminal_width and wrap_indicator fields
- [ ] T026 [US2] Implement `WrappingCalculator::wrap_text()` method in `src/tui/input/wrapping.rs` using unicode-width for character width calculations, preserving hard breaks (\n), marking soft-wrap points
- [ ] T027 [US2] Implement force-break logic for words exceeding terminal width in `src/tui/input/wrapping.rs` with visual indicator
- [ ] T028 [US2] Integrate wrapping calculation into `Input` component rendering in `src/tui/components/input.rs` to display wrapped text with indicators
- [ ] T029 [US2] Add terminal resize event handler in `src/tui/app_loop.rs` to trigger rewrapping calculation
- [ ] T030 [US2] Add visual indicator rendering (↩ symbol) at soft-wrap points in `src/tui/components/input.rs` using ratatui styling
- [ ] T031 [US2] Implement cursor navigation logic for wrapped content in `src/tui/handlers/text_input_handler.rs` to correctly move cursor across soft-wrap boundaries and hard breaks, mapping visual line positions to content positions

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently - text wraps cleanly at terminal boundaries

---

## Phase 5: User Story 3 - Edit in Expanded View (Priority: P2)

**Goal**: Provide expanded editor mode (50-60% terminal height) for comfortable multi-line editing with Ctrl+E / Esc keybindings

**Independent Test**: Press Ctrl+E and verify interface switches to expanded view occupying 50-60% of terminal height. Enter 50+ lines of content and verify smooth scrolling. Press Esc and verify return to normal mode with all content preserved.

### Tests for User Story 3

- [ ] T032 [P] [US3] Integration test for expanded mode toggling in `tests/integration/input_expanded_mode_tests.rs` covering: Ctrl+E activates expanded mode, expanded view occupies 50-60% height, Esc returns to normal mode, content preserved across mode switches, scrolling in expanded view for 100+ lines

### Implementation for User Story 3

- [ ] T033 [P] [US3] Create `ExpandedEditor` component struct in `src/tui/components/expanded_editor.rs` implementing Component trait
- [ ] T034 [US3] Implement expanded area calculation logic in `src/tui/components/expanded_editor.rs` as 55% of terminal height, min 10 lines, centered vertically
- [ ] T035 [US3] Implement `ExpandedEditor::render()` method in `src/tui/components/expanded_editor.rs` rendering TextArea with block title "Expanded Editor (Esc to exit)"
- [ ] T036 [US3] Add Ctrl+E keybinding handler in `src/tui/handlers/text_input_handler.rs` to set `input_mode = InputMode::Expanded`
- [ ] T037 [US3] Add Esc keybinding handler in `src/tui/handlers/text_input_handler.rs` to return to Normal mode when in Expanded mode
- [ ] T038 [US3] Integrate mode switching into rendering logic in `src/tui/app_loop.rs` to render ExpandedEditor when `input_mode == Expanded`, otherwise Input component
- [ ] T039 [US3] Add scrollbar rendering for expanded editor in `src/tui/components/expanded_editor.rs` when content exceeds visible area
- [ ] T040 [US3] Add visual styling for expanded mode in `src/tui/components/expanded_editor.rs` with distinct border color/thickness using palette colors

**Checkpoint**: All P1 and P2 user stories should now be independently functional - expanded editing is comfortable

---

## Phase 6: User Story 4 - Manage Attached Content (Priority: P3)

**Goal**: Provide attachment management UI to list, view, edit, and delete attachments with Ctrl+A keybinding

**Independent Test**: Create 2 attachments by pasting large content. Press Ctrl+A and verify attachment list displays with IDs, sizes, and line counts. Select attachment and press Enter to view/edit. Make edits, save with Ctrl+S, and verify changes persist. Delete attachment with 'd' and verify reference disappears from input.

### Tests for User Story 4

- [ ] T041 [P] [US4] Integration test for attachment management in `tests/integration/input_attachment_tests.rs` covering: Ctrl+A opens attachment list, list shows IDs and metadata, Enter opens attachment for viewing, editing and saving with Ctrl+S, deletion with 'd' removes attachment and reference, Esc closes attachment UI

### Implementation for User Story 4

- [ ] T042 [P] [US4] Create `AttachmentList` component struct in `src/tui/components/attachment_list.rs` with selection state
- [ ] T043 [P] [US4] Create `AttachmentViewer` component struct in `src/tui/components/attachment_viewer.rs` for editing attachments
- [ ] T044 [US4] Implement `AttachmentList::render()` method in `src/tui/components/attachment_list.rs` displaying attachments with metadata (ID, size, line count)
- [ ] T045 [US4] Implement `AttachmentViewer::render()` method in `src/tui/components/attachment_viewer.rs` with TextArea editor and title showing attachment ID
- [ ] T046 [US4] Add Ctrl+A keybinding handler in `src/tui/handlers/text_input_handler.rs` to set `input_mode = InputMode::AttachmentList`
- [ ] T047 [US4] Create `attachment_handler.rs` in `src/tui/handlers/attachment_handler.rs` with navigation handlers (Up/Down arrows, Enter to open, 'd' to delete, Esc to close)
- [ ] T048 [US4] Implement attachment opening logic in `src/tui/handlers/attachment_handler.rs` to populate `AttachmentViewState` with editor initialized from attachment content
- [ ] T049 [US4] Implement attachment deletion logic in `src/tui/handlers/attachment_handler.rs` to remove attachment from state and remove reference token from input field
- [ ] T050 [US4] Add Ctrl+S keybinding handler in `src/tui/handlers/attachment_handler.rs` when in AttachmentView mode to save edits back to attachment and recalculate sizes
- [ ] T051 [US4] Integrate AttachmentList and AttachmentViewer rendering into `src/tui/app_loop.rs` based on `input_mode` state
- [ ] T052 [US4] Add attachment list UI layout in `src/tui/components/attachment_list.rs` with block border, title "Attachments ({count})", and help text "Enter: View/Edit  d: Delete  Esc: Close"
- [ ] T053 [US4] Implement attachment metadata display formatting in `src/tui/components/attachment_list.rs` showing "pasted text-{id}" with size and line count

**Checkpoint**: All user stories should now be independently functional - full attachment management available

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories, edge case handling, and documentation

- [ ] T054 [P] Add edge case handling for exactly 200 character pastes in `src/tui/handlers/paste_handler.rs` (treat as inline per spec)
- [ ] T055 [P] Add edge case handling for narrow terminals (40 columns) in `src/tui/input/wrapping.rs` with minimum width checks
- [ ] T056 [P] Add Unicode/emoji width handling validation in `src/tui/input/wrapping.rs` using unicode-width crate
- [ ] T057 [P] Add terminal resize during paste handling in `src/tui/handlers/paste_handler.rs` with graceful degradation
- [ ] T058 [P] Add handling for editing attachment reference tokens in input field in `src/tui/handlers/text_input_handler.rs` (treat as regular text)
- [ ] T059 Add comprehensive error messages in `src/tui/handlers/paste_handler.rs` for all rejection cases (>5MB, binary data, etc.)
- [ ] T060 [P] Performance optimization: Add caching for wrapped text results in `src/tui/input/wrapping.rs` when text + width unchanged
- [ ] T061 [P] Add input module re-exports in `src/tui/input/mod.rs` for public API
- [ ] T062 Code cleanup: Remove any debug logging or temporary code from all input-related modules
- [ ] T063 Run `cargo clippy` and address all warnings in input-related code
- [ ] T064 Run `cargo fmt` on all modified files
- [ ] T065 Validate quickstart.md scenarios manually with actual hoosh binary

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3-6)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (US1 → US2 → US3 → US4)
- **Polish (Phase 7)**: Depends on all user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories (wrapping is independent of attachments)
- **User Story 3 (P2)**: Can start after Foundational (Phase 2) - No dependencies on other stories (expanded mode uses same TextArea infrastructure)
- **User Story 4 (P3)**: Depends on User Story 1 (attachment system must exist to manage attachments)

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Struct/enum definitions before methods
- Core logic before integration
- Rendering logic after state management
- Story complete before moving to next priority

### Parallel Opportunities

**Phase 1 (Setup)**: All tasks marked [P] can run in parallel (T002, T003, T004, T005)

**Phase 2 (Foundational)**: All tasks marked [P] can run in parallel after T006 (T007-T011)

**Phase 3 (US1)**:
- Tests T013, T014, T015 can run in parallel (different files)
- Implementation T016, T017 can run in parallel (different files, T017 uses T016's struct definition)

**Phase 4 (US2)**:
- Tests T023, T024 can run in parallel (different files)
- Implementation T025, T026 can run in parallel (T026 uses T025's struct definition)

**Phase 5 (US3)**:
- Implementation T032, T033, T034 can run in parallel (same file but different methods)

**Phase 6 (US4)**:
- Implementation T041, T042 can run in parallel (different components, different files)

**Phase 7 (Polish)**:
- All tasks marked [P] can run in parallel (different files, independent edge cases)

**User Story Level Parallelization**:
- US1 and US2 can run in parallel after Foundational (different modules: attachment vs wrapping)
- US3 can run in parallel with US1 and US2 (different module: expanded_editor)
- US4 must wait for US1 to complete (depends on attachment system)

---

## Parallel Example: User Story 1

```bash
# Launch all tests for User Story 1 together:
Task: "Unit test for PasteDetector::classify_paste() in tests/unit/paste_detector_tests.rs"
Task: "Unit test for AppState::create_attachment() in tests/unit/attachment_tests.rs"
Task: "Integration test for paste workflow in tests/integration/input_attachment_tests.rs"

# Then launch parallel implementation:
Task: "Implement PasteDetector struct in src/tui/input/paste_detector.rs"
Task: "Implement PasteDetector::classify_paste() in src/tui/input/paste_detector.rs"
```

## Parallel Example: Cross-Story (After Foundational Complete)

```bash
# Launch all P1 user stories in parallel with separate team members:
Developer A: Complete Phase 3 (User Story 1 - Paste handling)
Developer B: Complete Phase 4 (User Story 2 - Text wrapping)
Developer C: Complete Phase 5 (User Story 3 - Expanded editor)

# After US1 completes, Developer D can start:
Developer D: Complete Phase 6 (User Story 4 - Attachment management)
```

---

## Implementation Strategy

### MVP First (User Stories 1 & 2 Only - Both P1)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1 (Paste handling)
4. Complete Phase 4: User Story 2 (Text wrapping)
5. **STOP and VALIDATE**: Test US1 and US2 independently
6. Deploy/demo MVP with core paste handling and wrapping functionality

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Core paste handling works
3. Add User Story 2 → Test independently → Text wrapping works (MVP with P1 features!)
4. Add User Story 3 → Test independently → Expanded editor enhances UX
5. Add User Story 4 → Test independently → Full attachment management available
6. Polish phase → Production-ready with edge cases handled
7. Each story adds value without breaking previous stories

### Parallel Team Strategy

With 3-4 developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 (Paste handling)
   - Developer B: User Story 2 (Text wrapping)
   - Developer C: User Story 3 (Expanded editor)
3. When US1 completes:
   - Developer A moves to User Story 4 (Attachment management - depends on US1)
4. Stories complete and integrate independently
5. Team reconvenes for Polish phase

---

## Task Summary

**Total Tasks**: 65

**By Phase**:
- Phase 1 (Setup): 5 tasks
- Phase 2 (Foundational): 7 tasks
- Phase 3 (US1 - P1): 10 tasks (3 tests + 7 implementation)
- Phase 4 (US2 - P1): 9 tasks (2 tests + 7 implementation)
- Phase 5 (US3 - P2): 9 tasks (1 test + 8 implementation)
- Phase 6 (US4 - P3): 13 tasks (1 test + 12 implementation)
- Phase 7 (Polish): 12 tasks

**By User Story**:
- US1 (Paste handling): 10 tasks
- US2 (Text wrapping): 9 tasks
- US3 (Expanded editor): 9 tasks
- US4 (Attachment management): 13 tasks

**Parallel Opportunities**: 29 tasks marked [P] (45% can run in parallel within their phase)

**Independent Test Criteria**:
- US1: Paste >200 chars creates attachment with reference, paste >5MB rejected with error
- US2: Text wraps at terminal edge with ↩ indicator, rewraps on resize <100ms
- US3: Ctrl+E opens 50-60% height editor, Esc returns preserving content
- US4: Ctrl+A lists attachments with metadata, Enter edits, 'd' deletes

**MVP Scope**: Phases 1, 2, 3, 4 (User Stories 1 & 2 - both P1 features) = Core paste handling + text wrapping

---

## Notes

- [P] tasks = different files or independent modules, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Tests follow TDD: write first, ensure they fail, then implement
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Constitution compliance: All principles satisfied (tests comprehensive, trait-based design, single responsibility, flat structure, clean code)
- Performance targets: Paste classification <100ms, rewrapping <100ms, attachment ops <30s
- Edge cases handled in Polish phase to avoid blocking core functionality
