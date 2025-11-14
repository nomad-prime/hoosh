# Test Coverage Analysis Report
**Current Coverage: 50% | Total Source Lines: ~23,072 | Total Test Functions: 183**

---

## Executive Summary

The codebase has **183 test functions** distributed across **40 test modules**, but they only cover ~50% of the code. The analysis identifies **high-value test opportunities** that would significantly improve coverage and catch critical bugs. The gaps are primarily in:

1. **Backend implementations** (API client logic) - 2,500+ lines untested
2. **TUI state management** - 2,000+ lines untested  
3. **Configuration & permissions** - 1,000+ lines untested
4. **Security validators** - 500+ lines untested

---

## Top Priority: High-Value Test Opportunities

### 🔴 CRITICAL (50-100% improvement potential per module)

#### 1. **Backend Implementations** (4 files, ~2,500 LOC)
**Files:** `anthropic.rs` (514 LOC), `openai_compatible.rs` (504 LOC), `together_ai.rs` (431 LOC), `ollama.rs` (416 LOC)

**Why it matters:**
- These are the API client layers that communicate with external LLM services
- Bugs here directly impact the core agent functionality
- Error handling and response parsing are critical

**What to test:**
- ✅ Request building with various message formats
- ✅ Response parsing (success cases, partial responses, errors)
- ✅ Error handling (network failures, timeouts, API errors)
- ✅ Tool/function calling request construction
- ✅ Message streaming and chunking
- ✅ Proxy and header configuration
- ✅ Token counting and limits

**Example high-value tests:**
```rust
#[test]
async fn test_anthropic_convert_messages_with_tool_calls() { }
#[test]
async fn test_anthropic_parse_error_responses() { }
#[test]
async fn test_anthropic_retry_on_timeout() { }
```

**Estimated lines to test:** 1,865 lines (77% gap)
**Estimated test effort:** 8-12 test functions per backend

---

#### 2. **TUI App State** (2 files, ~1,455 LOC)
**Files:** `app_state.rs` (728 LOC), `terminal/custom_terminal.rs` (727 LOC)

**Why it matters:**
- Core state management for the user interface
- Manages conversation history, active tool calls, dialog states
- Complex state transitions and user interactions

**What to test:**
- ✅ State initialization and defaults
- ✅ Adding/removing messages from conversation
- ✅ Active tool call tracking (status transitions)
- ✅ Dialog state management (permissions, approvals)
- ✅ Completion state and candidate selection
- ✅ UI mode transitions (normal, edit, dialog)
- ✅ Scrolling and viewport calculations

**Example high-value tests:**
```rust
#[test]
fn test_app_state_add_assistant_message() { }
#[test]
fn test_active_tool_call_status_transitions() { }
#[test]
fn test_completion_state_selection_cycle() { }
#[test]
fn test_dialog_state_option_navigation() { }
```

**Estimated lines to test:** 1,455 lines (100% gap)
**Estimated test effort:** 12-16 test functions

---

#### 3. **Config & Session Management** (2 files, ~1,000+ LOC)
**Files:** `config/mod.rs` (397 LOC), `session.rs` (300+ LOC)

**Why it matters:**
- Configuration parsing and validation affects all downstream components
- Session initialization orchestrates multiple subsystems
- Errors here prevent the application from starting

**What to test:**
- ✅ TOML config file parsing (valid & invalid configs)
- ✅ Backend configuration validation
- ✅ Default values and fallbacks
- ✅ Config merging (app + project config)
- ✅ Session initialization with various dependencies
- ✅ Conversation continuation from saved state

**Example high-value tests:**
```rust
#[test]
fn test_config_parse_valid_toml() { }
#[test]
fn test_config_merge_app_and_project() { }
#[test]
fn test_session_initialize_with_missing_backend() { }
#[test]
async fn test_session_load_existing_conversation() { }
```

**Estimated lines to test:** 697 lines (85% gap)
**Estimated test effort:** 8-10 test functions

---

### 🟠 HIGH (20-50% improvement potential per module)

