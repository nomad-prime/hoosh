# Implementation Plan: Session Summary Mode

**Branch**: `007-session-summary-mode` | **Date**: 2026-05-05 | **Spec**: [spec.md](spec.md)  
**Input**: Feature specification from `/specs/007-session-summary-mode/spec.md`

## Summary

Add an opt-in `--memory-mode summary` that replaces full conversation history retention with turn-by-turn summary injection. After each agent turn, the agent calls a new `update_session_file` tool to write a concise summary to `.hoosh/memory/<conv_id>/summary.txt`. At the start of each subsequent turn, that summary is injected as a system message (replacing prior history) before `handle_turn()` runs. If the agent fails to write the summary, the full history is retained for that turn as a fallback. Implementation is isolated in a new `src/memory_mode/` module with minimal additive changes to existing files.

## Technical Context

**Language/Version**: Rust 2024 edition  
**Primary Dependencies**: tokio (async), serde/serde_json (serialization), anyhow (errors), clap (CLI) — all existing  
**Storage**: Plain text file at `<data_dir>/memory/<conv_id>/summary.txt`; `std::fs` for I/O  
**Testing**: `cargo test` — unit tests for `MemoryModeManager`, `UpdateSessionFileTool`, `clear_turn_history()`; integration test for full turn cycle  
**Target Platform**: macOS/Linux CLI (same as existing)  
**Performance Goals**: Negligible — one file read and one optional file write per turn  
**Constraints**: Minimal changes to existing code; new code in `src/memory_mode/`; `--memory-mode conversation` must produce zero behavior change  
**Scale/Scope**: Single-user CLI; single active session per process

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Test-First Development | ✅ Pass | Unit tests required for `MemoryModeManager`, tool, `clear_turn_history()`; integration test for full turn |
| II. Trait-Based Design | ✅ Pass | `UpdateSessionFileTool` implements `Tool` trait; `MemoryModeManager` injected into action layer |
| III. Single Responsibility | ✅ Pass | `memory_mode/` handles one concern; tool handles one operation; manager handles file access |
| IV. Flat Module Structure | ✅ Pass | `src/memory_mode/` is one new top-level module with two files |
| V. Clean Code | ✅ Pass | No obvious comments; descriptive naming; existing patterns followed |

No violations.

## Project Structure

### Documentation (this feature)

```text
specs/007-session-summary-mode/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── tool-contract.md
└── tasks.md             # Phase 2 output (/speckit.tasks — not created here)
```

### Source Code

```text
src/
├── memory_mode/         # NEW — entire feature module
│   ├── mod.rs           # MemoryMode enum + MemoryModeManager
│   └── tool.rs          # UpdateSessionFileTool (implements Tool trait)
├── config/
│   └── mod.rs           # +1 field: memory_mode: Option<MemoryMode>
├── cli/
│   ├── mod.rs           # +1 arg: --memory-mode
│   └── agent.rs         # Parse mode, conditionally register tool, pass to session
├── session.rs           # +1 field: memory_mode in SessionConfig; propagate to EventLoopContext
├── tui/
│   └── actions.rs       # Inject summary before handle_turn(); fallback check after
├── tagged_mode.rs       # Same injection logic as actions.rs
└── agent/
    └── conversation.rs  # +1 method: clear_turn_history()

tests/
└── (unit tests colocated in source files per constitution)
```

**Structure Decision**: Single project. New top-level module `memory_mode/` keeps all new code isolated. Changes to existing files are additive-only except for `clear_turn_history()` (new method on existing struct).

## Phase 0: Research

See [research.md](research.md) for full decision log. Key decisions:

1. `MemoryMode` enum in `src/memory_mode/mod.rs` — mirrors `TerminalMode` pattern exactly
2. `UpdateSessionFileTool` gets conversation ID from `ToolExecutionContext.parent_conversation_id` (already present)
3. History clearing via new `Conversation::clear_turn_history()` — truncates to first 2 messages (system prompts)
4. Fallback detection via file modification timestamp (no shared mutable state)
5. Summary injected as system message between `add_user_message()` and `handle_turn()`
6. Tool registered conditionally in `handle_agent()` only when `memory_mode == Summary`
7. Agent instructed via additional system message in `load_or_create_conversation()` when in summary mode

## Phase 1: Design & Contracts

See [data-model.md](data-model.md) for full entity definitions and state flow.  
See [contracts/tool-contract.md](contracts/tool-contract.md) for `update_session_file` input/output contract.

### Implementation Steps (ordered by dependency)

#### Step 1 — `src/memory_mode/mod.rs`

Implement `MemoryMode` enum (mirrors `TerminalMode`):
- `Conversation` (default) and `Summary` variants
- `FromStr`, `Display`, `Serialize`, `Deserialize`, `Default`, `Clone`, `Copy`, `PartialEq`
- `serde(rename_all = "lowercase")`

Implement `MemoryModeManager`:
```rust
pub struct MemoryModeManager {
    conversation_id: String,
    memory_dir: PathBuf,
}

impl MemoryModeManager {
    pub fn new(conversation_id: &str) -> Result<Self>  // creates dir
    pub fn summary_path(&self) -> PathBuf
    pub fn read_summary(&self) -> Option<String>       // None on any error
    pub fn summary_modified_since(&self, since: SystemTime) -> bool
}
```

Unit tests:
- `memory_mode_defaults_to_conversation()`
- `memory_mode_parses_from_str()`
- `manager_creates_directory_on_new()`
- `read_summary_returns_none_when_missing()`
- `read_summary_returns_content_when_present()`
- `summary_modified_since_detects_write()`

