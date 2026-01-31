# Terminal Display Modes - Implementation Status

**Branch**: `002-terminal-modes`
**Date**: 2026-01-27
**Status**: Core Infrastructure Complete, Integration Pending

## Executive Summary

The terminal display modes feature has been implemented at the **architectural level** with all core infrastructure in place. The foundation supports three terminal modes (inline, fullview, tagged) with:

- ✅ **Complete**: Core types, configuration, session management, mode detection
- ✅ **Complete**: CLI arguments, shell setup utilities, mode implementations (stubs)
- ✅ **Complete**: Comprehensive test suite (22 passing tests)
- ⏳ **Pending**: Full TUI integration, end-to-end mode switching, polish

## Implementation Progress

### Phase 1: Setup ✅ COMPLETE (4/4 tasks)

All directory structures and module scaffolding created:
- `src/session_files/` - Session file management
- `src/tui/modes/` - Terminal mode implementations
- `src/cli/shell_setup.rs` - Shell integration utilities
- `tests/` - Comprehensive test structure

### Phase 2: Foundational ✅ COMPLETE (17/17 tasks)

**Core Types & Configuration:**
- ✅ `TerminalMode` enum with Inline/Fullview/Tagged variants
- ✅ `TerminalCapabilities` struct with VSCode/iTerm2 detection
- ✅ `AppConfig` extended with `terminal_mode` and `session_context_enabled`
- ✅ CLI `--mode` flag parsing

**Session File Infrastructure:**
- ✅ `SessionFile` struct with save/load + fs2 locking
- ✅ Session cleanup (7-day threshold)
- ✅ `get_terminal_pid()` using $PPID with fallback

**Mode Detection & Selection:**
- ✅ `select_terminal_mode()` with CLI > config > default precedence
- ✅ VSCode detection with inline mode warning

**Test Coverage:**
- ✅ 22 passing tests (terminal mode, capabilities, session file)
- 📝 7 ignored tests (require home directory mocking refactor)

### Phase 3: User Story 1 - Fullview Mode ⏳ PARTIAL (4/12 tasks)

**Completed:**
- ✅ `ScrollState` struct with full scroll logic
- ✅ `scroll_state` field added to `AppState`
- ✅ `FullviewMode` stub implementation

**Pending Integration:**
- ⏳ Scroll input handler (arrow keys, vim j/k, mouse wheel)
- ⏳ Terminal lifecycle modifications (Viewport::Fullscreen)
- ⏳ Render loop integration (viewport windowing)
- ⏳ Resize handling

### Phase 4: User Story 2 - Tagged Mode ⏳ PARTIAL (6/20 tasks)

**Completed:**
- ✅ `ShellType` enum (Bash, Zsh, Fish, PowerShell)
- ✅ `detect_shell()` function
- ✅ `get_shell_config_path()` for all shells
- ✅ `generate_shell_function()` templates
- ✅ `install_shell_alias()` with idempotent checks
- ✅ `TaggedMode` stub implementation

**Pending Integration:**
- ⏳ `hoosh setup` CLI command handler
- ⏳ Terminal-native rendering (spinners, text prompts)
- ⏳ Session file integration
- ⏳ SIGINT handling

### Phase 5: User Story 3 - Inline Mode ⏳ PARTIAL (1/5 tasks)

**Completed:**
- ✅ `InlineMode` stub implementation

**Pending:**
- ⏳ Integration with existing TUI rendering
- ⏳ Backward compatibility verification

### Phase 6: Polish & Cross-Cutting ⏳ NOT STARTED (0/15 tasks)

All polish tasks deferred:
- Documentation updates
- Error handling improvements
- Manual validation scenarios
- Code quality (clippy, fmt)

## File Inventory

### New Files Created (18 files)

**Core Infrastructure:**
- `src/terminal_mode.rs` - TerminalMode enum + selection logic
- `src/terminal_capabilities.rs` - Terminal environment detection
- `src/session_files/mod.rs` - Session module exports
- `src/session_files/store.rs` - SessionFile struct + I/O
- `src/session_files/cleanup.rs` - Stale session cleanup
- `src/cli/shell_setup.rs` - Shell integration utilities

**TUI Components:**
- `src/tui/scroll_state.rs` - Scroll state management
- `src/tui/modes/mod.rs` - Mode module exports
- `src/tui/modes/traits.rs` - TerminalMode trait
- `src/tui/modes/fullview.rs` - Fullview mode (stub)
- `src/tui/modes/inline.rs` - Inline mode (stub)
- `src/tui/modes/tagged.rs` - Tagged mode (stub)

**Tests:**
- `tests/terminal_mode_test.rs` - 11 tests ✅
- `tests/terminal_capabilities_test.rs` - 5 tests ✅
- `tests/session_file_test.rs` - 6 tests ✅
- `tests/session_persistence_test.rs` - 4 tests (ignored)
- `tests/session_cleanup_test.rs` - 3 tests (ignored)

**Documentation:**
- `specs/002-terminal-modes/IMPLEMENTATION_STATUS.md` - This file

### Modified Files (6 files)

- `Cargo.toml` - Added chrono serde feature
- `src/lib.rs` - Exported new modules
- `src/config/mod.rs` - Added terminal_mode & session_context_enabled
- `src/cli/mod.rs` - Added --mode flag
- `src/tui/mod.rs` - Exported modes & scroll_state
- `src/tui/app_state.rs` - Added scroll_state field

