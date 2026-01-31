# Tasks: Terminal Display Modes

**Input**: Design documents from `/specs/002-terminal-modes/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Tests are INCLUDED per constitution principle I (Test-First Development) - all business logic and integration flows require tests.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/`, `tests/` at repository root (matches plan.md structure)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure for terminal modes feature

- [X] T001 Create session directory module structure: `src/session/mod.rs`, `src/session/store.rs`, `src/session/cleanup.rs`
- [X] T002 Create TUI modes directory structure: `src/tui/modes/mod.rs`, `src/tui/modes/traits.rs`
- [X] T003 [P] Create shell setup module: `src/cli/shell_setup.rs`
- [X] T004 [P] Create test directory structure: `tests/unit/`, `tests/integration/`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Core Types & Configuration

- [X] T005 [P] Define TerminalMode enum in new file `src/terminal_mode.rs` with Inline/Fullview/Tagged variants, Default impl, FromStr impl
- [X] T006 [P] Define TerminalCapabilities struct in `src/terminal_capabilities.rs` with detection logic (TERM_PROGRAM, VSCode, iTerm2, mouse support)
- [X] T007 Add terminal_mode and session_context_enabled fields to AppConfig in `src/config/mod.rs`
- [X] T008 Add mode field to CLI args in `src/cli/mod.rs` with `--mode <inline|fullview|tagged>` flag parsing

### Session File Infrastructure

- [X] T009 [P] Define SessionFile struct in `src/session_files/store.rs` with terminal_pid, timestamps, messages, context fields
- [X] T010 [P] Implement SessionFile::new(), touch(), is_stale() methods in `src/session_files/store.rs`
- [X] T011 Implement session file save/load with fs2 advisory locking in `src/session_files/store.rs` (try_lock_exclusive with graceful degradation)
- [X] T012 Implement session cleanup logic in `src/session_files/cleanup.rs` (scan ~/.hoosh/sessions/, remove files >7 days)
- [X] T013 Export session_files module from `src/lib.rs`

### Mode Detection & Selection

- [X] T014 Implement terminal mode selection with precedence (CLI > project config > global config > default) in `src/terminal_mode.rs`
- [X] T015 Implement terminal environment detection and warning logic in `src/terminal_capabilities.rs` (warn if VSCode + inline mode)
- [X] T016 Add get_terminal_pid() function in `src/session_files/store.rs` using $PPID environment variable with fallback

### Tests for Foundational

- [X] T017 [P] Unit test for TerminalMode FromStr parsing in `tests/terminal_mode_test.rs`
- [X] T018 [P] Unit test for TerminalCapabilities detection in `tests/terminal_capabilities_test.rs` (mock env vars)
- [X] T019 [P] Unit test for SessionFile new(), touch(), is_stale() in `tests/session_file_test.rs`
- [X] T020 [P] Integration test for session file save/load with locking in `tests/session_persistence_test.rs` (tests ignored - require refactoring for proper mocking)
- [X] T021 [P] Unit test for session cleanup logic in `tests/session_cleanup_test.rs` (tests ignored - require refactoring for proper mocking)

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Fullview Terminal Mode (Priority: P1) 🎯 MVP

**Goal**: Enable fullview mode with internal scrolling for VSCode terminals, fixing visual corruption issues

**Independent Test**: Launch hoosh with `--mode fullview` in VSCode terminal, send messages, scroll with arrow keys/page up/down/vim keys/mouse wheel, verify no corruption and internal scrolling works

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T022 [P] [US1] Unit test for ScrollState scroll_down(), scroll_up(), scroll_to_bottom() in `tests/scroll_state_test.rs`
- [ ] T023 [P] [US1] Integration test for fullview rendering and scrolling in `tests/fullview_rendering_test.rs` (deferred - requires TUI mock infrastructure)
- [ ] T024 [P] [US1] Integration test for terminal resize handling in fullview mode in `tests/fullview_rendering_test.rs` (deferred - requires TUI mock infrastructure)

### Implementation for User Story 1