#### 4. **Security & Path Validation** (1 file, ~592 LOC)
**Files:** `permissions/mod.rs` (592 LOC)

**Why it matters:**
- Permission management is security-critical
- Determines which tools users can execute
- Prevents unauthorized operations

**What to test:**
- ✅ Permission rule matching (glob patterns)
- ✅ Allow/deny decision logic
- ✅ Permission persistence to disk
- ✅ Project-wide vs specific permission scopes
- ✅ Async permission request/response flow

**Existing coverage:** Already has some tests in `path_validator.rs` but main `mod.rs` needs more

**Estimated lines to test:** ~200 lines (50% gap)
**Estimated test effort:** 6-8 additional test functions

---

#### 5. **Tool Execution & Registration** (1 file, ~442 LOC)
**Files:** `tools/mod.rs` (442 LOC)

**Why it matters:**
- Central registry for all tools (bash, file ops, grep, etc.)
- Tool resolution and error handling
- Bridges user requests to tool implementations

**What to test:**
- ✅ Tool registration and lookup
- ✅ Tool availability based on configuration
- ✅ Error handling for missing tools
- ✅ Tool parameter validation

**Existing coverage:** Has some tests but incomplete

**Estimated lines to test:** ~150 lines (35% gap)
**Estimated test effort:** 4-6 test functions

---

### 🟡 MEDIUM (5-20% improvement potential per module)

#### 6. **Agent Conversation Logic** (1 file, ~782 LOC)
**Files:** `agent/conversation.rs` (782 LOC)

**Why it matters:**
- Core data structures for multi-turn conversations
- Message building and serialization
- Tool call request/response handling

**Existing tests:** Has some tests in `core_tests.rs` but `conversation.rs` itself has limited coverage

**Test gaps:**
- ✅ ToolCallResponse error/success variants
- ✅ Conversation persistence (save/load)
- ✅ Message role validation

**Estimated test effort:** 4-6 test functions

---

#### 7. **Context Management Strategies** (1 file, ~789 LOC)
**Files:** `context_management/tool_output_truncation_strategy.rs`

**Existing coverage:** Already has tests

**Remaining gaps:** Edge cases in truncation logic
- ✅ Unicode handling
- ✅ Very large outputs (multi-MB)
- ✅ JSON output formatting

**Estimated test effort:** 3-4 edge case test functions

---

## File-by-File Gap Analysis

### Files with NO Tests (Priority Order by Impact)

| File | LOC | Type | Impact | Effort |
|------|-----|------|--------|--------|
| `anthropic.rs` | 514 | Backend | 🔴 Critical | High |
| `openai_compatible.rs` | 504 | Backend | 🔴 Critical | High |
| `app_state.rs` | 728 | TUI State | 🔴 Critical | High |
| `terminal/custom_terminal.rs` | 727 | TUI Terminal | 🔴 Critical | High |
| `together_ai.rs` | 431 | Backend | 🔴 Critical | High |
| `ollama.rs` | 416 | Backend | 🔴 Critical | Medium |
| `config/mod.rs` | 397 | Config | 🟠 High | Medium |
| `tui/markdown.rs` | 510 | Display | 🟠 High | Medium |
| `tui/layout_builder.rs` | ~200 | UI Layout | 🟠 High | Low |
| `tui/actions.rs` | ~150 | UI Events | 🟠 High | Low |
| `session.rs` | ~300 | Bootstrap | 🟠 High | Medium |
| `tui/events.rs` | ~150 | Events | 🟡 Medium | Low |
| `security/mod.rs` | 592 | Security | 🟡 Medium | Medium |

---

## Recommended Test Implementation Plan

### Phase 2: Backend APIs (2-3 days, +20-25% coverage)
Each backend needs comprehensive testing:

1. **`backends/anthropic.rs`** (10-12 tests)
2. **`backends/openai_compatible.rs`** (10-12 tests)
3. **`backends/together_ai.rs`** (8-10 tests)
4. **`backends/ollama.rs`** (8-10 tests)

