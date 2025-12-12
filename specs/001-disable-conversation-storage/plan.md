# Implementation Plan: Disable Conversation Storage

**Branch**: `001-disable-conversation-storage` | **Date**: 2025-12-11 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-disable-conversation-storage/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

Add a configuration option `disable_conversation_storage` that prevents conversation message content from being persisted to disk while allowing the application to function normally. When enabled, conversations run in ephemeral mode with no message history saved, but previously saved conversations remain accessible for reading. Metadata and logs may continue to be persisted for system debugging and monitoring.

## Technical Context

**Language/Version**: Rust 2024 edition
**Primary Dependencies**: tokio (async runtime), serde/toml (config), anyhow (errors), ratatui (TUI)
**Storage**: File-based (.hoosh/conversations/) - JSONL for messages, JSON for metadata/index
**Testing**: cargo test, tokio::test for async tests, tempfile for test fixtures
**Target Platform**: Cross-platform CLI (Linux, macOS, Windows)
**Project Type**: Single CLI application with TUI
**Performance Goals**: Instant config reads, no impact on conversation response time
**Constraints**: No breaking changes to existing config format, backward compatible
**Scale/Scope**: Single-user local application, affects config and session initialization modules

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Verify compliance with `.specify/memory/constitution.md`:

- [x] **Modularity First**: Feature adds single field to existing config module, leverages existing storage abstraction
- [x] **Explicit Error Handling**: Config loading already uses `anyhow::Result<T>`, no new error paths introduced
- [x] **Async-First Architecture**: N/A - config is sync read at startup, storage abstraction already uses `Arc<ConversationStorage>`
- [x] **Testing Discipline**: Will add behavioral tests for config parsing and conversation creation with storage disabled
- [x] **Simplicity and Clarity**: Single boolean flag, minimal changes to existing initialization flow, no new dependencies

**Violations**: None - feature integrates cleanly with existing architecture

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
src/
├── main.rs                     # CLI entry point
├── lib.rs                      # Library exports
├── session.rs                  # Session initialization (WILL MODIFY)
├── console.rs                  # Console/verbosity
│
├── config/                     # Configuration module (WILL MODIFY)
│   ├── mod.rs                  # Config structures and loading
│   ├── error.rs
│   └── mod_tests.rs            # Config tests (WILL ADD TESTS)
│
├── agent/                      # Agent and conversation logic
│   ├── mod.rs
│   ├── conversation.rs         # Conversation handling (already supports optional storage)
│   └── core.rs
│
├── storage/                    # Conversation storage (READ ONLY)
│   ├── mod.rs
│   ├── conversation.rs         # Storage implementation
│   └── index.rs
│
└── tui/                        # Terminal UI
    ├── app_loop.rs             # Main event loop (may need minor display update)
    └── app_state.rs            # Application state

