# Tasks: Custom Commands

**Input**: Design documents from `specs/001-custom-commands/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/`, `tests/` at repository root
- Paths assume single Rust project structure from plan.md

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [x] T001 Add `gray_matter = "0.2"` dependency to Cargo.toml
- [x] T002 Create `src/commands/custom/` module directory
- [x] T003 [P] Create `src/commands/custom/mod.rs` with module exports
- [x] T004 [P] Update `src/commands/mod.rs` to declare `custom` submodule

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T005 [P] Create CommandMetadata struct in `src/commands/custom/metadata.rs`
- [x] T006 [P] Create Handoff struct in `src/commands/custom/metadata.rs`
- [x] T007 Implement frontmatter parser function `parse_command_file()` in `src/commands/custom/parser.rs` using gray_matter
- [x] T008 [P] Add YAML validation logic in `src/commands/custom/parser.rs` for required fields
- [x] T009 [P] Add error context to parser errors with file paths in `src/commands/custom/parser.rs`
- [x] T010 Create ParsedCommand struct in `src/commands/custom/parser.rs`
- [x] T011 Create CustomCommandWrapper struct in `src/commands/custom/wrapper.rs`
- [x] T012 Implement Command trait for CustomCommandWrapper in `src/commands/custom/wrapper.rs`

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Create Custom Command (Priority: P1) 🎯 MVP

**Goal**: Users can create custom commands by adding markdown files to `.hoosh/commands/` directory

**Independent Test**: Create `test.md` in `.hoosh/commands/`, restart Hoosh, verify `/test` command available and executes with correct body content

### Implementation for User Story 1

- [x] T013 [P] [US1] Implement `name()` method returning command name in `src/commands/custom/wrapper.rs`
- [x] T014 [P] [US1] Implement `description()` method returning metadata description in `src/commands/custom/wrapper.rs`
- [x] T015 [P] [US1] Implement `aliases()` method returning empty vec in `src/commands/custom/wrapper.rs`
- [x] T016 [P] [US1] Implement `usage()` method returning usage string in `src/commands/custom/wrapper.rs`
- [x] T017 [US1] Implement `execute()` method with argument substitution in `src/commands/custom/wrapper.rs`
- [x] T018 [US1] Add $ARGUMENTS placeholder replacement logic in `execute()` method in `src/commands/custom/wrapper.rs`
- [x] T019 [US1] Add conversation message insertion in `execute()` method in `src/commands/custom/wrapper.rs`
- [x] T020 [US1] Create CustomCommandManager struct in `src/commands/custom/manager.rs`
- [x] T021 [US1] Implement `new()` method to get commands directory path in `src/commands/custom/manager.rs`
- [x] T022 [US1] Implement `load_commands()` method to scan `.hoosh/commands/` for .md files in `src/commands/custom/manager.rs`
- [x] T023 [US1] Add error resilience to load_commands (log warnings, continue on failures) in `src/commands/custom/manager.rs`
- [x] T024 [US1] Implement `register_commands()` method in `src/commands/custom/manager.rs`
- [x] T025 [US1] Add name conflict detection (skip if built-in exists) in `register_commands()` in `src/commands/custom/manager.rs`
- [x] T026 [US1] Update `src/commands/register.rs` to call CustomCommandManager loading after built-in registration
- [x] T027 [US1] Update `src/session.rs` initialization to integrate custom command loading
- [x] T028 [US1] Add logging for custom command count at startup in `src/session.rs`

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently - users can create and execute custom commands

---

## Phase 4: User Story 2 - Auto-Create Commands Directory (Priority: P1)

**Goal**: Automatically create `.hoosh/commands/` directory on first run (zero setup)

**Independent Test**: Run Hoosh in directory without `.hoosh/commands/`, verify directory created automatically

### Implementation for User Story 2

- [x] T029 [US2] Add directory existence check in CustomCommandManager::new() in `src/commands/custom/manager.rs`
- [x] T030 [US2] Add directory creation logic with fs::create_dir_all() in `src/commands/custom/manager.rs`
- [x] T031 [US2] Add error handling for directory creation with context in `src/commands/custom/manager.rs`
- [x] T032 [US2] Add logging when directory is auto-created in `src/commands/custom/manager.rs`

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently - zero-setup custom commands

---

## Phase 5: User Story 3 - List Available Custom Commands (Priority: P2)

**Goal**: Users can discover custom commands via `/help` command

**Independent Test**: Create multiple command files, run `/help`, verify all custom commands listed with descriptions

### Implementation for User Story 3

- [ ] T033 [US3] Add `list_commands()` method to CustomCommandManager in `src/commands/custom/manager.rs`
- [ ] T034 [US3] Update `/help` command implementation in `src/commands/help_command.rs` to query custom commands
- [ ] T035 [US3] Add "Custom Commands" section to help output formatting in `src/commands/help_command.rs`
- [ ] T036 [US3] Display command name and description for each custom command in `src/commands/help_command.rs`
- [ ] T037 [US3] Handle empty custom commands case with appropriate message in `src/commands/help_command.rs`

