# Data Model: Disable Conversation Storage

**Feature**: 001-disable-conversation-storage
**Date**: 2025-12-11

## Overview

This feature introduces a single configuration field to control conversation persistence. The data model is minimal since we're leveraging existing storage abstractions.

## Entities

### AppConfig (Modified)

**Location**: `src/config/mod.rs`

**Purpose**: Global application configuration loaded from `~/.config/hoosh/config.toml`

**Fields** (new field only):

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `conversation_storage` | `Option<bool>` | No | `None` (treated as `false`) | When `true`, enables conversation persistence; when `false` or missing, runs in ephemeral mode (privacy-first default) |

**Existing Fields**: (unchanged)
- `default_backend: String`
- `backends: HashMap<String, BackendConfig>`
- `verbosity: Option<String>`
- `default_agent: Option<String>`
- `agents: HashMap<String, AgentConfig>`
- `context_manager: Option<ContextManagerConfig>`
- `core_reminder_token_threshold: Option<usize>`

**Serialization**: TOML via serde

**Example**:
```toml
default_backend = "anthropic"
conversation_storage = true  # Explicitly enable persistence

[backends.anthropic]
api_key = "sk-..."
model = "claude-sonnet-4"
```

**Validation Rules**:
- Must be valid boolean if present
- Defaults to `false` (storage **disabled**, privacy-first) if missing or invalid
- No interaction with other config fields

---

### ProjectConfig (Modified)

**Location**: `src/config/mod.rs`

**Purpose**: Project-specific configuration overrides loaded from `.hoosh/config.toml`

**Fields** (new field only):

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `conversation_storage` | `Option<bool>` | No | `None` | Overrides user config when present |

**Behavior**:
- When set in project config, overrides user config value
- Allows per-project control over storage behavior
- Useful for enabling persistence in trusted projects or disabling for sensitive work

**Example**:
```toml
# .hoosh/config.toml (project-specific)
conversation_storage = true  # Override to enable storage for this project
```

---

### Conversation (Unchanged)

**Location**: `src/agent/conversation.rs`

**Purpose**: Runtime representation of a conversation

**Relevant Field**:

| Field | Type | Description |
|-------|------|-------------|
| `storage` | `Option<Arc<ConversationStorage>>` | Optional storage backend (already supports None) |

**Key Methods**:
- `Conversation::new()` → Creates conversation **without** storage (storage = None)
- `Conversation::with_storage(id, storage)` → Creates conversation **with** storage
- `persist_message()` → Only persists if `storage.is_some()`

**State Transitions**:

```
┌─────────────────────────────────────┐
│  Session Initialization             │
└──────────────┬──────────────────────┘
               │
               ├─ conversation_storage = true?
               │  YES → Conversation::with_storage()
               │         (storage = Some(Arc<ConversationStorage>))
               │
               └─ NO (false or None) → Conversation::new()
                       (storage = None, privacy-first default)
```

**Storage Behavior**:

| Scenario | Method Called | Result |
|----------|---------------|--------|
| Storage disabled (privacy-first default) | `new()` | Messages kept in memory only, no files created |
| Storage enabled (`conversation_storage = true`) | `with_storage()` | Messages persisted to `.hoosh/conversations/{id}/messages.jsonl` |
| Add message (storage enabled) | `add_user_message()` | Calls `persist_message()` → writes to JSONL |
| Add message (storage disabled) | `add_user_message()` | Calls `persist_message()` → no-op (storage is None) |

---

## Configuration Lifecycle

### Load Process

```
1. AppConfig::load()
   ├─ Load ~/.config/hoosh/config.toml
   │  └─ Parse conversation_storage (if present)
   │
   ├─ Load .hoosh/config.toml (if exists)
   │  └─ Parse conversation_storage (if present)
   │
   └─ Merge ProjectConfig into AppConfig
      └─ Project value overrides user value if both present
```

### Merge Logic

```rust
// Pseudocode for merge behavior
if project_config.conversation_storage.is_some() {
    app_config.conversation_storage = project_config.conversation_storage;
}
```

**Truth Table**:

| User Config | Project Config | Final Value | Notes |
|-------------|----------------|-------------|-------|
| `None` | `None` | `false` | **Privacy-first default** (storage disabled) |
| `true` | `None` | `true` | User config applied (storage enabled) |
| `false` | `None` | `false` | User config applied (storage disabled) |
| `true` | `false` | `false` | Project overrides (disable for sensitive project) |
| `false` | `true` | `true` | Project overrides (enable for trusted project) |
| `None` | `true` | `true` | Project provides value (enable storage) |