#### Step 2 — `src/memory_mode/tool.rs`

Implement `UpdateSessionFileTool` with `Tool` trait:
- `name()` → `"update_session_file"`
- `execute()`: get `conv_id` from `context.parent_conversation_id`, construct path, write file
- `parameter_schema()`: `{ summary: { type: "string" } }`
- `describe_permission()`: write, low-risk
- `is_hidden()` → `false` (agent must see it)

Unit tests:
- `tool_name_is_update_session_file()`
- `tool_writes_summary_to_correct_path()`
- `tool_returns_error_without_conversation_id()`
- `tool_overwrites_existing_summary()`

#### Step 3 — `src/config/mod.rs`

Add to `AppConfig`:
```rust
#[serde(default)]
pub memory_mode: Option<MemoryMode>,
```

Add to `AppConfig::default()`:
```rust
memory_mode: None,
```

Add same field to `ProjectConfig`.

#### Step 4 — `src/cli/mod.rs`

Add to `Cli` struct:
```rust
/// Memory mode: how conversation history is managed (conversation, summary)
#[arg(long = "memory-mode", value_parser = ["conversation", "summary"])]
pub memory_mode: Option<String>,
```

#### Step 5 — `src/cli/agent.rs`

After existing `terminal_mode` parsing, add:
```rust
let memory_mode = cli_memory_mode
    .as_deref()
    .and_then(|s| s.parse::<MemoryMode>().ok())
    .unwrap_or_else(|| config.memory_mode.unwrap_or_default());
```

Conditionally register tool:
```rust
if memory_mode == MemoryMode::Summary {
    tool_registry.register_tool(Arc::new(UpdateSessionFileTool)).ok();
}
```

Pass `memory_mode` into `SessionConfig` (after Step 6).

#### Step 6 — `src/session.rs`

Add `memory_mode: MemoryMode` to `SessionConfig`. Propagate into `RuntimeState` so `actions.rs` can read it via `event_loop_context.runtime`. Construct `MemoryModeManager` once in `initialize_session()` when `memory_mode == Summary` and store in `RuntimeState` as `Option<Arc<MemoryModeManager>>`.

**Note**: Do NOT inject `SUMMARY_MODE_AGENT_INSTRUCTIONS` here. Instructions are embedded in the per-turn injection block in `actions.rs` (Step 8) so they survive `clear_turn_history()` on every turn.

#### Step 7 — `src/agent/conversation.rs`

Add method:
```rust
pub fn clear_turn_history(&mut self) {
    if self.messages.len() > 2 {
        self.messages.truncate(2);
    }
}
```

Unit tests:
- `clear_turn_history_preserves_system_messages()`
- `clear_turn_history_removes_user_and_assistant_messages()`
- `clear_turn_history_is_safe_with_fewer_than_two_messages()`

#### Step 8 — `src/tui/actions.rs`

In `answer()`, before `add_user_message()`:
```rust
let turn_start = SystemTime::now();
if let Some(ref manager) = memory_manager {
    // Always inject on every turn — clears prior history, re-embeds instructions
    let mut conv = conversation.lock().await;
    conv.clear_turn_history();
    let summary = manager.read_summary();
    let content = match summary {
        Some(ref s) => format!("{}\n\n## Session Memory\n\n{}", SUMMARY_MODE_AGENT_INSTRUCTIONS, s),
        None => SUMMARY_MODE_AGENT_INSTRUCTIONS.to_string(),
    };
    conv.add_system_message(content);
}
```

After `handle_turn()` returns:
```rust
if let Some(ref manager) = memory_manager {
    manager.record_turn_end(turn_start); // store turn_start so next turn can check it
}
```

`MemoryModeManager` tracks `last_turn_start: Arc<Mutex<Option<SystemTime>>>`. At turn start, before injecting:
```rust
let should_clear = manager.summary_written_since_last_turn();
if should_clear {
    conv.clear_turn_history();
}
```

Where `summary_written_since_last_turn()` checks if the summary file's modification time is newer than `last_turn_start`. On the first turn, `last_turn_start` is `None` → returns `false` → no clear (correct: nothing to clear). On subsequent turns where the agent wrote a summary → `true` → clear and inject. If agent skipped the tool → `false` → skip clear, full history retained silently for that turn.

`memory_manager: Option<Arc<MemoryModeManager>>` is `None` when `memory_mode == Conversation`.

**Why always inject instructions**: `clear_turn_history()` truncates to the 2 base system messages (agent def + env context). By embedding instructions in the per-turn injection block rather than as a separate initial message, they survive `clear_turn_history()` on every turn.

#### Step 9 — `src/tagged_mode.rs`

Same injection pattern as Step 8, applied to the tagged mode turn loop.

### Agent Instructions Constant

```rust
// src/memory_mode/mod.rs
pub const SUMMARY_MODE_AGENT_INSTRUCTIONS: &str = r#"
## Session Memory Mode

You are in session memory mode. Call `update_session_file` as your **last tool
call** each turn — after all work is done, before your final response to the user.

Write the summary for an agent that has never seen this conversation. Use this
structure:

**Goal**: What the user is ultimately trying to achieve (one sentence, carry
forward unchanged until the goal changes).

**This turn**: What was done — files created/modified, decisions made, commands
run, errors hit and resolved. Be specific; skip anything irrelevant to future turns.

**State**: Where things stand right now. What exists, what works, what doesn't.
Enough context to continue without re-reading the conversation.

**Next**: What remains, if anything.

Rules: under 800 words · no raw file contents · no verbose tool output · call
exactly once per turn.
"#;
```

## Complexity Tracking

No constitution violations — no complexity justification required.
