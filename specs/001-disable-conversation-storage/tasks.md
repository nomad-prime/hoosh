# Tasks: Disable Conversation Storage

**Input**: Design documents from `/specs/001-disable-conversation-storage/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, quickstart.md

**Tests**: Included as specified in plan.md testing strategy

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/`, `tests/` at repository root
- Paths use absolute references from repository root

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization - validate existing structure

- [X] T001 Verify Rust 2024 edition in Cargo.toml
- [X] T002 [P] Verify existing dependencies (serde, toml, anyhow, tokio, tempfile)
- [X] T003 [P] Run `cargo test` to establish baseline (all tests should pass)
- [X] T004 [P] Run `cargo clippy` to establish code quality baseline

**Checkpoint**: Development environment validated

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core configuration infrastructure - MUST be complete before user stories

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T005 Add `conversation_storage: Option<bool>` field to `AppConfig` struct in src/config/mod.rs
- [X] T006 Add `conversation_storage: Option<bool>` field to `ProjectConfig` struct in src/config/mod.rs
- [X] T007 Update `AppConfig::merge()` method to handle `conversation_storage` override in src/config/mod.rs
- [X] T008 Verify `cargo build` succeeds with new config fields

**Checkpoint**: Foundation ready - user story implementation can now begin

---

## Phase 3: User Story 1 - Disable Storage via Configuration (Priority: P1) 🎯 MVP

**Goal**: Implement core functionality to prevent conversation persistence when `conversation_storage` is false/None, with privacy-first default

**Independent Test**: Set `conversation_storage = false` (or omit field), run conversation, verify zero files created in `.hoosh/conversations/`. Set `conversation_storage = true`, run conversation, verify files created.

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T009 [P] [US1] Create config parsing test `test_conversation_storage_true_enables_persistence` in src/config/mod_tests.rs
- [X] T010 [P] [US1] Create config parsing test `test_conversation_storage_false_disables_persistence` in src/config/mod_tests.rs
- [X] T011 [P] [US1] Create config parsing test `test_conversation_storage_missing_defaults_to_none` in src/config/mod_tests.rs
- [X] T012 [P] [US1] Create config merge test `test_project_config_overrides_user_config` in src/config/mod_tests.rs
- [X] T013 [P] [US1] Create integration test file tests/conversation_storage_test.rs
- [X] T014 [P] [US1] Write test `test_conversation_without_storage_creates_no_files` in tests/conversation_storage_test.rs
- [X] T015 [P] [US1] Write test `test_conversation_with_storage_creates_files` in tests/conversation_storage_test.rs
- [X] T016 [P] [US1] Write test `test_messages_not_persisted_when_disabled` in tests/conversation_storage_test.rs
- [X] T017 [US1] Run `cargo test` and verify all new tests FAIL as expected

### Implementation for User Story 1

- [X] T018 [US1] Modify session initialization in src/session.rs: read `conversation_storage` config value
- [X] T019 [US1] Implement conditional logic in src/session.rs: `if conversation_storage.unwrap_or(false) { with_storage } else { new }`
- [X] T020 [US1] Update conversation creation to use `Conversation::new()` when storage disabled in src/session.rs
- [X] T021 [US1] Update conversation creation to use `Conversation::with_storage()` when storage enabled in src/session.rs
- [X] T022 [US1] Run `cargo test` and verify all US1 tests now PASS
- [X] T023 [US1] Run `cargo clippy` and address any warnings

**Checkpoint**: User Story 1 complete and independently testable. At this point:
- Config parsing works correctly
- Storage disabled creates zero files
- Storage enabled creates files normally
- Project config overrides global config
- Privacy-first default (None → false → disabled) works

---

## Phase 4: User Story 2 - Clear Indication of Storage Status (Priority: P2)

**Goal**: Display startup message "Conversation storage disabled" when storage is off, providing user feedback

**Independent Test**: Run app with `conversation_storage = false`, verify message appears. Run with `conversation_storage = true`, verify no message.

### Implementation for User Story 2

- [X] T024 [US2] Locate TUI initialization code in src/tui/app_loop.rs (or equivalent startup location)
- [X] T025 [US2] Add conditional check: `if !config.conversation_storage.unwrap_or(false)`
- [X] T026 [US2] Implement message display: `console.info("Conversation storage disabled")` or equivalent
- [X] T027 [US2] Manual test: Run app with storage disabled, verify message appears
- [X] T028 [US2] Manual test: Run app with storage enabled, verify NO message appears
- [X] T029 [US2] Manual test: Run app with field omitted (default), verify message appears (privacy-first)

**Checkpoint**: User Story 2 complete. User now receives clear feedback about storage state.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, validation, and final touches

