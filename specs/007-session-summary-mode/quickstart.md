# Quickstart: Session Summary Mode (007)

## For Users

### Enable via CLI flag
```bash
hoosh --memory-mode summary
```

### Enable via config file (`~/.config/hoosh/config.toml`)
```toml
memory_mode = "summary"
```

### Resume a session in summary mode
```bash
hoosh --memory-mode summary --continue
```

The summary from the last turn is automatically injected at conversation start.

### Revert to full history mode
```bash
hoosh --memory-mode conversation
# or simply omit --memory-mode (conversation is the default)
```

---

## For Implementers

### New module layout
```
src/memory_mode/
├── mod.rs       # MemoryMode enum + MemoryModeManager
└── tool.rs      # UpdateSessionFileTool (implements Tool trait)
```

### Files changed (minimal)
| File | Change |
|------|--------|
| `src/config/mod.rs` | +1 field `memory_mode: Option<MemoryMode>` in `AppConfig` and `ProjectConfig` |
| `src/cli/mod.rs` | +1 arg `--memory-mode` in `Cli` |
| `src/cli/agent.rs` | Parse memory mode, conditionally register tool, pass to session |
| `src/session.rs` | +1 field `memory_mode` in `SessionConfig`, propagate to context |
| `src/tui/actions.rs` | Inject summary + fallback logic (~25 lines) |
| `src/tagged_mode.rs` | Same injection logic (~15 lines) |
| `src/agent/conversation.rs` | +1 method `clear_turn_history()` |

### Turn flow in summary mode
```
user submits message
  → [NEW] read .hoosh/memory/<conv_id>/summary.txt
  → [NEW] if exists: conv.clear_turn_history() + conv.add_system_message(summary)
  → conv.add_user_message(input)           [existing]
  → agent.handle_turn(&mut conv)           [existing]
    → agent calls update_session_file(summary) near end of turn
    → tool writes .hoosh/memory/<conv_id>/summary.txt
  → [NEW] check if file was modified → if not, log warning (fallback: full history retained)
user sees response
```

### Summary file location
```
~/.local/share/hoosh/memory/<conversation_id>/summary.txt
```
Created on first `update_session_file` call. Persists across process restarts.

### Testing the feature
```bash
# Run with summary mode
hoosh --memory-mode summary

# Check summary file was written after first turn
cat ~/.local/share/hoosh/memory/<conv_id>/summary.txt

# Verify next turn injects it (enable verbose logging)
hoosh --memory-mode summary --continue -v
```