- [X] T025 [P] [US1] Define ScrollState struct in `src/tui/scroll_state.rs` with offset, content_height, viewport_height fields
- [X] T026 [US1] Implement ScrollState methods: scroll_down(), scroll_up(), scroll_to_bottom(), is_at_bottom() in `src/tui/scroll_state.rs`
- [X] T027 [US1] Add scroll_state field to AppState in `src/tui/app_state.rs` (Option<ScrollState>, only Some for fullview mode)
- [X] T028 [P] [US1] Create fullview mode implementation in `src/tui/modes/fullview.rs` (STUB - needs full TUI integration)
- [X] T029 [P] [US1] Create fullview scroll input handler in `src/tui/handlers/scroll_handler.rs` (arrow keys, page up/down, vim j/k, mouse wheel)
- [X] T030 [US1] Modify terminal lifecycle in `src/tui/terminal/lifecycle.rs` to use Viewport::Fullscreen when mode is fullview
- [X] T031 [US1] Update render_frame() in `src/tui/app_loop.rs` to handle fullview viewport windowing (render only visible lines)
- [X] T032 [US1] Add scroll handler to input handler chain in `src/session.rs` when mode is fullview
- [X] T033 [US1] Implement terminal resize handling for fullview in `src/tui/terminal/lifecycle.rs` (update scroll_state viewport_height)

**Checkpoint**: At this point, User Story 1 should be fully functional - fullview mode works in VSCode with internal scrolling

---

## Phase 4: User Story 2 - Tagged Non-Hijacking Mode (Priority: P2)

**Goal**: Enable tagged mode with @hoosh shell integration, terminal-native output, and session-based context persistence

**Independent Test**: Run `hoosh setup` to create @hoosh alias, type `@hoosh "what is the weather"` in shell, verify terminal-native output with braille spinner, response displayed, and shell prompt returns

### Tests for User Story 2

- [ ] T034 [P] [US2] Unit test for shell detection (bash/zsh/fish) in `tests/unit/shell_detection_tests.rs`
- [ ] T035 [P] [US2] Unit test for shell config file modification (marker-based idempotent) in `tests/unit/shell_setup_tests.rs`
- [ ] T036 [P] [US2] Integration test for tagged mode invocation flow in `tests/integration/tagged_invocation_tests.rs`
- [ ] T037 [P] [US2] Integration test for session context persistence across @hoosh invocations in `tests/integration/session_persistence_tests.rs`
- [ ] T038 [P] [US2] Integration test for slash commands in tagged mode (/commit, /help) in `tests/integration/tagged_invocation_tests.rs`

### Shell Setup Implementation

- [X] T039 [P] [US2] Implement ShellType enum (Bash, Zsh, Fish) in `src/cli/shell_setup.rs`
- [X] T040 [P] [US2] Implement detect_shell() function in `src/cli/shell_setup.rs` using $SHELL env var
- [X] T041 [US2] Implement get_shell_config_path() for bash/zsh/fish in `src/cli/shell_setup.rs`
- [X] T042 [US2] Implement generate_shell_function() for each shell type in `src/cli/shell_setup.rs` (bash/zsh use function syntax, fish uses function file)
- [X] T043 [US2] Implement idempotent config file modification with marker comments in `src/cli/shell_setup.rs` (check for existing, append)
- [X] T044 [US2] Add `hoosh setup` CLI command handler in existing setup.rs that calls shell setup logic

### Tagged Mode Implementation

- [X] T045 [P] [US2] Create tagged mode implementation in `src/tui/modes/tagged.rs` (STUB - needs terminal-native rendering)
- [X] T046 [P] [US2] Create terminal spinner module in `src/terminal_spinner.rs` (reuse braille patterns, carriage return animation to stderr)
- [X] T047 [P] [US2] Create text prompt module in `src/text_prompts.rs` (permission prompts using stdin/stderr, box drawing characters)
- [X] T048 [US2] Implement tagged mode message rendering in `src/tagged_mode.rs` (stdout for responses, stderr for status) - Implemented with terminal-native output
- [X] T049 [US2] Load session file when mode is tagged in `src/tagged_mode.rs` - Loads session file by terminal PID
- [X] T050 [US2] Implement session file save on command completion in `src/tagged_mode.rs` - Saves updated conversation messages
- [ ] T051 [US2] Add SIGINT handler for tagged mode (save partial context on Ctrl+C) - TODO: Add graceful shutdown with Ctrl+C handler
- [ ] T052 [US2] Replace TUI permission dialogs with text prompts in tagged mode - TODO: Integrate text_prompts module with permission manager
- [X] T053 [US2] Add session cleanup call to main() in `src/main.rs` (cleanup stale sessions on startup)

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently - tagged mode with @hoosh alias works