- [X] T030 [P] Update example_config.toml with conversation_storage documentation and use cases
- [X] T031 [P] Document privacy-first default in example_config.toml comments
- [X] T032 [P] Add global vs project config usage examples to example_config.toml
- [X] T033 [P] Create or update README section explaining conversation_storage feature
- [X] T034 Run complete manual test: storage disabled (verify no files, message shown)
- [X] T035 Run complete manual test: storage enabled (verify files created, no message)
- [X] T036 Run complete manual test: project override (global=false, project=true, verify enabled)
- [X] T037 Run complete manual test: project override (global=true, project=false, verify disabled)
- [X] T038 Verify old conversations still accessible when storage disabled
- [X] T039 Verify config changes ignored mid-session (restart required)
- [X] T040 [P] Final `cargo test` - all tests pass
- [X] T041 [P] Final `cargo clippy` - no warnings
- [X] T042 [P] Final `cargo build --release` - successful build
- [X] T043 Review quickstart.md validation checklist and verify all items pass

**Checkpoint**: Feature complete, tested, and documented

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User Story 1 (P1): Can start after Foundational
  - User Story 2 (P2): Can start after Foundational (independent of US1)
- **Polish (Phase 5)**: Depends on both user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - INDEPENDENT of US1, but logically follows it

**Note**: While US2 is independent, it makes more sense to complete US1 first so the feature actually works before adding the visual feedback.

### Within Each User Story

**User Story 1**:
1. Write all tests first (T009-T017) - tests MUST FAIL
2. Tests can run in parallel (all marked [P])
3. Implement config fields (sequential, modifies same file)
4. Run tests - should now PASS
5. Story complete

**User Story 2**:
1. Implementation tasks sequential (same file modifications)
2. Manual testing after implementation
3. Story complete

### Parallel Opportunities

- **Phase 1**: T002, T003, T004 can run in parallel
- **Phase 3 Tests**: T009-T016 can all run in parallel (different test files/functions)
- **Phase 5**: T030, T031, T032, T033 can run in parallel (different files)
- **Phase 5**: T040, T041, T042 can run in parallel (independent validation commands)

---

## Parallel Example: User Story 1 Tests

```bash
# Launch all config parsing tests together:
Task: "test_conversation_storage_true_enables_persistence in src/config/mod_tests.rs"
Task: "test_conversation_storage_false_disables_persistence in src/config/mod_tests.rs"
Task: "test_conversation_storage_missing_defaults_to_none in src/config/mod_tests.rs"
Task: "test_project_config_overrides_user_config in src/config/mod_tests.rs"

# Launch all integration tests together:
Task: "test_conversation_without_storage_creates_no_files in tests/conversation_storage_test.rs"
Task: "test_conversation_with_storage_creates_files in tests/conversation_storage_test.rs"
Task: "test_messages_not_persisted_when_disabled in tests/conversation_storage_test.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T004)
2. Complete Phase 2: Foundational (T005-T008) - CRITICAL
3. Complete Phase 3: User Story 1 (T009-T023)
4. **STOP and VALIDATE**: Run all tests, verify storage can be disabled
5. **MVP READY**: Core feature functional

### Full Feature (Both User Stories)

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Core feature works
3. Add User Story 2 → Test independently → User feedback added
4. Polish → Documentation and final validation
5. **FEATURE COMPLETE**

### Parallel Team Strategy

With 2 developers:

1. Both complete Setup + Foundational together (T001-T008)
2. Developer A: User Story 1 tests (T009-T017) while Developer B: Prepares test fixtures
3. Both review test failures together
4. Developer A: User Story 1 implementation (T018-T023)
5. Developer B: User Story 2 implementation (T024-T029) once US1 tests pass
6. Both: Polish tasks in parallel (T030-T043)

---

## Task Summary

**Total Tasks**: 43

**By Phase**:
- Phase 1 (Setup): 4 tasks
- Phase 2 (Foundational): 4 tasks
- Phase 3 (User Story 1): 15 tasks (9 tests + 6 implementation)
- Phase 4 (User Story 2): 6 tasks
- Phase 5 (Polish): 14 tasks

**By User Story**:
- User Story 1 (P1 - MVP): 15 tasks
- User Story 2 (P2): 6 tasks
- Infrastructure/Polish: 22 tasks

**Parallelizable Tasks**: 18 tasks marked [P]

**Estimated Time**:
- MVP (Setup + Foundational + US1): 45-60 minutes
- Full Feature (+ US2 + Polish): 90-120 minutes
- With parallel execution: 60-90 minutes

---

## Notes

- [P] tasks = different files, no dependencies - can run in parallel
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- Tests written FIRST and must FAIL before implementation
- Privacy-first default: `None` → `false` → storage disabled
- Config hierarchy: project config overrides global config
- Breaking change acceptable: hoosh is pre-production
- Commit after each logical group or checkpoint
- Stop at any checkpoint to validate story independently
