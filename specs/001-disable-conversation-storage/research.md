# Research: Disable Conversation Storage

**Feature**: 001-disable-conversation-storage
**Date**: 2025-12-11
**Status**: Complete

## Overview

This document captures research findings and technical decisions for implementing a configuration option to disable conversation storage in hoosh.

## Research Questions

### 1. How does the current configuration system work?

**Finding**: The hoosh configuration system uses TOML files with a two-tier override mechanism:

- **User config**: `~/.config/hoosh/config.toml` (global settings)
- **Project config**: `.hoosh/config.toml` (project-specific overrides)

**Implementation Details**:
- `AppConfig` struct in `src/config/mod.rs` (lines 84-98) defines available options
- `ProjectConfig` struct provides override capability
- `AppConfig::load()` merges both configs (project overrides user)
- Uses serde for TOML serialization/deserialization
- Config files enforce 0600 permissions on Unix for security

**Decision**: Add `conversation_storage: Option<bool>` field to both `AppConfig` and `ProjectConfig` structs, following the existing pattern for optional configuration fields.

**Rationale**:
- Positive naming (`conversation_storage`) is clearer than negative (`disable_conversation_storage`)
- Maintains consistency with existing config design
- Allows both global and per-project control over storage behavior
- Defaults to `false` (storage disabled) for privacy-first approach

**Alternatives Considered**:
- Environment variable: Rejected - not consistent with current config approach
- Command-line flag: Rejected - config file is more permanent and discoverable
- Separate config file: Rejected - unnecessary complexity

---

### 2. How is conversation storage currently implemented?

**Finding**: Hoosh uses a file-based storage system with the following architecture:

**Storage Structure**:
```
.hoosh/conversations/
├── index.json                  # Index of all conversations
└── {conversation_id}/
    ├── messages.jsonl          # Messages in JSONL format (one per line)
    └── meta.json              # Conversation metadata
```

**Key Components**:
- `ConversationStorage` struct manages file I/O operations
- `Conversation` struct holds messages and optional storage reference: `storage: Option<Arc<ConversationStorage>>`
- Messages are appended to JSONL file via `persist_message()` method
- Metadata (title, timestamps, message count) stored separately

**Critical Insight**: The `Conversation` struct already supports optional storage! The `storage` field is `Option<Arc<ConversationStorage>>`, which means conversations can be created without storage:

```rust
// From src/agent/conversation.rs
pub fn new() -> Self {
    // Creates conversation WITHOUT storage
}

pub fn with_storage(id: String, storage: Arc<ConversationStorage>) -> Self {
    // Creates conversation WITH storage
}
```

**Decision**: Leverage the existing `Conversation::new()` method when storage is disabled. No changes needed to the storage module itself.

**Rationale**: The architecture already supports this use case. We only need to control which constructor is called based on the config flag.

**Alternatives Considered**:
- Null object pattern for storage: Rejected - unnecessary when `Option<T>` already handles this
- Conditional storage methods: Rejected - already implemented via `Option<Arc<ConversationStorage>>`

---

### 3. Where should the storage check be implemented?

**Finding**: Session initialization occurs in `src/session.rs` (line 151):

```rust
let storage = ConversationStorage::with_default_path()?;
```

The `ConversationState` in `src/tui/app_loop.rs` (lines 38-45) holds both conversation and storage.

**Decision**: Implement the check in `session.rs` during initialization:

1. Read `conversation_storage` config value
2. If `true`: create conversation with `Conversation::with_storage()` (enable persistence)
3. If `false` or `None` (missing): create conversation with `Conversation::new()` (no storage, privacy-first)

**Rationale**: Session initialization is the single point where conversations are created, making it the ideal place to apply the configuration. Privacy-first default means storage is disabled unless explicitly enabled.

