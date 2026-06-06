# Research: Session Summary Mode (007)

## Decision 1: MemoryMode Enum Placement

**Decision**: New file `src/memory_mode/mod.rs` — not inside `config/` or alongside `terminal_mode.rs`.

**Rationale**: `TerminalMode` is a display concern. `MemoryMode` is a conversation-management concern. Keeping it in its own top-level module (`memory_mode/`) allows the entire feature to be self-contained, grouping the enum, manager, and tool together. Follows constitution principle IV (flat module structure).

**Alternatives considered**: Adding to `terminal_mode.rs` — rejected because unrelated concerns. Adding to `config/` — rejected because it's more than config, it drives runtime behavior.

---

## Decision 2: UpdateSessionFileTool — Conversation ID Access

**Decision**: Use `ToolExecutionContext.parent_conversation_id` (already present in the tool execution context) to construct the summary file path at execution time.

**Rationale**: `ToolExecutionContext` already carries `parent_conversation_id: Option<String>` — this is exactly the identifier needed to locate `.hoosh/memory/<conv_id>/summary.txt`. No new infrastructure needed.

**Alternatives considered**: Storing the path in the tool struct at construction time — rejected because it requires the tool to be rebuilt per-conversation. Using a global/shared state — rejected as over-engineering.

---

## Decision 3: History Clearing Mechanism

**Decision**: Add a `clear_turn_history()` method to `Conversation` that removes all messages except the initial system messages (agent definition and environment context, which are always at indices 0 and 1).

**Rationale**: The injection point in `actions.rs::answer()` needs to strip prior turn messages before adding the summary and the new user message. A single targeted method on `Conversation` is the minimal change — it's a conceptually clean operation ("reset to initial state") and keeps the logic out of the action layer.

**Alternatives considered**: Creating a fresh `Conversation` each turn — rejected because it loses the initial system messages and requires re-initializing. Modifying context manager strategies — rejected as over-engineering and the wrong abstraction (summary mode is not context compression).

---

## Decision 4: Fallback Detection

**Decision**: Record the summary file's last-modified timestamp before `handle_turn()` starts. After `handle_turn()` returns, compare. If unchanged → fallback mode for this turn (skip history clear on next turn). Store as `Option<SystemTime>` in turn-local state within `answer()`.

**Rationale**: The tool writes a file — modification time is a reliable, side-effect-free way to detect whether it was called. No shared mutable state needed between tool and action layer.

**Alternatives considered**: Shared `Arc<AtomicBool>` flag set by the tool — rejected as unnecessary coupling between tool and action layer. Scanning conversation for tool call — rejected as fragile.

---

## Decision 5: Summary Injection as System Message

**Decision**: Inject the summary via `conversation.add_system_message()` after `clear_turn_history()` and before `add_user_message()`.

**Rationale**: System messages are authoritative background context in the Anthropic message format. The summary IS background context ("here is what happened before"), not a user turn. Injecting as system preserves the correct role semantics and matches how environment context is already injected.

**Alternatives considered**: Injecting as user message — rejected because it corrupts the user/assistant turn alternation and semantically misrepresents the summary.

---

## Decision 6: Tool Registration — Conditional on Memory Mode

**Decision**: Register `UpdateSessionFileTool` in `handle_agent()` (in `src/cli/agent.rs`) only when `memory_mode == MemoryMode::Summary`, alongside the existing `BuiltinToolProvider` registration.

**Rationale**: Tool registration happens at session setup in `handle_agent()`. This is the right point — before `SessionConfig` is built. Conditional registration ensures the tool is genuinely unavailable (not just hidden) in `conversation` mode.

**Alternatives considered**: Always registering but gating at execute time — rejected because the tool would still appear in the agent's tool list, wasting tokens and causing confusion. Moving registration into `session.rs` — possible but `handle_agent` is cleaner since it already has the mode.

---

## Decision 7: Agent Prompt for Summary Instructions

**Decision**: Add a new system prompt block injected as an additional `add_system_message()` call in `load_or_create_conversation()` when `memory_mode == MemoryMode::Summary`. Content instructs the agent to call `update_session_file` at end of every turn with: actions taken, outcomes, key decisions, and current state.

**Rationale**: The agent needs explicit instruction since `update_session_file` is a new tool with a non-obvious calling convention (must be called at end of turn, not mid-turn). A system message at conversation start is the correct place — it frames agent behavior for the whole session.

**Alternatives considered**: Baking instructions into the tool description — insufficient since tool descriptions are short and don't convey the "call at end of turn" timing constraint. Adding to core instructions file — rejected because memory mode is opt-in and shouldn't affect agents not using it.

---

## Decision 8: Memory Directory Creation

**Decision**: `MemoryModeManager::new()` creates `.hoosh/memory/<conv_id>/` directory on construction if it doesn't exist, using `fs::create_dir_all()`.

**Rationale**: Fail-fast: if the directory can't be created, it's better to error at session start than silently fail at turn end when the tool tries to write. Mirror pattern used by `ConversationStorage`.

---

## Integration Points Summary

| Concern | File | Change Type |
|---------|------|-------------|
| Enum + manager | `src/memory_mode/mod.rs` | New |
| Tool | `src/memory_mode/tool.rs` | New |
| Config field | `src/config/mod.rs` | Additive (1 field) |
| CLI flag | `src/cli/mod.rs` | Additive (1 arg) |
| Tool registration + mode wiring | `src/cli/agent.rs` | ~15 lines |
| Session propagation | `src/session.rs` | ~10 lines |
| Turn injection + fallback | `src/tui/actions.rs` | ~25 lines |
| Tagged mode injection | `src/tagged_mode.rs` | ~15 lines |
| Clear method | `src/agent/conversation.rs` | 1 new method |
