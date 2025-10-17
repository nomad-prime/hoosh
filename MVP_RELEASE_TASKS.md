# Hoosh MVP Release Task List

**Goal**: Prepare hoosh for local project usage with minimum viable features, clean code, and production readiness.

**Current Status**: Development mode, running via `cargo run`, basic features working.

---

## 🔴 CRITICAL - Must Fix Before Any Use

### 1. Security Audit - API Key Handling

- **Issue**: API keys stored in plaintext in `~/.config/hoosh/config.toml`
- **Risk**: HIGH - keys could be exposed if config file is accidentally committed or shared
- **Recommendations**:
    - Add warning in README about protecting config file
    - Validate file permissions on config file (should be 0600)
- **Priority**: CRITICAL

### 2. Error Handling - Remove Unwrap() Calls

- **Issue**: 26 instances of `.unwrap()` found in codebase
- **Risk**: MEDIUM - potential panics in production
- **Action**: Audit all unwrap() calls and replace with proper error handling
- **Priority**: HIGH
- **Effort**: 2-3 hours
- **Files to check**: All `.rs` files with unwrap() calls

---

## 🟡 HIGH PRIORITY - MVP Blockers

### 3. Graceful LLM Error Handling & Recovery

- **Issue**: No graceful handling of downstream LLM errors (rate limits, API failures, timeouts)
- **Risk**: HIGH - Poor user experience when LLM backends fail mid-conversation
- **Required**:
    - Detect and handle HTTP 429 (rate limit) errors with automatic retry logic
    - Detect and handle 5xx server errors with exponential backoff
    - Offer user option to switch model mid-flight (e.g., from gpt-4 to gpt-3.5-turbo)
    - Offer user option to switch backend mid-flight (e.g., from OpenAI to Anthropic)
    - Display clear, actionable error messages (e.g., "Rate limit hit. Retry in 20s or switch model?")
    - Implement `/switch-model <model>` and `/switch-backend <backend>` commands
    - Preserve conversation context when switching backends/models
- **Priority**: HIGH
- **Effort**: 4-5 hours
- **Why MVP**: Essential for production reliability and user experience

### 4. Installation & Distribution

- **Issue**: Currently requires `cargo run` to execute
- **Required**: use `cargo-dist` to create distributable binaries for major platforms (Linux, macOS, Windows)
- **Documentation**: Add installation instructions to README.md
- **Priority**: HIGH
- **Effort**: 2-3 hours

### 5. Conversation Persistence

- **Status**: Not implemented (mentioned in ROADMAP)
- **Required Commands**:
    - `/save [name]` - Save current conversation
    - `/load <name>` - Load saved conversation
    - `/list` - List saved conversations
    - `/delete <name>` - Delete conversation
- **Storage**: `~/.config/hoosh/conversations/` (JSON format)
- **Why MVP**: Essential for multi-session work
- **Priority**: HIGH
- **Effort**: 4-6 hours
- **Dependencies**: Command system (✅ already implemented)

### 6. Better Error Messages

- **Issue**: Some errors are cryptic (e.g., backend configuration errors)
- **Required**:
    - User-friendly error messages with actionable suggestions
    - Better validation on startup (check for API keys, validate config)
    - Graceful degradation when backend unavailable
- **Priority**: HIGH
- **Effort**: 3-4 hours

### 7. Documentation Improvements

- **Missing**:
    - Quick start guide for first-time users
    - Troubleshooting section
    - Example workflows
    - Command reference
    - Agent system explanation
- **Required Files**:
    - QUICKSTART.md
    - TROUBLESHOOTING.md
    - Update README.md with better examples
- **Priority**: HIGH
- **Effort**: 3-4 hours

---

## 🟢 MEDIUM PRIORITY - Quality of Life

### 8. Config Validation & Defaults

- **Issue**: Silent failures when config is invalid
- **Required**:
    - Validate config on load with helpful error messages
    - Better defaults (e.g., use mock backend if no API key)
    - Config migration/upgrade system for future versions
    - `hoosh config validate` command
- **Priority**: MEDIUM
- **Effort**: 2-3 hours

### 9. Command History Persistence

- **Status**: In-memory only (mentioned in ROADMAP)
- **Required**: Save command history to `~/.config/hoosh/command_history`
- **Why Useful**: Improve UX with persistent history across sessions
- **Priority**: MEDIUM
- **Effort**: 1-2 hours

### 10. Better Logging System

- **Issue**: Debug messages sent via AgentEvent but not used
- **Required**:
    - Proper logging framework (e.g., `tracing` or `env_logger`)
    - Log file at `~/.config/hoosh/logs/hoosh.log`
    - Configurable log levels
    - Log rotation
- **Priority**: MEDIUM
- **Effort**: 2-3 hours

### 11. Graceful Shutdown

- **Issue**: No cleanup on exit (e.g., save unsaved work)
- **Required**:
    - Prompt to save conversation if modified
    - Clean up temp files
    - Close backend connections gracefully
- **Priority**: MEDIUM
- **Effort**: 1-2 hours

---

## 🔵 LOW PRIORITY - Nice to Have

### 12. Testing Coverage

- **Current**: 57 tests, good coverage
- **Improvements**:
    - Integration tests for full workflows
    - Backend integration tests (with mocks)
    - TUI testing (if feasible)
    - Benchmark tests for performance
- **Priority**: LOW (but important for long-term)
- **Effort**: 8-10 hours

### 13. Code Quality Improvements

- **Issues**:
    - Some functions are too long (e.g., `create_backend` in main.rs)
    - Repeated code patterns (e.g., config path handling)
    - Missing documentation comments on public APIs
- **Required**:
    - Refactor large functions
    - Extract common utilities
    - Add rustdoc comments
    - Run `cargo fmt` and `cargo clippy --all-features` regularly
- **Priority**: LOW
- **Effort**: 4-6 hours

### 14. Better Agent System

- **Current**: Basic agent loading from files
- **Improvements**:
    - Hot-reload agents without restart
    - Agent validation on load
    - Better error messages when agent file missing
    - `/agent reload` command
    - cycle between agents in TUI
- **Priority**: LOW
- **Effort**: 2-3 hours

### 15. Bash Tool Improvements

- **Current**: Good security checks
- **Improvements**:
    - Whitelist mode (only allow specific commands)
    - Command aliases for common operations
    - Better output formatting
    - Support for interactive commands (with timeout)
- **Priority**: LOW
- **Effort**: 3-4 hours

---

## 🚫 OUT OF SCOPE FOR MVP (Post-v1)

These are explicitly mentioned in ROADMAP as post-v1:

1. **Web Search Tool** - Requires API integration, rate limiting, etc.
2. **MCP (Model Context Protocol)** - Large feature, needs server integration
3. **LSP Integration** - Complex, needs language server management
4. **Project Indexing** - Requires AST parsing, symbol indexing
5. **Multi-Agent Orchestration (ACE)** - Advanced feature, needs reflection/curator agents
6. **Screenshot Tool** - Platform-specific, not critical
7. **Markdown Rendering in TUI** - Nice-to-have UI enhancement
8. **Multi-file Operations** - Can be done with multiple commands for MVP