**Checkpoint**: All priority P1 and P2 user stories are now independently functional

---

## Phase 6: User Story 4 - Command Validation and Error Handling (Priority: P3)

**Goal**: Provide clear, actionable error messages for malformed command files

**Independent Test**: Create various invalid command files (missing frontmatter, malformed YAML, empty description), verify clear error messages

### Implementation for User Story 4

- [ ] T038 [P] [US4] Add description validation (non-empty after trim) in `src/commands/custom/parser.rs`
- [ ] T039 [P] [US4] Add handoff validation (non-empty label, agent, prompt) in `src/commands/custom/parser.rs`
- [ ] T040 [US4] Add validation error messages with file paths in `src/commands/custom/parser.rs`
- [ ] T041 [US4] Add helpful error message for missing frontmatter in `src/commands/custom/parser.rs`
- [ ] T042 [US4] Add helpful error message for malformed YAML with suggestions in `src/commands/custom/parser.rs`
- [ ] T043 [US4] Enhance error reporting in load_commands to show all errors in `src/commands/custom/manager.rs`
- [ ] T044 [US4] Add file size check (1MB limit) with clear error in `src/commands/custom/parser.rs`

**Checkpoint**: All user stories should now be independently functional with comprehensive error handling

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [ ] T045 [P] Add comprehensive doc comments to public API in `src/commands/custom/mod.rs`
- [ ] T046 [P] Add example custom command files to repository in `examples/commands/analyze.md`
- [ ] T047 [P] Add example custom command files to repository in `examples/commands/review-pr.md`
- [ ] T048 Update main README.md with custom commands section and quickstart link
- [ ] T049 [P] Run `cargo clippy` and fix any warnings in custom module files
- [ ] T050 [P] Run `cargo fmt` on all custom module files
- [ ] T051 Run `cargo build --release` to verify compilation
- [ ] T052 Run `cargo test` to verify no regressions in existing tests
- [ ] T053 Manual testing: Create test command, verify end-to-end functionality
- [ ] T054 Manual testing: Verify error messages for common mistakes (missing description, malformed YAML)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3-6)**: All depend on Foundational phase completion
  - US1 (Create Custom Command): No dependencies on other stories - can start after Foundational
  - US2 (Auto-Create Directory): No dependencies on other stories - can start after Foundational (parallel with US1)
  - US3 (List Commands): Depends on US1 completion (needs CustomCommandManager::list_commands)
  - US4 (Validation): Can enhance US1 (parallel with US3, after US1)
- **Polish (Phase 7)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P1)**: Can start after Foundational (Phase 2) - Can run parallel with US1
- **User Story 3 (P2)**: Depends on US1 (needs list_commands method) - Start after US1 complete
- **User Story 4 (P3)**: Enhances US1 (validation) - Can run parallel with US3 after US1

### Within Each User Story

- User Story 1:
  - T013-T016 (wrapper methods) can run in parallel
  - T017-T019 (execute logic) sequential, depend on T013-T016
  - T020-T023 (manager) sequential
  - T024-T025 (registration) depend on T020-T023
  - T026-T028 (integration) depend on all above

- User Story 2:
  - T029-T032 all sequential in manager.rs

- User Story 3:
  - T033 must complete first
  - T034-T037 sequential in help_command.rs

- User Story 4:
  - T038-T039 can run in parallel (different validations)
  - T040-T042 sequential (error messages)
  - T043-T044 can run parallel with above

### Parallel Opportunities

- **Setup phase**: T003 and T004 can run in parallel (different files)
- **Foundational phase**: T005-T006 can run in parallel, T008-T009 can run in parallel
- **User Story 1**: T013-T016 can run in parallel (all in wrapper.rs, different methods)
- **User Story 4**: T038-T039 can run in parallel (different validation functions)
- **Polish phase**: T045-T047 can run in parallel, T049-T050 can run in parallel

---

## Parallel Example: User Story 1

```bash
# After Foundational phase completes, launch these together:
Task T013: Implement name() method
Task T014: Implement description() method
Task T015: Implement aliases() method
Task T016: Implement usage() method

# Then sequentially:
Task T017: Implement execute() method (depends on T013-T016)
Task T018: Add $ARGUMENTS replacement (extends T017)
Task T019: Add conversation insertion (extends T018)

# In parallel with execute() work, can do:
Task T020: Create CustomCommandManager struct
Task T021: Implement new() method
# ... etc
```

---

## Implementation Strategy

### MVP First (User Stories 1 + 2 Only)

**Recommended for initial release:**

1. Complete Phase 1: Setup (T001-T004)
2. Complete Phase 2: Foundational (T005-T012) - CRITICAL foundation
3. Complete Phase 3: User Story 1 (T013-T028) - Core functionality
4. Complete Phase 4: User Story 2 (T029-T032) - Zero-setup UX
5. **STOP and VALIDATE**: Test independently
   - Create test.md in .hoosh/commands
   - Restart Hoosh
   - Verify /test command works
   - Verify arguments pass through
