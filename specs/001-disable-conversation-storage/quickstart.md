# Quickstart: Disable Conversation Storage

**Feature**: 001-disable-conversation-storage
**For**: Developers implementing this feature
**Est. Time**: 30-45 minutes

## Overview

Add a configuration option to disable conversation message persistence while keeping the app fully functional. This guide walks through the implementation in dependency order.

### Important: Two-Tier Configuration

This feature supports configuration at **both** levels:

1. **Global Config**: `~/.config/hoosh/config.toml` (user-wide default)
2. **Project Config**: `<project_root>/.hoosh/config.toml` (project-specific override)

When both are present, the **project-level setting overrides the global setting**. This allows users to:
- Set a global privacy default (e.g., storage always disabled)
- Override for specific projects (e.g., enable storage for work projects)
- Enforce ephemeral conversations for sensitive projects

The implementation in this guide ensures this override behavior works correctly via the existing `AppConfig::merge()` mechanism.

## Prerequisites

- Rust 2024 edition toolchain
- Hoosh development environment set up
- Familiarity with TOML configuration
- Understanding of Rust `Option<T>` and `Arc<T>`

## Implementation Steps

### Step 1: Update Configuration Structures (5 min)

**File**: `src/config/mod.rs`

**1.1 Add field to AppConfig**

Find the `AppConfig` struct (around line 84) and add:

```rust
pub struct AppConfig {
    pub default_backend: String,
    pub backends: HashMap<String, BackendConfig>,
    pub verbosity: Option<String>,
    pub default_agent: Option<String>,
    pub agents: HashMap<String, AgentConfig>,
    pub context_manager: Option<ContextManagerConfig>,
    pub core_reminder_token_threshold: Option<usize>,
    // ADD THIS LINE:
    pub conversation_storage: Option<bool>,
}
```

**1.2 Add field to ProjectConfig**

Find the `ProjectConfig` struct and add the same field:

```rust
pub struct ProjectConfig {
    pub default_backend: Option<String>,
    pub backends: HashMap<String, BackendConfig>,
    pub verbosity: Option<String>,
    pub default_agent: Option<String>,
    pub agents: HashMap<String, AgentConfig>,
    pub context_manager: Option<ContextManagerConfig>,
    pub core_reminder_token_threshold: Option<usize>,
    pub core_instructions_file: Option<String>,
    // ADD THIS LINE:
    pub conversation_storage: Option<bool>,
}
```

**1.3 Update merge logic**

Find the `merge()` method on `AppConfig` and add:

```rust
impl AppConfig {
    pub fn merge(&mut self, project: ProjectConfig) {
        // ... existing merge logic ...

        // ADD THIS:
        if project.conversation_storage.is_some() {
            self.conversation_storage = project.conversation_storage;
        }
    }
}
```

**Verification**: Run `cargo build` to ensure no compilation errors.

---

### Step 2: Modify Session Initialization (10 min)

**File**: `src/session.rs`

**2.1 Read configuration value**

Around line 151 where `ConversationStorage` is created, modify:

```rust
// BEFORE:
let storage = ConversationStorage::with_default_path()?;

// AFTER:
let storage_enabled = config
    .conversation_storage
    .unwrap_or(false);  // Privacy-first: default to disabled

let storage = if storage_enabled {
    Some(Arc::new(ConversationStorage::with_default_path()?))
} else {
    None  // Storage disabled
};
```

**2.2 Update conversation creation logic**

Find where `Conversation` is created and modify to handle optional storage:

```rust
// BEFORE:
let conversation = Conversation::with_storage(id, Arc::new(storage));

// AFTER:
let conversation = match storage {
    Some(storage_arc) => Conversation::with_storage(id, storage_arc),
    None => Conversation::new(),  // Privacy-first: no persistence
};
```

**Note**: You may need to adjust the exact code based on how sessions currently create conversations. The key is to use `Conversation::new()` when storage is disabled.

**Verification**: Run `cargo build` to ensure changes compile.

---

### Step 3: Add Startup Message (10 min)

**File**: `src/tui/app_loop.rs` or equivalent TUI initialization

**3.1 Display message when storage disabled**

Find the TUI initialization or startup message section and add:

```rust
// During initialization, after config is loaded:
let storage_enabled = config.conversation_storage.unwrap_or(false);

if !storage_enabled {
    // Display message to user when storage is DISABLED (privacy-first default)
    console.info("Conversation storage disabled");
    // OR add to app_state messages:
    // app_state.add_system_message("Conversation storage disabled");
}
```

**Note**: The exact implementation depends on how hoosh displays system messages. Check existing code for patterns like startup messages or system notifications.

**Verification**: Run the app with storage disabled and confirm message appears.

---

### Step 4: Update Example Configuration (2 min)

**File**: `example_config.toml`

Add documentation for the new field:

```toml
# Disable conversation message storage (optional, defaults to false)
# When enabled, conversations run in memory only with no persistence
# Previously saved conversations remain accessible for reading
#
# This can be set at two levels:
#   - Global: ~/.config/hoosh/config.toml (user-wide default)
#   - Project: <project_root>/.hoosh/config.toml (overrides global)
#
# Use cases:
#   - Set globally to "true" for privacy by default
#   - Override per-project to "false" for specific work projects
#   - Enforce ephemeral mode for sensitive projects
#
# disable_conversation_storage = false
```

**Verification**: Ensure TOML is valid (no syntax errors).

---

### Step 5: Add Tests (15 min)

**File**: `src/config/mod_tests.rs`

**5.1 Test config parsing**

```rust
#[test]
fn test_conversation_storage_true_enables_persistence() {
    let config_content = r#"
        default_backend = "anthropic"
        conversation_storage = true
    "#;

    let config: AppConfig = toml::from_str(config_content).unwrap();
    assert_eq!(config.conversation_storage, Some(true));
}

#[test]
fn test_conversation_storage_false_disables_persistence() {
    let config_content = r#"
        default_backend = "anthropic"
        conversation_storage = false
    "#;

    let config: AppConfig = toml::from_str(config_content).unwrap();
    assert_eq!(config.conversation_storage, Some(false));
}

#[test]
fn test_conversation_storage_missing_defaults_to_none() {
    let config_content = r#"
        default_backend = "anthropic"
    "#;

    let config: AppConfig = toml::from_str(config_content).unwrap();
    assert_eq!(config.conversation_storage, None);
    // Note: None is treated as false (storage disabled, privacy-first)
}

#[test]
fn test_project_config_overrides_user_config() {
    let mut app_config = AppConfig {
        conversation_storage: Some(false),  // User has storage disabled
        // ... other required fields ...
    };

    let project_config = ProjectConfig {
        conversation_storage: Some(true),   // Project enables storage
        // ... other fields ...
    };

    app_config.merge(project_config);
    assert_eq!(app_config.conversation_storage, Some(true));
}
```

**5.2 Test conversation creation**

**File**: `tests/conversation_storage_test.rs` (new file)

```rust
use hoosh::agent::Conversation;
use tempfile::TempDir;
use std::sync::Arc;

#[test]
fn test_conversation_without_storage_creates_no_files() {
    let temp_dir = TempDir::new().unwrap();

    // Create conversation without storage
    let mut conversation = Conversation::new();
    conversation.add_user_message("test message".to_string());

    // Verify no files created
    let entries: Vec<_> = std::fs::read_dir(temp_dir.path())
        .unwrap()
        .collect();
    assert_eq!(entries.len(), 0, "No files should be created");
}

#[test]
fn test_conversation_with_storage_creates_files() {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(
        ConversationStorage::new(temp_dir.path().to_path_buf()).unwrap()
    );

    let mut conversation = Conversation::with_storage(
        "test-id".to_string(),
        storage
    );
    conversation.add_user_message("test message".to_string());

    // Verify files were created
    let conv_dir = temp_dir.path().join("test-id");
    assert!(conv_dir.exists());
    assert!(conv_dir.join("messages.jsonl").exists());
}
```

**Verification**: Run `cargo test` and ensure all tests pass.

---

### Step 6: Manual Testing (5-10 min)

**6.1 Test with storage disabled (privacy-first default)**

1. Edit `~/.config/hoosh/config.toml`:
   ```toml
   # Option 1: Explicit disable
   conversation_storage = false

   # Option 2: Omit field entirely (defaults to false)
   # (no conversation_storage line)
   ```

