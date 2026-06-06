# Tool Contract: update_session_file

## Tool Identity

| Field | Value |
|-------|-------|
| Name | `update_session_file` |
| Availability | Only when `--memory-mode summary` is active |
| Permission level | Write (low-risk, fixed path) |

## Input Contract

```json
{
  "type": "object",
  "properties": {
    "summary": {
      "type": "string",
      "description": "Concise summary of this turn: actions taken, outcomes, decisions, and state relevant to the next turn.",
      "minLength": 1
    }
  },
  "required": ["summary"]
}
```

## Output Contract

**Success**: Returns a confirmation string, e.g.:
```
Session summary written to memory.
```

**Failure — missing conversation ID**:
```
ToolError::ExecutionFailed("update_session_file: no conversation ID available in execution context")
```

**Failure — I/O error**:
```
ToolError::ExecutionFailed("update_session_file: failed to write summary: <os error>")
```

## Side Effects

- Writes (overwrites) `<data_dir>/memory/<conv_id>/summary.txt`
- No network calls, no other file system mutations

## Calling Convention

- MUST be called at end of turn, after all other tool calls and work are complete
- MUST NOT be called mid-turn (intermediate calls will be overwritten anyway)
- SHOULD be called once per turn; multiple calls within a turn are idempotent (last write wins)

## Permission Descriptor

```rust
ToolPermissionDescriptor {
    tool_name: "update_session_file",
    action: "write session summary",
    target: "<data_dir>/memory/<conv_id>/summary.txt",
    risk_level: RiskLevel::Low,
    requires_approval: false,
}
```