6. Deploy/release MVP

**At this point you have a fully functional custom commands feature!**

### Incremental Delivery

**After MVP, add features incrementally:**

1. **MVP (P1 stories)**: Setup + Foundational + US1 + US2 → Deploy (Core value)
2. **Enhancement 1**: Add US3 (List Commands) → Deploy (Discoverability)
3. **Enhancement 2**: Add US4 (Validation) → Deploy (Better UX)
4. **Polish**: Phase 7 → Deploy (Production ready)

Each increment adds value without breaking previous functionality.

### Parallel Team Strategy

With multiple developers:

1. **Team completes Setup + Foundational together** (T001-T012)
2. Once Foundational is done:
   - Developer A: User Story 1 (T013-T028) - Core command execution
   - Developer B: User Story 2 (T029-T032) - Directory auto-creation
3. After US1 completes:
   - Developer C: User Story 3 (T033-T037) - Help command integration
   - Developer D: User Story 4 (T038-T044) - Enhanced validation
4. Team: Polish phase together (T045-T054)

---

## Implementation Notes

### Critical Path

```
Setup → Foundational → US1 (Create Custom Command) → Integration & Testing
```

**US1 is the blocking story** - all other stories enhance it but aren't required for core functionality.

### File Modification Order

1. **New files first** (less risk):
   - `src/commands/custom/metadata.rs`
   - `src/commands/custom/parser.rs`
   - `src/commands/custom/wrapper.rs`
   - `src/commands/custom/manager.rs`
   - `src/commands/custom/mod.rs`

2. **Existing file updates** (higher risk, do last):
   - `src/commands/mod.rs` (T004)
   - `src/commands/register.rs` (T026)
   - `src/session.rs` (T027-T028)
   - `src/commands/help_command.rs` (T034-T037)

### Testing Checkpoints

**After Foundational (Phase 2)**:
- Run `cargo build` - should compile without errors
- Run `cargo test` - existing tests should pass

**After US1 (Phase 3)**:
- Create test command file manually
- Start Hoosh, verify command loads
- Execute command, verify behavior
- Check startup logs for command count

**After US2 (Phase 4)**:
- Delete `.hoosh/commands/` directory
- Start Hoosh
- Verify directory auto-created
- Check logs for creation message

**After US3 (Phase 5)**:
- Run `/help` command
- Verify custom commands section appears
- Verify all commands listed

**After US4 (Phase 6)**:
- Create invalid command files
- Verify clear error messages
- Verify partial loading works

### Error Handling Strategy

Follow Hoosh's existing patterns (from AGENTS.md and plan.md):

```rust
// File I/O errors
fs::read_to_string(file_path)
    .with_context(|| format!("Failed to read command file: {}", file_path.display()))?

// Validation errors
if description.trim().is_empty() {
    anyhow::bail!("Command file '{}' has empty description", file_path.display());
}

// Parsing errors
parse_frontmatter(content)
    .context("Failed to parse YAML frontmatter")?
```

### Performance Targets

From plan.md success criteria:

- Command loading: <50ms for 50 commands
- Command execution: <1ms overhead
- Startup impact: <100ms total

**Optimization opportunities** (if needed):
- Parallel file loading (use `rayon` if >100 commands)
- Lazy parsing (parse on first use, not at startup)
- Command caching (already done - loaded once at startup)

---

## Notes

- All task IDs (T001-T054) are in suggested execution order
- [P] tasks can run in parallel if different files or independent logic
- [Story] labels map to user stories: US1, US2, US3, US4
- Each user story is independently testable (checkpoints after each phase)
- MVP = Phases 1-4 (Setup + Foundational + US1 + US2)
- Commit after each completed user story phase for clean git history
- Avoid: Modifying existing commands while implementing (reduces risk)
- Testing: Manual testing checkpoints provided, no automated tests in MVP scope

---

## Task Count Summary

- **Total Tasks**: 54
- **Phase 1 (Setup)**: 4 tasks
- **Phase 2 (Foundational)**: 8 tasks (CRITICAL BLOCKER)
- **Phase 3 (US1 - P1)**: 16 tasks (Core functionality)
- **Phase 4 (US2 - P1)**: 4 tasks (Zero-setup)
- **Phase 5 (US3 - P2)**: 5 tasks (Discoverability)
- **Phase 6 (US4 - P3)**: 7 tasks (Validation)
- **Phase 7 (Polish)**: 10 tasks (Cross-cutting)

**MVP Scope** (Recommended): Phases 1-4 = 32 tasks
**Full Feature**: All 54 tasks

---

## Validation Checklist

✅ All tasks follow checklist format: `- [ ] [TaskID] [P?] [Story?] Description with file path`
✅ Tasks organized by user story (US1, US2, US3, US4)
✅ Each user story has independent test criteria
✅ Dependencies clearly documented
✅ Parallel opportunities identified ([P] markers)
✅ MVP scope defined (US1 + US2)
✅ File paths included in all implementation tasks
✅ Checkpoints after each user story phase
✅ Critical path identified (Setup → Foundational → US1)