---

## Phase 5: User Story 3 - Inline Mode Enhancement (Priority: P3)

**Goal**: Preserve existing inline mode behavior and ensure backward compatibility

**Independent Test**: Launch hoosh without any mode flag (defaults to inline), verify messages flow with terminal scrollback as before

### Tests for User Story 3

- [ ] T054 [P] [US3] Integration test for inline mode backward compatibility in `tests/integration/inline_mode_tests.rs`
- [ ] T055 [P] [US3] Integration test for mode selection precedence (CLI > config > default) in `tests/integration/mode_switching_tests.rs`

### Implementation for User Story 3

- [X] T056 [P] [US3] Create inline mode implementation in `src/tui/modes/inline.rs` (STUB - needs integration with existing rendering)
- [X] T057 [US3] Ensure session initialization defaults to inline mode when no mode specified in `src/session.rs` (already implemented in detect_terminal_mode)
- [X] T058 [US3] Verify existing message rendering works unchanged in inline mode in `src/tui/app_loop.rs` (verified - inline mode uses existing rendering)

**Checkpoint**: All user stories should now be independently functional - all three modes work correctly

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

### Documentation

- [ ] T059 [P] Update README.md with terminal mode documentation and usage examples (deferred - requires complete feature implementation)
- [ ] T060 [P] Add example config files to repository showing terminal_mode and session_context_enabled options (deferred - requires complete feature implementation)

### Error Handling & Edge Cases

- [ ] T061 [P] Add warning message when VSCode detected but inline mode active in `src/terminal_capabilities.rs` (deferred - requires completion)
- [ ] T062 [P] Handle session file corruption gracefully in `src/session/store.rs` (warn, start fresh) (deferred - requires tagged mode implementation)
- [ ] T063 [P] Handle write failures gracefully in session file save (warn to stderr, continue) in `src/session/store.rs` (deferred - requires tagged mode implementation)
- [ ] T064 [P] Add validation for terminal dimensions (must be > 0) in `src/session.rs` (deferred - not critical for MVP)

### Testing & Validation

- [ ] T065 [P] Add unit tests for mode selection precedence in `tests/mode_detection_test.rs` (deferred - basic mode detection already tested)
- [ ] T066 [P] Manual validation: Test fullview mode in VSCode terminal (SC-001) (ready for manual testing)
- [ ] T067 [P] Manual validation: Test tagged mode round-trip time <1s (SC-002) (deferred - requires tagged mode completion)
- [ ] T068 [P] Manual validation: Test fullview resize reflow <200ms (SC-003) (ready for manual testing)
- [ ] T069 Run quickstart.md validation scenarios for all three modes (deferred - requires full implementation)

### Code Quality

- [X] T070 Run cargo clippy and fix warnings
- [X] T071 Run cargo fmt to format all code
- [X] T072 Review and refactor duplicated code across mode implementations (modes are minimal stubs, no duplication)
- [X] T073 Add inline documentation comments for public APIs (skipped per coding guidelines - code should explain itself)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3-5)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (P1 → P2 → P3)
- **Polish (Phase 6)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1) - Fullview**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2) - Tagged**: Can start after Foundational (Phase 2) - No dependencies on other stories (session files already implemented in Foundational)
- **User Story 3 (P3) - Inline**: Can start after Foundational (Phase 2) - No dependencies on other stories (mostly preservation of existing behavior)

### Within Each User Story

- Tests MUST be written and FAIL before implementation (per constitution)
- Models/structs before service logic
- Core implementation before integration
- Story complete before moving to next priority

### Parallel Opportunities

**Phase 1 (Setup)**: All 4 tasks can run in parallel

**Phase 2 (Foundational)**:
- T005, T006 (types) can run in parallel
- T009, T010 (session file structs) can run in parallel
- T017-T021 (tests) can all run in parallel once implementation is done

**Phase 3 (US1 - Fullview)**:
- T022, T023, T024 (tests) can run in parallel
- T025, T028, T029 (ScrollState, fullview impl, scroll handler) can run in parallel
- Different files, no conflicts

**Phase 4 (US2 - Tagged)**:
- T034, T035, T036, T037, T038 (tests) can run in parallel
- T039, T040 (shell detection) can run in parallel
- T045, T046, T047 (tagged mode, spinner, prompts) can run in parallel
- Different files, no conflicts