tests/
└── config_tests/               # Integration tests (WILL ADD)
```

**Structure Decision**: Single project structure. This feature only modifies the config module and session initialization. The existing `Conversation` struct already supports optional storage via `Option<Arc<ConversationStorage>>`, so no changes needed to storage or agent modules.

## Complexity Tracking

N/A - No constitution violations. Feature follows all established principles.

---

## Phase 0: Research (Complete)

**Output**: [research.md](./research.md)

**Key Findings**:
- Configuration system uses TOML with two-tier override (user + project config)
- `Conversation` struct already supports optional storage via `Option<Arc<ConversationStorage>>`
- Session initialization in `src/session.rs` is the ideal injection point
- No new dependencies required
- Estimated complexity: Low (50-100 LOC)

**Decisions Made**:
- Add `conversation_storage: Option<bool>` to config structs (positive naming)
- Use `Conversation::new()` when storage disabled (`conversation_storage = false` or `None`)
- Display simple startup message: "Conversation storage disabled" when storage is off
- **Default to storage disabled** (privacy-first, `None` → `false`)

---

## Phase 1: Design (Complete)

**Artifacts Generated**:
- [data-model.md](./data-model.md) - Configuration and entity model
- [quickstart.md](./quickstart.md) - Implementation guide
- CLAUDE.md - Updated with new technologies

**Design Decisions**:

### Configuration Design
- **Field**: `conversation_storage: Option<bool>`
- **Default**: `None` (treated as `false` - **storage disabled, privacy-first**)
- **Values**:
  - `true` = Enable storage (persist conversations to disk)
  - `false` = Disable storage (ephemeral mode, no persistence)
- **Locations**:
  - **Global**: `~/.config/hoosh/config.toml` (user-wide setting)
  - **Project**: `<project_root>/.hoosh/config.toml` (project-specific override)
- **Override Behavior**: Project-level setting overrides global setting when both present
- **Validation**: Invalid values default to false (storage disabled)

**Important**: This allows users to:
- Privacy by default (storage disabled unless explicitly enabled)
- Set global enable (e.g., `conversation_storage = true` for all projects)
- Override per-project (e.g., disable storage for sensitive client work)
- Use project config to enforce ephemeral conversations

### Implementation Points
1. **Config Module** (`src/config/mod.rs`):
   - Add field to `AppConfig` and `ProjectConfig`
   - Update `merge()` method

2. **Session Initialization** (`src/session.rs`):
   - Read config flag
   - Conditionally create conversation with or without storage

3. **User Feedback** (`src/tui/app_loop.rs`):
   - Display "Conversation storage disabled" on startup when enabled

### Testing Strategy
- Config parsing tests (TOML validation)
- Conversation creation tests (file system verification)
- Integration tests (end-to-end behavior)
- Manual testing checklist

---

## Implementation Summary

### Files to Modify

| File | Changes | Lines Added | Complexity |
|------|---------|-------------|------------|
| `src/config/mod.rs` | Add config field, update merge | ~10 | Low |
| `src/session.rs` | Conditional conversation creation | ~10 | Low |
| `src/tui/app_loop.rs` | Startup message display | ~5 | Low |
| `example_config.toml` | Documentation comment | ~4 | Trivial |
| `src/config/mod_tests.rs` | Config parsing tests | ~40 | Low |
| `tests/conversation_storage_test.rs` | Integration tests (new file) | ~40 | Low |

**Total Estimated Changes**: ~110 lines

### Dependencies

**New Dependencies**: None

**Existing Dependencies Used**:
- `serde` - Config serialization
- `toml` - Config parsing
- `anyhow` - Error handling
- `tempfile` - Test fixtures

### Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Users surprised by privacy-first default | Medium | Low | Clear documentation, startup message when disabled |
| Storage still persisting when disabled | Low | Medium | Comprehensive tests, manual verification |
| TUI message interfering | Low | Low | Simple non-intrusive message |
| Config merge bug | Low | Medium | Unit tests for all merge scenarios |

**Breaking Change Note**: Default behavior is storage **disabled** (privacy-first). Users must explicitly set `conversation_storage = true` to enable persistence. This is acceptable since hoosh is pre-production.

---

## Constitution Re-Check (Post-Design)

Verify compliance after design phase:

- [x] **Modularity First**: Single config field, leverages existing abstractions, no new modules
- [x] **Explicit Error Handling**: Uses existing `anyhow::Result<T>` patterns, no new error paths
- [x] **Async-First Architecture**: N/A - config is synchronous at startup
- [x] **Testing Discipline**: Tests cover behavior (config parsing, file creation/absence), clear test names
- [x] **Simplicity and Clarity**: Minimal changes, single boolean flag, clear naming, no abstraction bloat

**Final Assessment**: ✅ All principles satisfied

---

## Next Steps

The planning phase is complete. To proceed with implementation:

1. Run `/speckit.tasks` to generate detailed implementation tasks
2. Review [quickstart.md](./quickstart.md) for step-by-step implementation guide
3. Review [data-model.md](./data-model.md) for configuration structure details
4. Reference [research.md](./research.md) for technical decisions and rationale

**Estimated Implementation Time**: 30-45 minutes for core feature + tests
