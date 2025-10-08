# Hoosh - Tasks and Improvements

### 7. Permission Manager Memory
**Priority:** MEDIUM
**Location:** `src/permissions/mod.rs:68-89`
**Issue:** Re-asks for permissions every time
**Fix:** Add session-based permission caching with "Allow for this session" option

### 8. Tool Registry Wrapping
**Priority:** MEDIUM
**Location:** `src/main.rs:68, 313`
**Issue:** Registry cloned multiple times, unclear intent
**Fix:** Wrap `ToolRegistry` itself in `Arc`

### 9. Silent File Reference Failures
**Priority:** MEDIUM
**Location:** `src/main.rs:82-84, 358`
**Issue:** Expansion errors logged but don't fail operation
**Fix:** Either fail early or make it more visible to user

## Low Priority Code Quality Issues

### 12. Duplicated Path Resolution
**Priority:** LOW
**Location:** All file operation tools
**Issue:** Identical `resolve_path()` methods
**Fix:** Extract to shared utility module

### 13. Hardcoded MAX_STEPS
**Priority:** LOW
**Location:** `src/main.rs:215`
**Issue:** `MAX_STEPS = 30` not configurable
**Fix:** Add to config file and CLI options

## Testing Improvements

### 15. Add Integration Tests
**Priority:** MEDIUM
**Issue:** Only unit tests exist
**Fix:** Add tests for full conversation flow with tool execution

### 16. Mock Backend Tool Testing
**Priority:** LOW
**Issue:** Mock backend may not properly test tool calls
**Fix:** Ensure mock exercises complete tool calling flow

## Documentation

### 17. Module Documentation
**Priority:** LOW
**Issue:** Most modules lack top-level docs
**Fix:** Add module-level documentation with usage examples

### 18. Move Agent Prompt to File
**Priority:** LOW
**Location:** `src/agents/mod.rs:33-62`
**Issue:** Long default prompt in code
**Fix:** Already writing to file, but consider making it more discoverable
