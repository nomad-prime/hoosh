# Implementation Plan: Terminal Display Modes

**Branch**: `002-terminal-modes` | **Date**: 2026-01-26 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/002-terminal-modes/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

Implement three terminal display modes (inline, fullview, tagged) to support different user workflows and terminal environments. Inline mode (current behavior) flows with terminal scrollback, fullview mode takes over the viewport with internal scrolling (fixes VSCode terminal issues), and tagged mode allows non-hijacking invocations via @hoosh alias with terminal-native output and session-based context preservation.

## Technical Context

**Language/Version**: Rust 2024 edition (matches project `Cargo.toml:4`)
**Primary Dependencies**: ratatui 0.29 (TUI), crossterm 0.27 (terminal control), tokio 1.0 (async runtime), clap 4.0 (CLI), serde/serde_json (serialization)
**Storage**: Session files in ~/.hoosh/sessions/ (JSON format, keyed by terminal PID)
**Testing**: cargo test (tokio::test for async tests)
**Target Platform**: Unix-like systems (Linux, macOS) with bash/zsh/fish shells
**Project Type**: Single binary CLI application
**Performance Goals**: Terminal resize reflow <200ms, @hoosh command return control <1s after completion, session file I/O <1ms
**Constraints**: Session file writes must be non-blocking, graceful degradation on write failures, SIGINT handling with partial context save
**Scale/Scope**: Support 3 shell types initially (bash/zsh/fish), session files auto-cleanup after 7 days, independent of conversation storage feature

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Core Principles Compliance

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Test-First Development | ✅ PASS | Unit tests for mode detection, session file I/O, shell setup; Integration tests for mode switching, TUI rendering, @hoosh invocation flow |
| II. Trait-Based Design | ✅ PASS | New `TerminalMode` trait for rendering/input handling; `SessionStore` trait for context persistence; `ShellDetector` trait for shell-specific setup |
| III. Single Responsibility | ✅ PASS | Separate modules: `tui/modes/` (mode implementations), `session/` (session file management), `cli/setup.rs` (shell alias setup) |
| IV. Flat Module Structure | ✅ PASS | No nested hierarchies; new modules at top level: `src/tui/modes/`, `src/session/`, shell setup in existing `src/cli/` |
| V. Clean Code Practices | ✅ PASS | Descriptive naming (e.g., `InlineMode`, `FullviewMode`, `TaggedMode`), error handling via `anyhow::Result`, idiomatic Rust patterns |

### Quality Gates Status

- ✅ **Test Coverage**: Unit + integration tests planned for all modes, session I/O, shell detection
- ✅ **Trait-Based Architecture**: All mode implementations behind `TerminalMode` trait
- ✅ **Error Handling**: Graceful degradation for session write failures, proper SIGINT handling
- ✅ **No Over-Engineering**: Minimal abstractions, direct implementations for each mode
- ⚠️ **NEEDS RESEARCH**: Best practices for TUI mode switching in ratatui, session file locking strategy, shell config file modification patterns

**Violations**: None

**Blockers**: None - can proceed to Phase 0 research

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
├── cli/
│   ├── mod.rs                  # MODIFY: Add mode flag parsing
│   └── setup.rs                # NEW: Shell detection & @hoosh alias setup
├── config/
│   └── mod.rs                  # MODIFY: Add terminal_mode field to AppConfig
├── session.rs                  # MODIFY: Refactor for mode-aware rendering
├── session/                    # NEW: Session file management
│   ├── mod.rs
│   ├── store.rs               # SessionStore trait & file-based impl
│   └── cleanup.rs             # Stale session cleanup logic
├── tui/
│   ├── mod.rs                 # MODIFY: Export new mode modules
│   ├── modes/                 # NEW: Terminal mode implementations
│   │   ├── mod.rs
│   │   ├── inline.rs          # InlineMode implementation
│   │   ├── fullview.rs        # FullviewMode implementation
│   │   ├── tagged.rs          # TaggedMode implementation (no TUI, terminal-native)
│   │   └── traits.rs          # TerminalMode trait definition
│   └── renderer.rs            # MODIFY: Use TerminalMode trait
└── lib.rs                     # MODIFY: Export session module

tests/
├── integration/
│   ├── mode_switching_tests.rs          # NEW: Test CLI flag -> mode selection
│   ├── fullview_rendering_tests.rs      # NEW: Test fullview scrolling, resize
│   ├── tagged_invocation_tests.rs       # NEW: Test @hoosh alias behavior
│   └── session_persistence_tests.rs     # NEW: Test session file save/load
└── unit/
    ├── session_store_tests.rs           # NEW: Test session file I/O, cleanup
    ├── shell_detection_tests.rs         # NEW: Test bash/zsh/fish detection
    └── mode_detection_tests.rs          # NEW: Test terminal env detection