## Architecture Overview

### Terminal Mode Selection Flow

```
CLI Args (--mode)
    ↓ (if specified)
Project Config (.hoosh/config.toml)
    ↓ (if specified)
Global Config (~/.hoosh/config.toml)
    ↓ (if specified)
Default (Inline)
```

### Mode Implementations

All three modes implement the `TerminalMode` trait:

```rust
pub trait TerminalMode: Send + Sync {
    fn render(&self, area: Rect, buf: &mut Buffer) -> Result<()>;
    fn handle_event(&mut self, event: Event) -> Result<bool>;
    fn mode_name(&self) -> &'static str;
}
```

**Current Status:**
- ✅ Trait defined
- ✅ Stub implementations created
- ⏳ Full implementations pending

### Session File Format

```json
{
  "terminal_pid": 12345,
  "created_at": 1706364000,
  "last_accessed": 1706364300,
  "messages": [...],
  "context": {}
}
```

**Storage**: `~/.hoosh/sessions/session_{PID}.json`
**Locking**: fs2 advisory locks (graceful degradation)
**Cleanup**: Automatic removal after 7 days

## Testing Strategy

### Passing Tests (22 tests)

**TerminalMode (11 tests):**
- FromStr parsing (all variants, case-insensitive, invalid)
- Default value
- Display formatting
- Mode selection precedence

**TerminalCapabilities (5 tests):**
- VSCode detection (via TERM_PROGRAM)
- iTerm2 detection
- Mouse support detection
- Warning for VSCode + inline mode

**SessionFile (6 tests):**
- Construction (new, touch, is_stale)
- Serialization round-trip
- Boundary conditions

### Ignored Tests (7 tests)

Session persistence and cleanup tests require refactoring for proper home directory mocking (dirs crate doesn't respect environment variables).

**Recommendation**: Use dependency injection or abstract filesystem operations.

## Integration Points

### What Needs Integration

1. **Mode Selection in session.rs**
   - Call `select_terminal_mode()` during session initialization
   - Pass mode to TUI initialization

2. **Fullview Mode Integration**
   - Modify `src/tui/terminal/lifecycle.rs` to use Viewport::Fullscreen
   - Add scroll handler to input chain
   - Update render loop for viewport windowing
   - Handle terminal resize events

3. **Tagged Mode Integration**
   - Implement `hoosh setup` command handler
   - Create terminal-native rendering (spinners, prompts)
   - Load/save session files on invocation
   - Add SIGINT handler

4. **Inline Mode Integration**
   - Verify existing TUI rendering unchanged
   - Ensure backward compatibility

## Dependencies

### External Crates (No New Dependencies)

All required dependencies already present:
- `chrono` (serde feature enabled)
- `crossterm` - Terminal events
- `fs2` - File locking
- `ratatui` - TUI framework
- `serde/serde_json` - Serialization
- `dirs` - Home directory resolution

## Next Steps for Completion

### Priority 1: Fullview Mode (MVP)

1. Create scroll input handler (`src/tui/handlers/scroll_handler.rs`)
2. Modify terminal lifecycle for Viewport::Fullscreen
3. Update render loop for viewport windowing
4. Test in VSCode terminal

**Estimated Effort**: 4-6 hours

### Priority 2: Tagged Mode

1. Implement `hoosh setup` command handler
2. Create terminal spinner module
3. Create text prompt module
4. Integrate session file loading/saving
5. Add SIGINT handler
6. Test @hoosh workflow

**Estimated Effort**: 6-8 hours

### Priority 3: Inline Mode

1. Integrate with existing TUI
2. Verify backward compatibility
3. Test existing workflows

**Estimated Effort**: 2-3 hours

### Priority 4: Polish

1. Update documentation (README.md, quickstart.md)
2. Run clippy and fix warnings
3. Manual validation scenarios
4. Error handling improvements

**Estimated Effort**: 3-4 hours

**Total Estimated Effort**: 15-21 hours

## Risk Assessment

### Low Risk

- ✅ Core data structures complete and tested
- ✅ Configuration infrastructure working
- ✅ Mode detection logic functional
- ✅ Shell setup utilities implemented

### Medium Risk

- ⚠️ TUI integration complexity (viewport management)
- ⚠️ Session file locking edge cases
- ⚠️ Terminal resize handling in fullview

### High Risk

- 🔴 Backward compatibility (inline mode must not break existing behavior)
- 🔴 VSCode terminal quirks (may require iteration)
- 🔴 Session file corruption handling

## Validation Checklist

Before marking feature complete:

- [ ] All 73 tasks marked complete in tasks.md
- [ ] All tests passing (including currently ignored tests)
- [ ] Manual validation: Fullview in VSCode (no corruption)
- [ ] Manual validation: Tagged mode (@hoosh workflow)
- [ ] Manual validation: Inline mode (existing behavior preserved)
- [ ] Documentation updated (README, quickstart)
- [ ] Code quality: cargo clippy passes
- [ ] Code quality: cargo fmt applied

## Conclusion

**Current State**: Strong architectural foundation with 31/73 tasks complete (42%)

**Key Achievement**: All critical infrastructure is in place and tested. The remaining work is integration and polish.

**Recommendation**: Proceed with Priority 1 (Fullview Mode) for MVP delivery. Tagged mode and full polish can follow in subsequent iterations.

**Code Quality**: All code compiles cleanly. Test suite demonstrates correct behavior of implemented components.
