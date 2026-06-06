# Data Model: Session Summary Mode (007)

## Entities

### MemoryMode (enum)

```rust
// src/memory_mode/mod.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MemoryMode {
    #[default]
    Conversation,   // Full history retention (existing behavior)
    Summary,        // Turn-by-turn summary injection
}
```

- Mirrors `TerminalMode` pattern exactly (same traits, same serde, same `FromStr`/`Display`)
- `Conversation` is default — zero behavior change for existing users
- Serializes as `"conversation"` / `"summary"` in config TOML

---

### MemoryModeManager (struct)

```rust
// src/memory_mode/mod.rs
pub struct MemoryModeManager {
    conversation_id: String,
    memory_dir: PathBuf,     // ~/.local/share/hoosh/memory/<conv_id>/
}
```

**Responsibilities**:
- Constructs and owns the path to `.hoosh/memory/<conv_id>/summary.txt`
- Creates the directory on construction (`create_dir_all`)
- Reads summary content for injection
- Checks whether the summary file was modified since a given timestamp (for fallback detection)

**Key methods**:
```rust
impl MemoryModeManager {
    pub fn new(conversation_id: &str) -> Result<Self>;
    pub fn summary_path(&self) -> PathBuf;
    pub fn read_summary(&self) -> Option<String>;
    pub fn summary_modified_since(&self, since: SystemTime) -> bool;
}
```

**Validation rules**:
- `read_summary()` returns `None` on any I/O error (never panics, never propagates)
- `summary_modified_since()` returns `false` if file doesn't exist or metadata unavailable

---

### UpdateSessionFileTool (struct)

```rust
// src/memory_mode/tool.rs
pub struct UpdateSessionFileTool;
```

**Tool trait implementation**:

| Method | Value |
|--------|-------|
| `name()` | `"update_session_file"` |
| `display_name()` | `"UpdateSessionFile"` |
| `description()` | See below |
| `parameter_schema()` | `{ summary: { type: "string", description: "..." } }` |
| `describe_permission()` | Write, low-risk, target = summary file path |

**Description** (shown to agent):
> Write a summary of the current turn to the session memory file. Call this tool at the END of every turn — after all work is complete — with a concise summary of: (1) actions taken, (2) outcomes and results, (3) key decisions made, (4) relevant state for the next turn. Do NOT call mid-turn.

**Execution**:
- Reads `summary` string from args
- Gets conversation ID from `context.parent_conversation_id`
- Constructs path: `<data_dir>/memory/<conv_id>/summary.txt`
- Writes summary (overwrites), returns success confirmation string
- Returns `ToolError::ExecutionFailed` if path construction fails (no conv ID)

**Parameter schema**:
```json
{
  "type": "object",
  "properties": {
    "summary": {
      "type": "string",
      "description": "Concise summary of this turn: actions taken, outcomes, decisions, and state relevant to the next turn."
    }
  },
  "required": ["summary"]
}
```

---

### SummaryFile (file artifact)

**Path**: `<data_dir>/memory/<conv_id>/summary.txt`

**Format**: Plain text (unstructured, agent-generated). The agent is instructed to include:
1. Actions taken
2. Outcomes and results
3. Key decisions
4. Current state / what's pending

**Lifecycle**:
- Created: first time agent calls `update_session_file`
- Updated: overwritten at end of each turn (not appended)
- Read: at start of each subsequent turn before `handle_turn()`
- Deleted: never automatically — persists until conversation is deleted

**Size**: No hard limit enforced, but agent is instructed to keep summaries concise. Practical limit driven by model context window.

---

## Config Changes

### AppConfig (additive)

```rust
// src/config/mod.rs — inside AppConfig struct
#[serde(default)]
pub memory_mode: Option<MemoryMode>,
```

```rust
// AppConfig::default() — add to Self { ... }
memory_mode: None,
```

### ProjectConfig (additive, mirrors AppConfig pattern)

```rust
#[serde(default)]
pub memory_mode: Option<MemoryMode>,
```

---

## CLI Changes

### Cli struct (additive)

```rust
// src/cli/mod.rs — inside Cli struct
/// Memory mode: how conversation history is managed (conversation, summary)
#[arg(long = "memory-mode", value_parser = ["conversation", "summary"])]
pub memory_mode: Option<String>,
```

---

## Conversation Changes

### New method on Conversation

```rust
// src/agent/conversation.rs
impl Conversation {
    /// Remove all messages after the initial system messages (agent def + env context).
    /// Preserves messages at indices 0 and 1 which are always the initial system prompts.
    pub fn clear_turn_history(&mut self) {
        if self.messages.len() > 2 {
            self.messages.truncate(2);
        }
    }
}
```

**Invariant**: Initial system messages are always added at positions 0 (agent definition) and 1 (environment context) in `load_or_create_conversation()`. This is a stable assumption.

---

## State Flow Per Turn (Summary Mode)

```
Turn N start:
  1. record turn_start = SystemTime::now()
  2. summary = manager.read_summary()          // None on first turn
  3. if summary.is_some():
       conv.clear_turn_history()
       conv.add_system_message(summary)
  4. conv.add_user_message(user_input)          // existing line
  5. agent.handle_turn(&mut conv)              // existing call
  6. wrote_summary = manager.summary_modified_since(turn_start)
  7. if !wrote_summary: log warning ("Memory mode: summary not written this turn, retaining full history for next turn")
```