```

**Structure Decision**: Single project structure maintained. New functionality added via:
1. **Mode implementations** in `src/tui/modes/` - each mode as separate module implementing `TerminalMode` trait
2. **Session management** in new `src/session/` module - handles file-based context persistence for tagged mode
3. **Shell setup** in `src/cli/setup.rs` - detects shell and modifies config files to add @hoosh alias
4. **Config extension** - add `terminal_mode` field to existing `AppConfig` struct

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No violations identified. No complexity tracking required.

---

## Planning Phases Completed

### Phase 0: Research ✅ COMPLETE

**Completed**: 2026-01-26

**Artifacts Generated**:
- ✅ `research.md` - Comprehensive research on:
  - TUI architecture exploration (existing ratatui patterns)
  - Session file locking strategy (fs2 advisory locks)
  - Shell configuration modification patterns (marker-based idempotent insertion)
  - Terminal environment detection (TERM_PROGRAM, VSCode detection)
  - Terminal-native output patterns (spinners, text prompts)

**Key Decisions**:
1. **Mode switching**: Add `TerminalMode` enum to `RuntimeState`, mode-specific layout/handler selection
2. **Session locking**: fs2 advisory locks with try-lock + graceful degradation
3. **Shell config**: Marker-based idempotent insertion with backup
4. **Fullview scrolling**: Add `ScrollState` to `AppState`, keyboard/mouse handlers
5. **Tagged mode**: Separate code path - no TUI, stdout/stderr only, text prompts

**Resolution of NEEDS CLARIFICATION**:
- ✅ Ratatui mode switching patterns → Use component trait with mode-aware layout selection
- ✅ Session file locking → fs2::FileExt with try_lock_exclusive()
- ✅ Shell config modification → Marker comments (`# >>> hoosh initialize >>>`)

---

### Phase 1: Design & Contracts ✅ COMPLETE

**Completed**: 2026-01-26

**Artifacts Generated**:
- ✅ `data-model.md` - Entity definitions:
  - `TerminalMode` enum (Inline/Fullview/Tagged)
  - `SessionFile` struct (JSON persistence)
  - `TerminalSession` struct (environment context)
  - `TerminalCapabilities` struct (feature detection)
  - `AppConfig` extensions (terminal_mode, session_context_enabled)
  - `ScrollState` struct (fullview scrolling)

- ✅ `contracts/` directory:
  - `session-file-schema.json` - JSON schema for session files
  - `config-schema.toml` - TOML schema for config fields
  - `shell-aliases.md` - Shell function templates (bash/zsh/fish)

- ✅ `quickstart.md` - User guide:
  - Mode selection guide (when to use each mode)
  - Setup instructions (shell integration)
  - Configuration examples
  - Troubleshooting tips

**Agent Context Updated**: ✅ COMPLETE
- Updated `CLAUDE.md` with new technologies:
  - Added: ratatui 0.29 (TUI)
  - Added: crossterm 0.27 (terminal control)
  - Added: Session files in ~/.hoosh/sessions/

**Constitution Re-Check**: ✅ PASS
- All principles still compliant after design
- No new violations introduced
- Trait-based design confirmed for all mode implementations

---

### Phase 2: Implementation Planning

**Status**: Ready to begin

**Next Steps**:
1. Run `/speckit.tasks` command to generate `tasks.md`
2. Tasks will be generated based on:
   - Functional requirements (FR-001 through FR-024)
   - Data model entities (6 core entities)
   - Source code structure (modifications + new files)
   - Test coverage requirements (unit + integration tests)

**Estimated Task Breakdown**:
- Core infrastructure (~8 tasks): TerminalMode enum, config extensions, session module
- Inline mode (~2 tasks): Minimal changes, preserve existing behavior
- Fullview mode (~6 tasks): ScrollState, scroll handlers, viewport management
- Tagged mode (~8 tasks): Shell setup, session persistence, terminal-native output
- Testing (~10 tasks): Unit tests, integration tests, manual validation
- Documentation (~2 tasks): README updates, example config

**Total Estimated Tasks**: ~36 tasks

---

## Notes

**Command Completion**: The `/speckit.plan` command ends after Phase 1 planning. Implementation planning (Phase 2) requires the `/speckit.tasks` command.

**Branch Status**: `002-terminal-modes` (created, ready for implementation)

**Generated Artifacts**:
- ✅ `/specs/002-terminal-modes/plan.md` (this file)
- ✅ `/specs/002-terminal-modes/research.md`
- ✅ `/specs/002-terminal-modes/data-model.md`
- ✅ `/specs/002-terminal-modes/quickstart.md`
- ✅ `/specs/002-terminal-modes/contracts/` (3 files)

**Agent Context**: ✅ Updated (`CLAUDE.md` reflects new technologies)