---

## Validation Rules

### Config Parsing

| Input | Valid? | Resulting Value | Notes |
|-------|--------|-----------------|-------|
| `conversation_storage = true` | ✅ Yes | `Some(true)` | Explicit enable |
| `conversation_storage = false` | ✅ Yes | `Some(false)` | Explicit disable |
| (field missing) | ✅ Yes | `None` | Defaults to false (privacy-first) |
| `conversation_storage = "yes"` | ❌ No | TOML parse error | Invalid type |
| `conversation_storage = 1` | ❌ No | TOML parse error | Invalid type |

### Runtime Behavior

| Config Value | Interpretation | Behavior |
|--------------|----------------|----------|
| `Some(true)` | Storage enabled | Use `Conversation::with_storage()` |
| `Some(false)` | Storage disabled | Use `Conversation::new()` |
| `None` | Storage disabled (privacy-first default) | Use `Conversation::new()` |

**Decision Logic**:
```rust
let storage_enabled = config
    .conversation_storage
    .unwrap_or(false);  // Privacy-first: default to disabled

if storage_enabled {
    Conversation::with_storage(id, storage)
} else {
    Conversation::new()
}
```

---

## File System Impact

### Storage Disabled (conversation_storage = false or None, privacy-first default)

**Files Created**:
- None (conversation runs entirely in memory)

**Files Accessible**:
- All existing conversations in `.hoosh/conversations/` remain readable
- Index and metadata from previous sessions accessible

**On Application Exit**:
- Current conversation discarded
- No trace of session on disk (except logs/telemetry if configured)

---

### Storage Enabled (conversation_storage = true)

**Files Created** (per conversation):
```
.hoosh/conversations/
├── index.json                      # Updated with new conversation
└── {conversation_id}/
    ├── messages.jsonl              # New messages appended
    └── meta.json                   # Metadata (title, timestamps, count)
```

**On Application Exit**:
- All messages persisted to disk
- Conversation available in history on next launch

---

## Relationships

```
┌─────────────────┐
│   AppConfig     │
│  ┌──────────────┴────────┐
│  │ disable_conversation_ │
│  │ storage: Option<bool> │
│  └──────────┬────────────┘
└─────────────┼─────────────┘
              │
              │ influences
              ▼
┌──────────────────────────┐
│  Session Initialization  │
│  (src/session.rs)        │
└──────────┬───────────────┘
           │
           │ creates
           ▼
┌──────────────────────────┐
│    Conversation          │
│  ┌──────────────────────┐│
│  │ storage: Option<Arc> ││
│  │   <ConversationStorage>
│  └──────────────────────┘│
└───────────────────────────┘
           │
           │ uses (if Some)
           ▼
┌──────────────────────────┐
│  ConversationStorage     │
│  (file system backend)   │
└──────────────────────────┘
```

---

## Data Integrity Considerations

### Backward Compatibility

⚠️ **Breaking Change**: Existing config files without `conversation_storage` field will have storage **disabled** by default
- Missing field defaults to `None` → treated as `false` (storage disabled)
- **Migration required**: Users must add `conversation_storage = true` to re-enable persistence
- Acceptable since hoosh is pre-production

### Forward Compatibility

✅ **Guaranteed**: Older versions of hoosh ignore unknown config fields
- TOML parsers skip unknown fields by default
- No version conflicts

### Data Loss Prevention

✅ **Protected**:
- Storage disabled → no data written → no data to lose
- Storage enabled → existing behavior unchanged
- Config change mid-session → ignored until restart (per FR-005)

⚠️ **User Awareness**:
- Users should be warned via documentation that storage disabled = ephemeral conversations
- Startup message "Conversation storage disabled" provides runtime confirmation

---

## Summary

**Entities Modified**: 2
- `AppConfig` - add 1 field
- `ProjectConfig` - add 1 field

**Entities Unchanged**: 2
- `Conversation` - already supports optional storage
- `ConversationStorage` - no changes needed

**New Entities**: 0

**Total Complexity**: Very low - single boolean field with simple merge logic

**Storage Impact**:
- Disabled: Zero files created
- Enabled: Identical to current behavior