**Expected coverage gain:** +15-20%

---

### Phase 3: TUI & Security (2-3 days, +10-15% coverage)
Polish edge cases:

1. **`tui/terminal/custom_terminal.rs`** (6-8 tests)
2. **`tui/markdown.rs`** (4-6 tests)
3. **`permissions/mod.rs`** - Additional security tests (6-8 tests)
4. **`agent/conversation.rs`** - Edge cases (4-6 tests)

**Expected coverage gain:** +10-12%

---

## Testing Patterns Already Established

The codebase has **good testing foundations** with these patterns you can replicate:

### ✅ Async Test Setup (`tools/bash.rs`)
```rust
#[tokio::test]
async fn test_bash_tool_simple_command() {
    let tool = BashTool::new();
    let args = serde_json::json!({ "command": "echo 'test'" });
    let result = tool.execute(&args).await.unwrap();
    assert!(result.contains("Exit code: 0"));
}
```

### ✅ Builder Pattern Testing (`task_management/task_manager.rs`)
```rust
#[test]
fn test_task_creation_with_builder() {
    let task = Task::builder()
        .name("test")
        .description("desc")
        .build();
    assert_eq!(task.name, "test");
}
```

### ✅ Error Case Testing (`tools/grep.rs`)
```rust
#[test]
fn test_error_on_invalid_regex() {
    let result = Regex::new("[invalid");
    assert!(result.is_err());
}
```

### ✅ Permission Testing (`permissions/mod.rs`)
```rust
#[test]
fn test_permission_matching() {
    let matcher = BashPatternMatcher;
    assert!(matcher.matches("rm -rf /", "rm -rf*"));
}
```

---

## Mock/Test Infrastructure Available

The project already uses:
- ✅ `tokio::test` for async tests
- ✅ `serde_json::json!` macros for test data
- ✅ `anyhow::Result` for error handling in tests
- ✅ Custom error types for assertion messages

---

## Specific Test Ideas by Module

### For Backend Services
```rust
#[test]
fn test_request_builder_with_system_prompt()
#[test]
fn test_tool_definition_conversion_to_api_format()
#[test]
fn test_error_response_parsing_and_mapping()
#[test]
async fn test_streaming_response_collection()
#[test]
fn test_message_role_validation()
#[test]
fn test_token_limit_validation()
```

### For TUI State
```rust
#[test]
fn test_message_deque_scrolling_boundaries()
#[test]
fn test_active_tool_call_lifecycle()
#[test]
fn test_dialog_state_option_cycling()
#[test]
fn test_completion_candidate_selection()
#[test]
fn test_ui_mode_transition_validity()
```

### For Configuration
```rust
#[test]
fn test_missing_required_field()
#[test]
fn test_invalid_backend_name()
#[test]
fn test_agent_definition_loading()
#[test]
fn test_context_manager_config_defaults()
```

---

## Summary Statistics

| Metric | Current | After Phase 1 | After Phase 2 | After Phase 3 |
|--------|---------|---------------|---------------|---------------|
| Coverage % | 50% | 60-65% | 75-80% | 85-90% |
| Test Functions | 183 | ~210 | ~280 | ~320 |
| Lines Tested | ~11,500 | ~13,500 | ~17,000 | ~19,500 |
| Untested Files | 18 | 15 | 8 | 2-3 |

---

## Risk Assessment

### Without Adding These Tests
- 🔴 Backend API changes go untested
- 🔴 TUI state corruption bugs in production
- 🔴 Config parsing silently fails
- 🔴 Permission logic regressions undetected

### After Implementation
- ✅ 85-90% coverage of critical paths
- ✅ Catch regressions in CI/CD
- ✅ Confidence in refactoring
- ✅ Better documentation via test examples

---

## Next Steps

1. **Start with Phase 1** - Pick `app_state.rs` first (largest single file, clearest patterns)
2. **Follow existing patterns** from well-tested modules like `bash.rs`
3. **Add both happy and sad paths** - Test errors as much as successes

