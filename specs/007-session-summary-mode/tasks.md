# Tasks: Session Summary Mode

**Input**: Design documents from `/specs/007-session-summary-mode/`  
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅

**Organization**: Tasks grouped by user story. Each phase is independently completable and testable.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no unresolved dependencies)
- **[Story]**: Maps to user story from spec.md (US1, US2, US3)

---

## Phase 1: Setup

**Purpose**: Register the new module and create file stubs so downstream tasks can work in parallel.

- [X] T001 Add `pub mod memory_mode;` to `src/lib.rs` (one line, after existing module list)
- [X] T002 Create `src/memory_mode/mod.rs` with empty module scaffold (no logic yet)
- [X] T003 Create `src/memory_mode/tool.rs` with empty module scaffold (no logic yet)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core types and utilities that ALL user stories depend on. Must be complete before any story work begins.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T004 Implement `MemoryMode` enum in `src/memory_mode/mod.rs` — variants `Conversation` (default) and `Summary`; derive `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default`; `#[serde(rename_all = "lowercase")]`; implement `FromStr` and `Display` mirroring `TerminalMode` pattern in `src/terminal_mode.rs`
- [X] T005 [P] Add `#[serde(default)] pub memory_mode: Option<MemoryMode>` to `AppConfig` struct and to `AppConfig::default()` return value in `src/config/mod.rs`; add same field to `ProjectConfig` struct
- [X] T006 [P] Add `#[arg(long = "memory-mode", value_parser = ["conversation", "summary"])] pub memory_mode: Option<String>` to `Cli` struct in `src/cli/mod.rs`
- [X] T007 Implement `MemoryModeManager` struct in `src/memory_mode/mod.rs` — fields `conversation_id: String`, `memory_dir: PathBuf`, `last_turn_start: Arc<Mutex<Option<SystemTime>>>`; implement `new(conversation_id: &str) -> Result<Self>` (calls `fs::create_dir_all`), `summary_path(&self) -> PathBuf`, `read_summary(&self) -> Option<String>` (returns `None` on any I/O error), `summary_written_since_last_turn(&self) -> bool` (compares file mtime to stored `last_turn_start`; returns `false` if no prior turn recorded), `record_turn_end(&self, turn_start: SystemTime)` (stores `turn_start` into `last_turn_start`)
- [X] T008 [P] Add `clear_turn_history(&mut self)` method to `Conversation` impl in `src/agent/conversation.rs` — truncates `self.messages` to first 2 entries (the initial system messages) when `len() > 2`
- [X] T009 [P] Unit tests for `MemoryMode` in `src/memory_mode/mod.rs`: `memory_mode_defaults_to_conversation()`, `memory_mode_parses_summary_from_str()`, `memory_mode_parses_conversation_from_str()`, `memory_mode_invalid_str_errors()`, `memory_mode_serializes_lowercase()`
- [X] T010 [P] Unit tests for `MemoryModeManager` in `src/memory_mode/mod.rs`: `manager_creates_directory_on_new()`, `read_summary_returns_none_when_file_missing()`, `read_summary_returns_content_when_present()`, `summary_modified_since_returns_false_when_missing()`, `summary_modified_since_detects_write()`
- [X] T011 [P] Unit tests for `clear_turn_history()` in `src/agent/conversation.rs`: `clear_turn_history_preserves_first_two_system_messages()`, `clear_turn_history_removes_user_and_assistant_messages()`, `clear_turn_history_is_safe_when_fewer_than_two_messages()`

**Checkpoint**: Run `cargo test` — all new unit tests must pass before proceeding.

---

## Phase 3: User Story 1 — Enable Memory Mode Summary (Priority: P1) 🎯 MVP

**Goal**: When `--memory-mode summary` is active, the agent calls `update_session_file` at turn end, and the summary is injected as a system message at the start of the next turn (replacing cleared history). Fallback to full history if the tool is not called.

**Independent Test**: Start hoosh with `--memory-mode summary`. Complete one turn involving tool calls. Verify (a) `~/.local/share/hoosh/memory/<conv_id>/summary.txt` was created, (b) second turn begins with the summary as a system message in the conversation, (c) prior raw messages are absent from the context.