2. Run hoosh and start a conversation

3. Verify:
   - ✅ Startup message "Conversation storage disabled" appears
   - ✅ Conversation works normally
   - ✅ No new files in `.hoosh/conversations/`
   - ✅ Previous conversations still listed (if any exist)

4. Exit and restart hoosh

5. Verify:
   - ✅ Previous session not in history
   - ✅ Old conversations still accessible

**6.2 Test with storage enabled**

1. Edit config:
   ```toml
   conversation_storage = true  # Explicitly enable persistence
   ```

2. Run hoosh and start a conversation

3. Verify:
   - ✅ No startup message (storage is enabled)
   - ✅ Conversation works normally
   - ✅ Files created in `.hoosh/conversations/`

4. Exit and restart

5. Verify:
   - ✅ Previous session in history

**6.3 Test project override** (CRITICAL TEST)

This test verifies the two-tier configuration system works correctly.

1. Set global config at `~/.config/hoosh/config.toml`:
   ```toml
   default_backend = "anthropic"
   disable_conversation_storage = false  # Storage ENABLED globally
   ```

2. Create project config at `<project_root>/.hoosh/config.toml`:
   ```toml
   disable_conversation_storage = true  # Storage DISABLED for this project
   ```

3. Run hoosh from the project directory

4. Verify:
   - ✅ Project setting **overrides** global setting
   - ✅ Storage is **disabled** (project config wins)
   - ✅ Startup message shows "Conversation storage disabled"

5. Run hoosh from a different directory (without `.hoosh/config.toml`)

6. Verify:
   - ✅ Global setting applies
   - ✅ Storage is **enabled**
   - ✅ No startup message

**Expected Behavior**:
- Global config sets user-wide default
- Project config overrides when present
- This allows per-project privacy control

---

## Testing Checklist

- [ ] Config parsing tests pass
- [ ] Conversation creation tests pass
- [ ] Manual test: storage disabled works
- [ ] Manual test: storage enabled works (default)
- [ ] Manual test: project override works
- [ ] Manual test: startup message displays when disabled
- [ ] Manual test: no files created when disabled
- [ ] Manual test: old conversations accessible when disabled
- [ ] Backward compatibility: old configs without field still work
- [ ] `cargo clippy` passes with no warnings
- [ ] `cargo test` passes all tests
- [ ] `cargo build --release` succeeds

---

## Common Issues

### Issue: Storage still being created when disabled

**Symptom**: Files appear in `.hoosh/conversations/` even with `conversation_storage = false` or field omitted

**Solution**: Check that session initialization correctly reads the config and passes the flag through to conversation creation. Remember the logic is inverted: `true` = enabled, `false`/`None` = disabled. Add debug logging to verify config value is being read correctly.

---

### Issue: Startup message not appearing

**Symptom**: No "Conversation storage disabled" message when expected

**Solution**: Verify the message display code is in the correct TUI initialization path. Check that the config value is being read before message display.

---

### Issue: Tests failing with "field not found"

**Symptom**: Compilation errors about missing field

**Solution**: Ensure all structs with config fields have been updated. Check both `AppConfig` and `ProjectConfig`. Run `cargo clean && cargo build` to force recompilation.

---

## Rollback Plan

If issues arise, revert changes in reverse order:

1. Remove tests
2. Remove startup message code
3. Revert session initialization changes
4. Remove config struct fields
5. Run `cargo test` to verify rollback successful

---

## Next Steps

After implementation:

1. Update CLAUDE.md with new configuration option
2. Create pull request with:
   - All code changes
   - Test results
   - Manual testing verification
3. Consider future enhancements:
   - Status bar indicator for storage state
   - Warning when attempting to save disabled conversation
   - Per-conversation storage control

---

## Summary

**Files Modified**: 4-5
- `src/config/mod.rs` - config structures
- `src/session.rs` - session initialization
- `src/tui/app_loop.rs` - startup message
- `example_config.toml` - documentation
- `src/config/mod_tests.rs` - tests

**Files Created**: 1
- `tests/conversation_storage_test.rs` - integration tests

**Lines Changed**: ~50-100 total

**Complexity**: Low - leverages existing architecture