**Alternatives Considered**:
- Check in `Conversation` constructor: Rejected - violates separation of concerns (conversation shouldn't know about config)
- Check in storage module: Rejected - storage module shouldn't know about config either

---

### 4. What about reading previously saved conversations?

**Finding**: The clarification session established that:
- Previously saved conversations should remain accessible when storage is disabled
- Only new message persistence should be blocked

**Decision**: Storage disable affects only new conversation creation. Conversation loading and listing remain functional:

- `ConversationStorage::load_messages()` - still works
- `ConversationStorage::list_conversations()` - still works
- Only `ConversationStorage::append_message()` is bypassed for new conversations

**Rationale**: Users may want to review old conversations even when preventing new storage. This separation provides maximum flexibility.

**Alternatives Considered**:
- Hide old conversations when storage disabled: Rejected per clarification session (user chose option B)
- Separate config for read vs write: Rejected as too complex for current requirements

---

### 5. How should the startup message be displayed?

**Finding**: The TUI console system in `src/console.rs` and `src/tui/app_loop.rs` handles display output.

**Decision**: Add a simple startup message "Conversation storage disabled" when the flag is enabled. This should be displayed:
- During TUI initialization (in `app_loop.rs`)
- As a system message or info banner
- Only when `disable_conversation_storage` is true

**Rationale**: Simple, non-intrusive feedback that confirms the setting is active without cluttering the interface.

**Alternatives Considered**:
- Persistent indicator in status bar: Considered for future enhancement
- Detailed explanation message: Rejected - too verbose per clarification session (user chose simple message)
- No message: Rejected - user needs confirmation the setting is active

---

### 6. What validation and error handling is needed?

**Finding**: Current config system handles:
- Missing config files (uses defaults)
- Invalid TOML syntax (returns error during parse)
- Missing fields (uses `Option<T>` with None default)

**Decision**: For `conversation_storage`:
- Missing field → defaults to `None` → treated as `false` (storage **disabled**, privacy-first)
- Invalid boolean value → TOML parse error (existing behavior)
- Malformed value → defaults to `false` (storage disabled)

**Rationale**: Follows existing config error handling patterns. Privacy-first default means storage is disabled unless user explicitly enables it. This is a breaking change but acceptable since hoosh is pre-production.

**Alternatives Considered**:
- Strict validation with startup failure: Rejected - too disruptive for optional feature
- Warning message for invalid values: Could be added in future enhancement

---

### 7. Testing strategy

**Finding**: Existing test patterns:
- Unit tests in module files (e.g., `config/mod_tests.rs`)
- Uses `tempfile` crate for temporary test directories
- Async tests with `#[tokio::test]`

**Decision**: Add tests for:

1. **Config parsing tests** (in `config/mod_tests.rs`):
   - `test_conversation_storage_true()` - verify `true` enables storage
   - `test_conversation_storage_false()` - verify `false` disables storage
   - `test_conversation_storage_missing()` - verify defaults to `false` (privacy-first)
   - `test_conversation_storage_invalid()` - verify defaults to `false` on parse error

2. **Conversation creation tests** (new integration test):
   - `test_conversation_without_storage()` - verify no files created when `false`
   - `test_conversation_with_storage()` - verify files created when `true`
   - `test_messages_not_persisted_when_disabled()` - verify JSONL file not written

**Rationale**: Comprehensive coverage of config parsing and storage behavior ensures feature works correctly and doesn't regress existing functionality.

---

## Technology Decisions

### Configuration Format

**Decision**: Use TOML boolean in existing config structure:

```toml
# Enable conversation storage (defaults to false, privacy-first)
conversation_storage = true   # Explicitly enable persistence
conversation_storage = false  # Explicitly disable (or omit for default)
```

**Rationale**:
- Positive naming is clearer than negative (`conversation_storage` vs `disable_conversation_storage`)
- Consistent with existing configuration approach
- TOML has native boolean type
- Clear, readable syntax
- No new dependencies required
- Privacy-first default (false = disabled)

---

### Message Display Approach

**Decision**: Add simple text message during TUI initialization

**Rationale**:
- Minimal code changes
- Clear user feedback
- Non-intrusive to workflow
- Matches user preference from clarification session

---

## Risk Assessment

### Low Risk Areas
- ✅ Config parsing - well-established pattern
- ✅ Conversation creation - already supports optional storage
- ✅ Testing - existing infrastructure sufficient

### Medium Risk Areas
- ⚠️ Session initialization changes - carefully preserve existing behavior
- ⚠️ TUI message display - ensure doesn't interfere with normal operation

### Mitigation Strategies
- Comprehensive test coverage for both storage-enabled and disabled modes
- Default to storage enabled for backward compatibility
- Minimal changes to existing code paths

---

## Dependencies

**New Dependencies**: None

**Existing Dependencies Used**:
- `serde` - config serialization (already present)
- `toml` - config parsing (already present)
- `anyhow` - error handling (already present)

---

## Implementation Checklist

- [ ] Add `conversation_storage` field to `AppConfig`
- [ ] Add `conversation_storage` field to `ProjectConfig`
- [ ] Update `AppConfig::merge()` to handle new field
- [ ] Modify `session.rs` to check config: `true` = with storage, `false`/`None` = without storage
- [ ] Add startup message "Conversation storage disabled" when storage is off
- [ ] Document field in `example_config.toml` with privacy-first default explanation
- [ ] Add config parsing tests
- [ ] Add conversation creation tests
- [ ] Update CLAUDE.md with new config option

---

## Summary

The implementation is straightforward due to excellent existing architecture:

1. **Config System**: Add single boolean field using established pattern
2. **Storage Architecture**: Already supports optional storage via `Option<Arc<ConversationStorage>>`
3. **Session Initialization**: Simple conditional to choose conversation constructor
4. **Testing**: Follow existing test patterns with no new infrastructure needed

**Estimated Complexity**: Low - approximately 50-100 lines of new code, mostly config boilerplate and tests.

**Breaking Change**: Privacy-first default (storage disabled by default). Users must set `conversation_storage = true` to enable persistence. Acceptable since hoosh is pre-production.
