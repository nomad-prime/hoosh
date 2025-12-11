# Implementation Plan: Input Field Refinement

**Branch**: `003-input-field-refinement` | **Date**: 2025-12-11 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/003-input-field-refinement/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

Refine the terminal input field to handle large paste operations gracefully by storing them as attachments, implement automatic line wrapping to respect terminal width, and provide an expanded text editor mode for comfortable multi-line editing. Technical approach uses existing tui-textarea widget with custom wrapper for width-aware wrapping, attachment storage in AppState, and modal editor view.

## Technical Context

**Language/Version**: Rust 2024 edition (cargo.toml:4)
**Primary Dependencies**:
- ratatui 0.29.0 (TUI framework with unstable features enabled)
- tui-textarea 0.4.0 (multi-line text input widget)
- crossterm 0.27.0 (terminal event handling)
- arboard 3.4 (clipboard operations)
- textwrap 0.16 (text wrapping utilities - already in deps)

**Storage**: In-memory (AppState struct) - no persistence needed
**Testing**: cargo test with unit and integration tests
**Target Platform**: Cross-platform terminal (Linux, macOS, Windows) via crossterm
**Project Type**: Single binary CLI application with TUI interface
**Performance Goals**:
- 60fps rendering (16ms frame budget)
- Instant mode transitions (<100ms)
- Handle 1MB attachments without lag
- Text wrapping calculations <5ms per frame

**Constraints**:
- Terminal width 80-240 columns
- Must work with existing event handler chain
- No breaking changes to public APIs
- Preserve existing keyboard shortcuts

**Scale/Scope**:
- Single input field with multiple attachments
- Editor mode up to 10,000 lines
- Up to 10 concurrent attachments per session
- 50-100 LOC per new module

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Verify compliance with `.specify/memory/constitution.md`:

- [x] **Modularity First**: Feature follows modular organization (clear module boundaries, single responsibility)
  - New modules: `src/tui/input/`, with submodules for wrapping, attachments, editor
  - Each module has single responsibility: wrapping logic, attachment storage, editor mode
  - Re-export through existing `src/tui/mod.rs`

- [x] **Explicit Error Handling**: All error paths use `anyhow::Result<T>` with context
  - Clipboard operations already use Result types
  - Will add context for wrapping calculations, attachment operations
  - No silent failures in paste/editor operations

- [x] **Async-First Architecture**: All I/O operations are async, shared state uses `Arc<T>`
  - N/A: This feature is purely synchronous TUI rendering and event handling
  - No I/O operations involved (clipboard via arboard is synchronous by design)
  - State stored in AppState (already passed around as mutable reference)

- [x] **Testing Discipline**: Tests focus on behavior, not implementation; test names describe behavior
  - Test names like `wrapping_respects_terminal_width`, `large_paste_creates_attachment`
  - Cover: normal paste, large paste, editor mode toggle, wrapping on resize
  - Unit tests for wrapping logic, integration tests for full paste workflow

- [x] **Simplicity and Clarity**: Code prioritizes clarity; naming follows conventions; dependencies justified
  - Use textwrap (already in deps) for wrapping - no new deps needed
  - Clear naming: `AttachmentManager`, `InputWrapper`, `EditorMode`
  - No premature abstraction - direct implementations first

**Violations**: None - feature aligns with all constitution principles

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
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
# [REMOVE IF UNUSED] Option 1: Single project (DEFAULT)
src/
├── models/
├── services/
├── cli/
└── lib/

tests/
├── contract/
├── integration/
└── unit/

# [REMOVE IF UNUSED] Option 2: Web application (when "frontend" + "backend" detected)
backend/
├── src/
│   ├── models/
│   ├── services/
│   └── api/
└── tests/

frontend/
├── src/
│   ├── components/
│   ├── pages/
│   └── services/
└── tests/

# [REMOVE IF UNUSED] Option 3: Mobile + API (when "iOS/Android" detected)
api/
└── [same as backend above]

ios/ or android/
└── [platform-specific structure: feature modules, UI flows, platform tests]
```

**Structure Decision**: [Document the selected structure and reference the real
directories captured above]

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