- [X] T012 [US1] Define `SUMMARY_MODE_AGENT_INSTRUCTIONS: &str` constant in `src/memory_mode/mod.rs` — instructs agent to call `update_session_file` as its last tool call each turn (before final response); summary format: **Goal** (user's overall aim, carry forward unchanged), **This turn** (files changed, decisions, errors resolved), **State** (current standing, enough to resume cold), **Next** (what remains); rules: under 800 words, no raw file contents, no verbose tool output, call exactly once per turn
- [X] T013 [US1] Implement `UpdateSessionFileTool` struct in `src/memory_mode/tool.rs` implementing `Tool` trait: `name()` → `"update_session_file"`, `display_name()` → `"UpdateSessionFile"`, `description()` referencing calling convention, `parameter_schema()` with required `summary: string` field, `execute()` reads `context.parent_conversation_id`, constructs `<data_dir>/memory/<conv_id>/summary.txt`, writes `args["summary"]` string, returns error if no conversation ID
- [X] T014 [P] [US1] Unit tests for `UpdateSessionFileTool` in `src/memory_mode/tool.rs`: `tool_name_is_update_session_file()`, `tool_writes_summary_to_correct_path()`, `tool_overwrites_existing_summary()`, `tool_returns_error_without_conversation_id()`
- [X] T015 [US1] Add `memory_mode: MemoryMode` to `SessionConfig` struct in `src/session.rs`; add `with_memory_mode(mut self, mode: MemoryMode) -> Self` builder method; when `memory_mode == Summary`, construct `MemoryModeManager::new(&conversation_id)` in `initialize_session()` and store as `Option<Arc<MemoryModeManager>>` in `RuntimeState` — constructed once per session, not per turn
- [X] T016 [US1] Depends on T015. In `initialize_session()` in `src/session.rs`: pass `Arc<MemoryModeManager>` (from T015) into `EventLoopContext` so it is accessible to `answer()` in `src/tui/actions.rs` without re-construction each turn
- [X] T017 [US1] In `handle_agent()` in `src/cli/agent.rs`: parse `--memory-mode` CLI arg (or fall back to `config.memory_mode.unwrap_or_default()`); when `memory_mode == MemoryMode::Summary`, call `tool_registry.register_tool(Arc::new(UpdateSessionFileTool)).ok()`; pass `memory_mode` to `SessionConfig` via `with_memory_mode()`
- [X] T018 [US1] In `answer()` in `src/tui/actions.rs`: before `conv.add_user_message()`, if `memory_manager.is_some()`: record `turn_start = SystemTime::now()`, check `manager.summary_written_since_last_turn()` — if true: lock conv and call `conv.clear_turn_history()`; call `manager.read_summary()`, construct combined message (`SUMMARY_MODE_AGENT_INSTRUCTIONS` + optional `## Session Memory` block), call `conv.add_system_message(content)`; after `handle_turn()` returns: call `manager.record_turn_end(turn_start)` — no warning emitted on missed writes, fallback (skip clear) is silent
- [X] T019 [US1] Apply identical injection + fallback logic to the turn loop in `src/tagged_mode.rs` (same position and same silent-fallback behavior as T018)
- [X] T020 [US1] Write integration test in `src/memory_mode/` (or `tests/`) exercising the full turn cycle: construct `MemoryModeManager` + `UpdateSessionFileTool` + mock `Conversation`, simulate a turn where tool writes summary, verify next turn starts with `clear_turn_history()` called + instructions + summary injected as system message — covers the multi-module workflow mandated by constitution Principle I

**Checkpoint**: `hoosh --memory-mode summary` — complete a multi-step turn, verify summary file written, verify next turn context is trimmed.

---

## Phase 4: User Story 2 — Reduced Token Consumption (Priority: P2)

**Goal**: Ensure the summary mechanism genuinely reduces tokens — both through the history-clearing and through agent instructions that produce concise, structured summaries.

**Independent Test**: Run equivalent 10-turn task with `--memory-mode summary` and `--memory-mode conversation`. Compare total tokens logged across turns — summary mode should show ≥30% reduction.

- [X] T021 [P] [US2] Review and confirm `SUMMARY_MODE_AGENT_INSTRUCTIONS` in `src/memory_mode/mod.rs` after first end-to-end test — verify the structure (Actions / Outcomes / Decisions / Current State) produces summaries substantially shorter than the raw turn history; refine wording if not; add a brief inline example to the constant's doc comment
- [X] T022 [P] [US2] Add `console().debug()` log line in `src/tui/actions.rs` after `clear_turn_history()`: log number of messages cleared (e.g., `"Memory mode: cleared N messages from prior turn"`) at debug verbosity
- [X] T023 [P] [US2] Add same debug log line to `src/tagged_mode.rs` after `clear_turn_history()`

**Checkpoint**: Run with `-vv --memory-mode summary` across multiple turns; confirm cleared-message count is logged and grows with conversation length.

---

## Phase 5: User Story 3 — Standard Mode Unaffected (Priority: P3)

**Goal**: Confirm `--memory-mode conversation` (the default) produces exactly the existing behavior with zero regression.

**Independent Test**: Run existing test suite (`cargo test`) without specifying `--memory-mode`; all pre-existing tests must pass. Run hoosh without any memory-mode flag and confirm full history is retained across turns.

- [X] T024 [US3] Unit test in `src/memory_mode/mod.rs`: `memory_mode_conversation_is_default()` — asserts `MemoryMode::default() == MemoryMode::Conversation`
- [X] T025 [P] [US3] Unit test alongside `src/tui/actions.rs`: `answer_with_conversation_mode_skips_injection()` — `memory_manager == None` path verifies `clear_turn_history()` is never called and no system message is prepended
- [X] T026 [P] [US3] Unit tests distinguishing FR-007 from FR-008 in `src/memory_mode/mod.rs`: `injection_skipped_silently_on_first_turn()` (no summary file, no warning emitted) and `injection_falls_back_with_warning_on_corrupt_file()` (file exists but unreadable, warning logged)
- [ ] T027 [US3] Add manual validation task for SC-002: after US1 is complete, run equivalent 3-turn task in both modes and confirm agent responses are correct in summary mode — document result as a comment in `specs/007-session-summary-mode/quickstart.md`
- [X] T028 [US3] Run `cargo test` — all pre-existing tests must pass without modification

**Checkpoint**: Zero test failures. `hoosh` with no flags behaves identically to pre-feature behavior.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T029 [P] Run `cargo clippy` on all modified and new files; fix any warnings in `src/memory_mode/`, `src/config/mod.rs`, `src/cli/mod.rs`, `src/cli/agent.rs`, `src/session.rs`, `src/tui/actions.rs`, `src/tagged_mode.rs`, `src/agent/conversation.rs`
- [X] T030 [P] Run `cargo fmt` on all new and modified files
- [ ] T031 Validate quickstart.md steps end-to-end: enable via CLI flag, enable via config, resume with `--continue`, verify summary file location, check debug logging with `-vv`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — **blocks all user story phases**
- **Phase 3 (US1)**: Depends on Phase 2 completion
- **Phase 4 (US2)**: Depends on Phase 3 completion (US1 must be working)
- **Phase 5 (US3)**: Depends on Phase 2 completion; runs best after Phase 3 is done
- **Phase 6 (Polish)**: Depends on all user story phases

### User Story Dependencies

- **US1 (P1)**: Requires Foundational complete. Core mechanism — US2 and US3 depend on it.
- **US2 (P2)**: Requires US1 complete. Refines prompt + adds observability.
- **US3 (P3)**: Requires Foundational complete. Regression verification — can run after US1.

### Within Each Phase

- T004 → T007 (MemoryModeManager uses MemoryMode)
- T012 → T018, T019 (instructions constant must exist before injection block is written)
- T013 → T017 (tool must exist before it can be registered)
- T015 → T016 (MemoryModeManager stored in RuntimeState before it can be threaded to EventLoopContext)
- T015 → T017 (SessionConfig field must exist before handle_agent passes it)
- T015, T016, T017 → T018, T019 (all wiring complete before injection logic)
- T018, T019 → T020 (integration test requires injection logic to exist)

### Parallel Opportunities

**Phase 2**: T005, T006, T008, T009, T010, T011 can all run in parallel after T004 is done.  
**Phase 3**: T014 can run in parallel with T015. Once T013, T015, T016, T017 done → T018 and T019 in parallel → T020.  
**Phase 4**: T021, T022, T023 can all run in parallel.  
**Phase 5**: T024, T025, T026 can all run in parallel after US1 complete; T027 after T025.

---

## Parallel Example: Phase 2

```
T004 first (MemoryMode enum must exist)
Then launch simultaneously:
  → T005: config field (src/config/mod.rs)
  → T006: CLI arg (src/cli/mod.rs)
  → T008: clear_turn_history() (src/agent/conversation.rs)
  → T009: MemoryMode tests (src/memory_mode/mod.rs)
  → T011: clear_turn_history() tests (src/agent/conversation.rs)
T007 after T004 (MemoryModeManager in same file as enum)
T010 after T007 (MemoryModeManager tests depend on its implementation)
```

## Parallel Example: Phase 3 (US1)

```
T012 first (constant needed by T016)
T013 + T015 in parallel (tool and SessionConfig are independent files)
T014 in parallel with T015, T016
After T012, T013, T015, T016, T017 complete:
  → T018: TUI injection (src/tui/actions.rs)
  → T019: Tagged mode injection (src/tagged_mode.rs)
T014 after T013 (tool must exist to be registered)
```

---

## Implementation Strategy

### MVP (User Story 1 only)

1. Phase 1: Setup (T001–T003)
2. Phase 2: Foundational (T004–T011)
3. Phase 3: US1 (T012–T019)
4. **STOP and validate**: `hoosh --memory-mode summary` works end-to-end
5. Polish (T025–T027)

### Full Incremental Delivery

1. Setup + Foundational → types and utilities ready
2. US1 → core mechanism working → validate
3. US2 → prompt quality + observability → validate token reduction
4. US3 → regression confirmation → `cargo test` passes
5. Polish → ship

### Task Count Summary

| Phase | Tasks | Notes |
|-------|-------|-------|
| Phase 1: Setup | 3 | T001–T003 |
| Phase 2: Foundational | 8 | T004–T011 |
| Phase 3: US1 | 9 | T012–T020 (incl. integration test) |
| Phase 4: US2 | 3 | T021–T023 |
| Phase 5: US3 | 5 | T024–T028 (incl. SC-002 validation) |
| Phase 6: Polish | 3 | T029–T031 |
| **Total** | **31** | |

---

## Notes

- Tests are included because the constitution mandates unit tests for all business logic
- `[P]` tasks touch different files with no unresolved dependencies — safe to parallelize
- Each user story phase ends with a concrete checkpoint verification step
- Commit after each phase or at each checkpoint
- The `--memory-mode conversation` default means existing behavior is unchanged unless the flag is explicitly passed