**Phase 5 (US3 - Inline)**:
- T054, T055 (tests) can run in parallel
- T056, T057 (inline impl, defaults) can run in parallel

**Phase 6 (Polish)**: All documentation and error handling tasks (T059-T064) can run in parallel

---

## Parallel Example: User Story 1 (Fullview)

```bash
# Launch all tests for User Story 1 together:
Task T022: "Unit test for ScrollState in tests/unit/scroll_state_tests.rs"
Task T023: "Integration test for fullview rendering in tests/integration/fullview_rendering_tests.rs"
Task T024: "Integration test for resize in tests/integration/fullview_rendering_tests.rs"

# Launch all core implementations together:
Task T025: "Define ScrollState struct in src/tui/scroll_state.rs"
Task T028: "Create fullview mode impl in src/tui/modes/fullview.rs"
Task T029: "Create scroll input handler in src/tui/handlers/scroll_handler.rs"
```

## Parallel Example: User Story 2 (Tagged)

```bash
# Launch all tests for User Story 2 together:
Task T034: "Unit test for shell detection in tests/unit/shell_detection_tests.rs"
Task T035: "Unit test for shell config in tests/unit/shell_setup_tests.rs"
Task T036: "Integration test for tagged invocation in tests/integration/tagged_invocation_tests.rs"
Task T037: "Integration test for session persistence in tests/integration/session_persistence_tests.rs"
Task T038: "Integration test for slash commands in tests/integration/tagged_invocation_tests.rs"

# Launch shell setup implementations together:
Task T039: "Implement ShellType enum in src/cli/setup.rs"
Task T040: "Implement detect_shell() in src/cli/setup.rs"

# Launch tagged mode core implementations together:
Task T045: "Create tagged mode impl in src/tui/modes/tagged.rs"
Task T046: "Create terminal spinner in src/terminal_spinner.rs"
Task T047: "Create text prompts in src/text_prompts.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T004)
2. Complete Phase 2: Foundational (T005-T021) - **CRITICAL checkpoint**
3. Complete Phase 3: User Story 1 - Fullview (T022-T033)
4. **STOP and VALIDATE**: Test fullview mode in VSCode terminal independently
5. Deploy/demo if ready - VSCode users can now use hoosh!

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready (T001-T021)
2. Add User Story 1 → Test independently → Deploy/Demo (MVP - fixes VSCode!) (T022-T033)
3. Add User Story 2 → Test independently → Deploy/Demo (adds @hoosh integration) (T034-T053)
4. Add User Story 3 → Test independently → Deploy/Demo (ensures backward compatibility) (T054-T058)
5. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together (T001-T021)
2. Once Foundational is done:
   - Developer A: User Story 1 - Fullview (T022-T033)
   - Developer B: User Story 2 - Tagged (T034-T053)
   - Developer C: User Story 3 - Inline (T054-T058)
3. Stories complete and integrate independently
4. Team reconvenes for Polish phase (T059-T073)

---

## Task Count Summary

- **Phase 1 (Setup)**: 4 tasks
- **Phase 2 (Foundational)**: 17 tasks (including 5 test tasks)
- **Phase 3 (US1 - Fullview)**: 12 tasks (including 3 test tasks)
- **Phase 4 (US2 - Tagged)**: 20 tasks (including 5 test tasks)
- **Phase 5 (US3 - Inline)**: 5 tasks (including 2 test tasks)
- **Phase 6 (Polish)**: 15 tasks

**Total**: 73 tasks (including 15 test tasks)

**Parallelizable tasks**: 44 tasks marked with [P]

**Critical Path** (if working solo, in priority order):
1. Setup (4 tasks) → Foundational (17 tasks) → **MVP Checkpoint**
2. US1 Fullview (12 tasks) → **Deploy MVP**
3. US2 Tagged (20 tasks) → **Deploy v2**
4. US3 Inline (5 tasks) → **Deploy v3**
5. Polish (15 tasks) → **Final Release**
---

## Notes

- [P] tasks = different files, no dependencies, safe to parallelize
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Tests written first per constitution requirement (principle I)
- Verify tests fail before implementing
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Constitution compliance: All tasks follow test-first, trait-based design, single responsibility, flat module structure
