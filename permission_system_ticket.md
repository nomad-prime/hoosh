# Ticket: Fix Permission Check Ordering & Add Persistence

## Problem Statement

1. **Permission Check Ordering Issue**: Currently, user approval is requested before system permissions are checked,
   leading to confusing UX where users approve operations that then get denied by the permission system.

2. **Missing Permission Persistence**: Granular permissions (e.g., "always allow cargo check in this project") are not
   persisted to disk, requiring users to re-approve operations every session.

## Current Behavior

```rust
// Current problematic flow in tool_executor.rs:97-115
1.Generate preview
2.Request user approval ( if not autopilot)
3.Check system permissions  // <- Should happen FIRST
4.Execute tool
```

## Expected Behavior

System permissions should be checked before requesting user approval, and permission decisions should be persisted
across sessions.

## Implementation Tasks

### Task 1: Fix Permission Check Ordering

**File**: `src/tool_executor.rs`

- [ ] Move `check_tool_permissions()` call before `generate_preview()` and approval flow
- [ ] Ensure permission denial messages are clear and actionable
- [ ] Test that cached permissions skip the approval dialog entirely

### Task 2: Add Permission Persistence

**Files**: `src/permissions/mod.rs`, `src/permissions/cache.rs` (new file)

- [ ] Create a persistence layer for the permission cache
- [ ] Store permissions in `~/.config/hoosh/permissions.json` or similar
- [ ] Implement the following structure:
  ```json
  {
    "projects": {
      "/path/to/project": {
        "commands": {
          "cargo_check": "allow",
          "cargo_build": "allow"
        },
        "files": {
          "/path/to/file.rs": "allow"
        },
        "directories": {
          "/path/to/src/": "allow"
        }
      }
    },
    "global": {
      "read_operations": "allow"
    }
  }
  ```

### Task 3: Update Permission Manager

**File**: `src/permissions/mod.rs`

- [ ] Add `load_from_disk()` method to load cached permissions on startup
- [ ] Add `save_to_disk()` method to persist after each permission update
- [ ] Ensure atomic writes to prevent corruption
- [ ] Add migration logic for future schema changes

### Task 4: Update CLI Commands

**File**: `src/tui/commands.rs` or relevant command handler

- [ ] Add `/permissions list` command to show current permissions
- [ ] Add `/permissions clear [project]` to reset permissions
- [ ] Add `/permissions export` and `/permissions import` for backup/sharing

## Technical Details

### Proposed Permission Check Flow

```rust
async fn execute_tool_call(&self, tool: &dyn Tool, args: &Value) -> ToolResult {
    // 1. Check system/path permissions first
    match self.check_tool_permissions(tool, args).await {
        Ok(PermissionResult::Allowed) => {
            // Has cached "always allow" - skip everything else
            return tool.execute(args).await;
        }
        Ok(PermissionResult::NeedsApproval) => {
            // Continue to preview/approval flow
        }
        Err(e) => {
            return ToolResult::error(format!(
                "Permission denied: {}. Use /trust or update permissions.", e
            ));
        }
    }

    // 2. Generate preview (only if needed)
    if let Some(preview) = tool.generate_preview(args).await {
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(AgentEvent::ToolPreview { ... });
        }

        // 3. Request approval (if not autopilot)
        if !is_autopilot {
            match self.request_approval(...).await {
                Ok(ApprovalResult::Approved) => {
                    // Continue to execution
                }
                Ok(ApprovalResult::ApprovedAlways) => {
                    // Cache and persist this decision
                    self.permission_manager.add_permission(...);
                    self.permission_manager.save_to_disk().await?;
                }
                Ok(ApprovalResult::Rejected) => {
                    return ToolResult::error("User rejected");
                }
                // ... handle other cases
            }
        }
    }

    // 4. Execute
    tool.execute(args).await
}
```

### File Format Considerations

- Use JSON for human readability and easy editing
- Consider using `serde_json::to_writer_pretty()` for formatted output
- Implement file locking to prevent concurrent modifications
- Add version field for future migrations

## Testing Requirements

1. **Unit Tests**
    - Test permission ordering logic
    - Test cache persistence and loading
    - Test permission matching logic

2. **Integration Tests**
    - Test full flow with persistent permissions
    - Test permission survival across sessions
    - Test project-specific vs global permissions

3. **Manual Testing**
    - Verify UX improvement: no approval-then-denial scenarios
    - Test "always allow" actually persists
    - Test `/permissions` commands work correctly

## Success Criteria

- [ ] Users never see "approved but then denied" scenarios
- [ ] "Always allow" decisions persist across sessions
- [ ] Permissions are project-scoped and portable
- [ ] Clear feedback when operations are blocked by permissions
- [ ] Existing permission system behavior is preserved (just reordered)

## Notes

- Consider adding an "always deny" option for security-conscious users
- Future enhancement: permission profiles (dev/production/restricted)
- Consider adding expiry dates to permissions for extra security
