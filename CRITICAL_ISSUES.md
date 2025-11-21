# Critical Issues Documentation

## 1. UNWRAP CALLS (335 total)

### High Priority - Production Code Unwraps

#### Permissions Module (18 unwraps)
- **File**: `src/permissions/storage.rs` (lines 184-235, 251-273, 291)
  - JSON serialization/deserialization without error handling
  - Test file creation and operations
  - **Risk**: Panic on invalid JSON or I/O failures

- **File**: `src/permissions/tool_permission.rs` (lines 301, 317, 331, 350, 364, 383, 398, 402)
  - Pattern matching and file operations
  - **Risk**: Panic when patterns fail to compile

#### Agent/Conversation Module (20 unwraps)
- **File**: `src/agent/conversation.rs` (lines 116, 308, 642, 683)
  - Tool call retrieval without fallback
  - **Risk**: Panic on missing tool calls

#### Storage Module (30+ unwraps)
- **File**: `src/storage/conversation.rs` (lines 23, 43, 202-287)
  - Message append, load, and list operations
  - **Risk**: Panic on file I/O failures

#### Context Management (40+ unwraps)
- **File**: `src/context_management/tool_output_truncation_strategy.rs` (lines 232-912)
- **File**: `src/context_management/sliding_window_strategy.rs` (lines 214-1121)
  - JSON parsing and message content access
  - **Risk**: Panic on malformed message data

#### Tools Module (50+ unwraps)
- **File**: `src/tools/grep.rs` (lines 204-206, 513-515, 543, 610, 669, 730)
- **File**: `src/tools/bash/tool.rs` (lines 74, 447, 465, 528, 538, 587, 623)
- **File**: `src/tools/file_ops/*` (read_file.rs, write_file.rs, edit_file.rs, list_directory.rs)
  - File operations and command execution
  - **Risk**: Panic on file access or parsing failures

#### Config Module (15+ unwraps)
- **File**: `src/config/mod_tests.rs` (lines 122-597)
  - TOML serialization/deserialization
  - **Risk**: Panic on invalid config data

### Low Priority - Test Code Unwraps

- **File**: `src/tool_executor.rs` (lines 302-362)
- **File**: `src/parser/mod.rs` (lines 278-308)
- **File**: `src/backends/openai_compatible_tests.rs` (multiple)
- **File**: `src/security/path_validator.rs` (multiple)
- Various test files with TempDir and mock data creation

**Recommendation**: Replace production unwraps with `?` operator or `.context()` for better error messages.

---

## 2. PANIC CALLS (9 total)

### Location 1: src/commands/compact_command.rs:172
```rust
_ => panic!("Expected Success result"),
```
**Issue**: Unmatched result variant in command execution  
**Fix**: Return proper error instead

### Location 2: src/permissions/storage.rs:349
```rust
_ => panic!("Expected UnsupportedVersion error"),
```
**Issue**: Test assertion using panic instead of assert!  
**Fix**: Use `assert!(matches!(result, Err(...)))`

### Location 3: src/permissions/storage.rs:393
```rust
_ => panic!("Expected UnsupportedVersion error"),
```
**Issue**: Same as Location 2

### Location 4: src/tools/grep.rs:429
```rust
_ => panic!("Expected Content output mode"),
```
**Issue**: Unhandled output mode variant  
**Fix**: Return error for unknown output modes

### Location 5: src/tools/grep.rs:453
```rust
_ => panic!("Expected FilesWithMatches output mode"),
```
**Issue**: Same as Location 4

### Location 6: src/backends/openai_compatible_tests.rs:317
```rust
panic!("Expected RecoverableByLlm error");
```
**Issue**: Test panic for failed assertion  
**Fix**: Use `assert!(matches!(...))`

### Locations 7-9: src/tui/init_permission/init_permission_state.rs:98, 108, 118
```rust
_ => panic!("Expected ReadOnly");
_ => panic!("Expected EnableWriteEdit");
_ => panic!("Expected Deny");
```
**Issue**: Permission state enum match panics  
**Fix**: Use proper error handling for invalid states

---

## 3. PERMISSION FILE RACE CONDITION

### Problem
**File**: `src/permissions/mod.rs` (lines 70-115)  
**File**: `src/permissions/storage.rs` (lines 50-59)

The permission file is not protected against concurrent writes:

```rust
// CURRENT IMPLEMENTATION - NOT THREAD SAFE
pub fn save_permissions(&self) -> Result<()> {
    let permissions_file = self.permissions_file.try_lock()?;  // Locks in-memory struct
    permissions_file.save_permissions(&project_root)           // But file I/O is unprotected!
}

pub fn save_permissions(&self, project_root: &Path) -> Result<(), anyhow::Error> {
    let path = Self::get_permissions_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;  // No lock here
    }
    let json = serde_json::to_string_pretty(self)?;
    std::fs::write(&path, json)?;         // Direct write - multiple processes can corrupt file
    Ok(())
}
```

### Race Condition Scenario
1. Process A locks `permissions_file` mutex
2. Process A serializes to JSON
3. Process A **releases lock** (step into save_permissions)
4. Process B locks `permissions_file` mutex
5. Process B serializes to JSON
6. Both processes write to disk simultaneously → **File corruption**

### Impact
- Concurrent calls from multiple tool executions can corrupt `~/.hoosh/permissions.json`
- Corrupted file causes `PermissionLoadError::Parse` on next load
- May require manual file deletion to recover

### Solution
Use file-level locking:
- Option 1: Use `fs2` crate for advisory file locking
- Option 2: Write to temporary file, then atomic rename
- Option 3: Extend Mutex scope to include all file operations

### Affected Code Paths
- `PermissionManager::add_tool_permission_rule()` → calls `save_permissions()` twice
- `PermissionManager::clear_all_permissions()` → calls `save_permissions()`
- Direct permission file saves during TUI interactions

---

## 4. SUMMARY TABLE

| Issue Type | Count | Severity | Files Affected |
|-----------|-------|----------|-----------------|
| Unwrap Calls | 335 | HIGH | 20+ files |
| Panic Calls | 9 | MEDIUM | 4 files |
| Race Conditions | 1 | HIGH | permissions/* |
| Compression Issues | REMOVED | N/A | N/A |

## Recommended Fix Order

1. **CRITICAL**: Fix permission file race condition (File locking)
2. **HIGH**: Replace production unwraps (Add error handling)
3. **MEDIUM**: Remove panic statements (Use proper error returns)
4. **LOW**: Clean up test unwraps (Use expect with descriptions)
